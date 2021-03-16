use std::fs;
extern crate yaml_rust;
use yaml_rust::{YamlLoader};
use tokio;
mod server;
use server::{PM, server, send};
mod plugin;

fn read_rsu_cfg(path: &str) -> (String, String, String, u64){
    let config_str = fs::read_to_string(path).unwrap();
    let config_docs = YamlLoader::load_from_str(config_str.as_str()).unwrap();
    let config = &config_docs[0];
    let port = String::from(config["port"].as_str().unwrap());
    let cv_zenoh_url = String::from(config["cv_zenoh_url"].as_str().unwrap());
    let road_id = String::from(config["road_id"].as_str().unwrap());
    let send_duration: u64 = config["report_duration"].as_i64().unwrap() as u64;

    (port, cv_zenoh_url, road_id, send_duration)
}


#[tokio::main]
async fn main() {
    let (port, cv_zenoh_url, road_id, send_duration) = read_rsu_cfg("./rsu.yaml");

    {
        let _pm = PM.lock().unwrap();
    }
    tokio::spawn(server(port));
    
    send(cv_zenoh_url, road_id, send_duration).await;
}