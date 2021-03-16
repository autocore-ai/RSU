use std::thread;
use std::thread::JoinHandle;
use libloading::Library;
use std::sync::Arc;
use std::sync::Mutex;
use std::collections::HashMap;
use std::fs;
extern crate yaml_rust;
use yaml_rust::{YamlLoader, YamlEmitter, Yaml};
use linked_hash_map::LinkedHashMap;
use serde::{Deserialize, Serialize};


type PluginFunc = unsafe extern fn(running_flag: Arc<Mutex<bool>>) -> i32;

#[derive(Debug)]
pub struct Plugin {
    lib_handle: Arc<Mutex<Library>>,
    thread_handle: Option<JoinHandle<Option<i32>>>,
    running_flag: Arc<Mutex<bool>>
}

impl Plugin {
    pub fn new(path: &str) -> Result<Plugin, String> {
        unsafe {
            let lib = Library::new(path).unwrap();
            let lib_handle = Arc::new(Mutex::new(lib));
            Ok(Plugin {
                lib_handle,
                running_flag: Arc::new(Mutex::new(true)),
                thread_handle: None,
            })
        }
        
    }

    fn start(&mut self) {
        let lib = Arc::clone(&self.lib_handle);
        let flag = Arc::clone(&self.running_flag);
        let join_handle = thread::spawn(move || -> Option<i32> {
            unsafe {
                let h = lib.lock().ok()?;
                let func = h.get::<PluginFunc>(b"run").ok()?;
                Some(func(flag))
            }
        });
        self.thread_handle = Some(join_handle);
    }

    fn stop(&mut self) -> Option<i32> {
        {
            let mut the_flag = self.running_flag.lock().ok()?;
            *the_flag = false;
        }
        let result = self.thread_handle.take()?.join();
        result.ok()?
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
    // read config and start plugin
    pub fn new(path: &str) -> Result<PluginMgr, String> {
        let mut obj = PluginMgr {config_path: String::from(path), plugin_cfg: HashMap::new(), plugins: HashMap::new()};
        let config_str = fs::read_to_string(path).unwrap();
        let config_docs = YamlLoader::load_from_str(config_str.as_str()).unwrap();
        let config = &config_docs[0];
        let plugin_cfg = &config["plugins"];
        for (name, info) in plugin_cfg.as_hash().unwrap().into_iter() {
            let name = name.as_str().unwrap();
            let path = info["path"].as_str().unwrap();
            let active = info["active"].as_bool().unwrap();
            obj.add_plugin_inner(name, path, active)?;
            println!("plugin added, name:{:?}, path: {}, actice: {}", name, path, active);
        }
        Ok(obj)
    }

    fn start_plugin_inner(&mut self, name: &str) -> Result<(), String> {
        if self.plugins.contains_key(name) {
            println!("plugin[{}] is already running", name);
            return Ok(());
        }
        let mut plugin_info = self.plugin_cfg.get_mut(name).unwrap();
        plugin_info.active = true;
        let mut plugin = Plugin::new(&plugin_info.path[..])?;
        plugin.start();
        self.plugins.insert(String::from(name), plugin);
        println!("plugin[{}] is running", name);
        Ok(())
    }

    pub fn start_plugin(&mut self, name: &str) -> Result<(), String> {
        self.start_plugin_inner(name)?;
        self.flush_cfg_to_file();
        Ok(())
    }

    pub fn stop_plugin(&mut self, name: &str) -> Result<i32, String> {
        if !self.plugins.contains_key(name) {
            println!("plugin[{}] has been stopped", name);
            return Ok(0)
        }
        let ret = self.plugins.get_mut(name).unwrap().stop();
        self.plugins.remove(name);
        let mut plugin_info = self.plugin_cfg.get_mut(name).unwrap();
        plugin_info.active = false;
        self.flush_cfg_to_file();
        println!("plugin[{}] has stopped", name);
        Ok(ret.unwrap())
    }

    fn add_plugin_inner(&mut self, name: &str, path: &str, active: bool) -> Result<(), String> {
        if self.plugin_cfg.contains_key(name) {
            return Ok(());
        }
        self.plugin_cfg.insert(String::from(name), PluginInfo {path: String::from(path), active});
        if active {
            self.start_plugin_inner(name)?;
        }
        Ok(())
    }

    pub fn add_plugin(&mut self, name: &str, path: &str, active: bool) -> Result<(), String> {
        self.add_plugin_inner(name, path, active)?;
        self.flush_cfg_to_file();
        println!("plugin[{}] has been added", name);
        Ok(())
    }

    pub fn remove_plugin(&mut self, name: &str) -> Result<(), String> {
        if self.plugins.contains_key(name) {
            self.stop_plugin(name)?;
        }
        self.plugin_cfg.remove(name);
        self.flush_cfg_to_file();
        println!("plugin[{}] has been removed", name);
        Ok(())
    }

    fn flush_cfg_to_file(&mut self) -> Option<()> {
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
        emitter.dump(&root_node).unwrap(); // dump the YAML object to a String
        let _ = fs::write(&self.config_path[..], out_str);
        Some(())
    }
}



