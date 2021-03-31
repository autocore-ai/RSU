use std::error::Error;
use std::fs;
use std::path::Path;
use log::{info, debug, error};
use std::thread;
use std::thread::JoinHandle;
use libloading::Library;
use std::sync::Arc;
use std::sync::Mutex;
use std::collections::HashMap;
extern crate yaml_rust;
use yaml_rust::{YamlLoader, YamlEmitter, Yaml};
use linked_hash_map::LinkedHashMap;
use serde::{Deserialize, Serialize};


type PluginFunc = unsafe extern fn(running_flag: Arc<Mutex<bool>>, error_flag: Arc<Mutex<bool>>) -> i32;

#[derive(Debug)]
pub struct Plugin {
    lib_handle: Arc<Mutex<Library>>,
    thread_handle: Option<JoinHandle<Result<i32, String>>>,
    running_flag: Arc<Mutex<bool>>,
    error_flag: Arc<Mutex<bool>>,
}

impl Plugin {
    pub fn new(path: &str) -> Result<Plugin, String> {
        unsafe {
            match Library::new(path) {
                Ok(lib) => {
                    let lib_handle = Arc::new(Mutex::new(lib));
                    Ok(Plugin {
                        lib_handle,
                        running_flag: Arc::new(Mutex::new(true)),
                        thread_handle: None,
                        error_flag: Arc::new(Mutex::new(false))
                    })
                },
                Err(error) => {
                    Err(format!("Problem opening the file: {:?}", error))
                },
            }
        }
    }

    fn start(&mut self) -> Result<(), String>{
        let lib = Arc::clone(&self.lib_handle);
        let flag = Arc::clone(&self.running_flag);
        let error_flag = Arc::clone(&self.error_flag);


        let join_handle = thread::spawn(move || -> Result<i32, String> {
            unsafe {
                let h = lib.lock().map_err(|e|
                    {
                        error!("lock lib failed: {:?}", e);
                        format!("lock lib failed: {:?}", e)
                    })?;

                let func = h.get::<PluginFunc>(b"run").map_err(|e|
                    {
                        error!("get lib fun[run]failed: {:?}", e);
                        format!("get lib fun[run]failed: {:?}", e)
                    })?;
                
                let ret = func(flag, error_flag);
                debug!("plugin func ret: {:?}", ret);
                if ret < 0 {
                    error!("start plugin failed: {:?}", ret);
                    return Err(format!("start plugin failed: {:?}", ret))
                }

                Ok(ret)
            }
        });
        
        self.thread_handle = Some(join_handle);
        Ok(())
    }

    fn stop(&mut self) -> Result<i32, String> {
        {
            let mut the_flag = self.running_flag.lock().map_err(|e| {
                error!("stop plugin failed: {:?}", e);
                format!("stop plugin failed: {:?}", e)
            })?;
            *the_flag = false;
        }

        if let Some(handle) = self.thread_handle.take() {
            match handle.join() {
                Ok(ret) => {
                    debug!("handle join ret {:?}", ret);
                    ret
                },
                Err(e) => {
                    error!("handle join failed {:?}", e);
                    Err(format!("stop plugin failed: {:?}", e))
                }
            }

        } else {
            error!("take plguin handle return None");
            return Err(String::from("take plguin handle return None")) 
        }
    }

    fn check(&mut self)-> Result<(), String> {
        let error_flag = Arc::clone(&self.error_flag);

        match error_flag.lock(){
            Ok(flag) => {
                if *flag {
                    error!("running plugin error");
                    return Err(format!("running plugin error"))
                }
            },
            Err(e) => {
                debug!("lock error_flag failed: {:?}", e);
                return Err(format!("start plugin failed: {:?}", e))
            },
        };
        
        Ok(())
    }
}


#[derive(Deserialize, Serialize)]
#[derive(Debug)]
pub struct PluginInfo {
    path: String,
    active: bool
}

#[derive(Debug)]
pub struct PluginMgr {
    config_path: String,
    pub plugin_cfg: HashMap<String, PluginInfo>,
    plugins: HashMap<String, Plugin>,
}

impl PluginMgr {
    pub fn new(path: &str) -> Result<PluginMgr, Box<dyn Error>> {
        if !Path::new(path).exists() {
            let mut dir_path_vec:Vec<&str> = path.split('/').collect();
            dir_path_vec.pop();
            fs::create_dir_all(dir_path_vec.join("/"))?;
            fs::File::create(path)?;
            generate_cfg(path)?;
        }
        let mut obj = PluginMgr {config_path: String::from(path), plugin_cfg: HashMap::new(), plugins: HashMap::new()};
        let config_str = fs::read_to_string(path)?;
        let config_docs = YamlLoader::load_from_str(config_str.as_str())?;
        let config = &config_docs[0];
        let plugin_cfg = &config["plugins"];
        for (name, info) in plugin_cfg.as_hash().ok_or("read plugin config failed".to_owned())?.into_iter() {
            let name = name.as_str().ok_or("read plugin name failed")?;
            let path = info["path"].as_str().ok_or("read plugin path failed")?;
            let active = info["active"].as_bool().ok_or("read plugin active failed")?;
            obj.add_plugin_inner(name, path, active).expect("initial plugin manage failed: ");
            info!("plugin added, name:{:?}, path: {}, actice: {}", name, path, active);
        }
        Ok(obj)
    }

