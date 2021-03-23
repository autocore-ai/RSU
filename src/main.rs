use std::fs;
use std::env;
extern crate yaml_rust;
use yaml_rust::{YamlLoader};
use tokio;
mod server;
use server::{PM, server, send};
mod plugin;

fn read_rsu_cfg(path: &str) -> (String, String, u64){
    let config_str = fs::read_to_string(path).unwrap();
    let config_docs = YamlLoader::load_from_str(config_str.as_str()).unwrap();
    let config = &config_docs[0];
    let port = String::from(config["port"].as_str().unwrap());
    let cv_zenoh_url = String::from(config["cv_zenoh_url"].as_str().unwrap());
    let send_duration: u64 = config["report_duration"].as_i64().unwrap() as u64;

    (port, cv_zenoh_url, send_duration)
}


#[tokio::main]
async fn main() {
    let current_path = env::current_exe().unwrap();
    let path_list = current_path.to_str().unwrap().split("target").collect::<Vec<_>>();
    let cfg_path = format!("{}/config/rsu.yaml", path_list[0]);
    
    let (port, cv_zenoh_url, send_duration) = read_rsu_cfg(&cfg_path);

    {
        let _pm = PM.lock().unwrap();
    }
    tokio::spawn(server(port));
    
    send(cv_zenoh_url, send_duration).await;
}
