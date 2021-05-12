use std::error::Error;
use std::env;
use std::fs;
use std::path::Path;
use log::{info, error};
extern crate yaml_rust;
use yaml_rust::{YamlLoader, YamlEmitter};
use tokio;
mod server;
use server::{PM, server, send};
mod plugin;

fn read_rsu_cfg(path: &str) -> Result<(String, String, u64), Box<dyn Error>>{
    if !Path::new(path).exists() {
        let mut dir_path_vec:Vec<&str> = path.split('/').collect();
        dir_path_vec.pop();
        fs::create_dir_all(dir_path_vec.join("/"))?;
        fs::File::create(path)?;
        generate_cfg(path)?;
    }
    let config_str = fs::read_to_string(path)?;
    let config_docs = YamlLoader::load_from_str(config_str.as_str())?;
    let config = &config_docs[0];
    let port = String::from(config["port"].as_str().ok_or("get port from cfg failed".to_owned())?);
    let ip = env::var("HOST_IP").unwrap_or("127.0.0.1".to_string());
    let center_db_url = String::from(config["center_db_url"].as_str().ok_or("get center_db_url from cfg failed".to_owned())?)
    .replace("127.0.0.1", &ip);
    let send_duration: u64 = config["report_duration"].as_i64().ok_or("get center_db_url from cfg failed".to_owned())? as u64;

    Ok((port, center_db_url, send_duration))
}


fn generate_cfg(cfg_path: &str)-> Result<(), Box<dyn Error>>{
    let rsu_default = r###"---
port: "61111"
center_db_url: ''
report_duration: 1"###;
    let docs = YamlLoader::load_from_str(&rsu_default)?;
    let doc = &docs[0];
    let mut writer = String::new();
    let mut emitter = YamlEmitter::new(&mut writer);
    emitter.dump(doc)?;
    fs::write(&cfg_path, writer)?;
    info!("Generate rsu default config successfully");
    Ok(())
}


#[tokio::main]
async fn main() {
    env_logger::init();
    
    let cfg_path = "./config/rsu.yaml";
    let (port, center_db_url, send_duration) = match read_rsu_cfg(&cfg_path){
        Ok((port, center_db_url, send_duration)) => (port, center_db_url, send_duration),
        Err(e) => {
            error!("start RSU failed, read config failed: {:?}", e);
            return
        }
    };

    if center_db_url == "" {
        error!("start RSU failed, center_db_url is empty");
        return
    }

    {
        let _pm = PM.lock().unwrap();
    }

    tokio::spawn(server(port));
    
    send(center_db_url, send_duration).await;
}