    fn start_plugin_inner(&mut self, name: &str) -> Result<String, String> {
        if self.plugins.contains_key(name) {
            debug!("plugin[{}] is already running", name);
            return Ok(format!("plugin[{}] is already running", name));
        }
        let mut plugin_info = self.plugin_cfg.get_mut(name).ok_or(format!("get plugin[{}] info failed, plugin does not exist", name))?;
        plugin_info.active = true;
        let mut plugin = Plugin::new(&plugin_info.path[..])?;

        plugin.start()?;
        debug!("start up plugin[{}] successful", name);
        
        self.plugins.insert(String::from(name), plugin);
        Ok(format!("plugin[{}] is running", name))
    }

    pub fn start_plugin(&mut self, name: &str) -> Result<String, String> {
        let desc = self.start_plugin_inner(name)?;
        self.flush_cfg_to_file()?;
        info!("start up plugin[{}] successful", name);
        Ok(desc)
    }

    pub fn stop_plugin(&mut self, name: &str) -> Result<String, String> {
        if !self.plugins.contains_key(name) {
            debug!("plugin[{}] is not running", name);
            return Ok(format!("plugin[{}] is not running", name))
        }
        self.plugins.get_mut(name).ok_or(format!("get plugin[{}] failed from plugins", name))?.stop()?;
        self.plugins.remove(name);
        let mut plugin_info = self.plugin_cfg.get_mut(name)
                                    .ok_or(format!("stop plugin[{}], get plugin info failed from plugin cfg", name))?;
        plugin_info.active = false;
        self.flush_cfg_to_file()?;
        info!("plugin[{}] stopped", name);
        Ok(format!("plugin[{}] stopped", name))
    }

    fn add_plugin_inner(&mut self, name: &str, path: &str, active: bool) -> Result<String, String> {
        if self.plugin_cfg.contains_key(name) {
            return Ok(format!("plugin[{}] has been added", name));
        }
        self.plugin_cfg.insert(String::from(name), PluginInfo {path: String::from(path), active});
        if active {
            self.start_plugin_inner(name)?;
        }
        Ok(format!("plugin[{}] added", name))
    }

    pub fn add_plugin(&mut self, name: &str, path: &str, active: bool) -> Result<String, String> {
        let desc = self.add_plugin_inner(name, path, active)?;
        self.flush_cfg_to_file()?;
        info!("add plugin[{}] successful", name);
        Ok(desc)
    }

    pub fn remove_plugin(&mut self, name: &str) -> Result<String, String> {
        if self.plugins.contains_key(name) {
            self.stop_plugin(name)?;
        }
        self.plugin_cfg.remove(name);
        self.flush_cfg_to_file()?;
        info!("remove plugin[{}] successful", name);
        Ok(format!("plugin[{}] has been removed", name))
    }

    fn flush_cfg_to_file(&mut self) -> Result<(), String> {
        let mut node_map: LinkedHashMap<Yaml, Yaml> = LinkedHashMap::new();
        for (name, info) in self.plugin_cfg.iter_mut() {
            let mut info_map: LinkedHashMap<Yaml, Yaml> = LinkedHashMap::new();
            info_map.insert(Yaml::from_str("path"), Yaml::from_str(&info.path[..]));
            info_map.insert(Yaml::from_str("active"), Yaml::from_str(if info.active {"true"} else {"false"}));
            let info_node: Yaml = Yaml::Hash(info_map);
            node_map.insert(Yaml::from_str(name), info_node);
        }
        let mut root_map: LinkedHashMap<Yaml, Yaml> = LinkedHashMap::new();
        root_map.insert(Yaml::from_str("plugins"), Yaml::Hash(node_map));
        let root_node = Yaml::Hash(root_map);

        let mut out_str = String::new();
        let mut emitter = YamlEmitter::new(&mut out_str);
        match emitter.dump(&root_node) {
            Ok(_) => {
                let _ = fs::write(&self.config_path[..], out_str);
                debug!("flush config into plugin config successful");
                Ok(())
            },
            Err(e) => {
                Err(format!("flush config into plugin config failed: {:?}", e))
            }
        }
    }

    pub fn check_plugin(&mut self) -> Result<(), String> {
        for (name, plugin_info) in self.plugin_cfg.iter_mut() {
            if plugin_info.active {
                let plugin = self.plugins.get_mut(name).ok_or(format!("get plugin[{}] failed from plugins", name))?;
                match plugin.check() {
                    Ok(_) => {
                        debug!("plugin {} running ok", name);
                    },
                    Err(_) => {
                        error!("plugin running error, now stop plugin: {}", name);
                        plugin.stop()?;
                        self.plugins.remove(name);
                        plugin_info.active = false;
                        return Ok(())
                    }
                }
            }
        }

        Ok(())
    }
}


fn generate_cfg(cfg_path: &str) -> Result<(), String>{
    let plugin_default = r#"---
plugins:
    vehicle_status:
        path: libvehicle_status.so
        active: false
    traffic_light:
        path: libtraffic_light.so
        active: false"#;

    let docs = YamlLoader::load_from_str(&plugin_default).map_err(|e| format!("Generate RSU config failed: {:?}", e))?;
    let doc = &docs[0];
    let mut writer = String::new();
    let mut emitter = YamlEmitter::new(&mut writer);
    emitter.dump(doc).map_err(|e| format!("Generate RSU config, dump str failed: {:?}", e))?;
    fs::write(&cfg_path, writer).map_err(|e| format!("Generate RSU config, write failed: {:?}", e))?;
    Ok(())
}
