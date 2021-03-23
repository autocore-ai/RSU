use futures::prelude::*;
use futures::select;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::fs;
use reqwest;
use std::convert::{TryInto};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use zenoh::*;
use yaml_rust::{YamlLoader};
use async_std::task;
use std::time;
use std::env;
use tokio;
use tokio::runtime::Runtime;
use tokio::time::Instant;
extern crate lazy_static;
use lazy_static::lazy_static;


#[derive(Deserialize, Serialize)] 
#[derive(Debug, Clone)]
struct Position {
    x: f64,
    y: f64,
    z: f64,
}  

#[derive(Deserialize, Serialize)]
#[derive(Debug, Clone)]
struct Orientation {
    x: f64,
    y: f64,
    z: f64,
    w: f64,
} 

#[derive(Deserialize, Serialize)]
#[derive(Debug, Clone)]
struct CurrentPose {
    // id: String,
    position: Position,
    orientation: Orientation,
}


impl CurrentPose {
    pub fn new(buf_vec: &Vec<u8>) -> CurrentPose {
        let frame_id_len: i32 = bincode::deserialize(&buf_vec[12..16]).unwrap();
        println!("frame_id_len: {:?}", frame_id_len);
        let frame_str_len_min = frame_id_len + 4;
        let multiple:usize = {if frame_str_len_min % 8 == 0 {frame_str_len_min/ 8} else {frame_str_len_min/ 8 +1}} as usize;

        let position_start:usize = 12+multiple*8;
        let position_x_end:usize = position_start+8;
        let position_y_end:usize = position_x_end+8;
        let position_z_end:usize = position_y_end+8;
        let ori_x_end:usize = position_z_end+8;
        let ori_y_end:usize = ori_x_end+8;
        let ori_z_end:usize = ori_y_end+8;
        let ori_w_end:usize = ori_z_end+8;
     
        CurrentPose {
                    position: 
                        Position{
                            x: bincode::deserialize(&buf_vec[position_start..position_x_end]).unwrap(), 
                            y: bincode::deserialize(&buf_vec[position_x_end..position_y_end]).unwrap(), 
                            z: bincode::deserialize(&buf_vec[position_y_end..position_z_end]).unwrap(),
                        }, 
                    orientation:Orientation{
                        x: bincode::deserialize(&buf_vec[position_z_end..ori_x_end]).unwrap(), 
                        y: bincode::deserialize(&buf_vec[ori_x_end..ori_y_end]).unwrap(), 
                        z: bincode::deserialize(&buf_vec[ori_y_end..ori_z_end]).unwrap(), 
                        w: bincode::deserialize(&buf_vec[ori_z_end..ori_w_end]).unwrap()}
                }
    }

}

lazy_static! {
    static ref VEHICLESTATUSMAP:Mutex<HashMap<String, CurrentPose>> = {
        Mutex::new(HashMap::new())
    };
}


fn read_config(file_name: &str) -> (String, String, u64) {
    println!("begin to read_config");
    let config_str = fs::read_to_string(file_name).unwrap();
    let config_docs = YamlLoader::load_from_str(config_str.as_str()).unwrap();
    let config = &config_docs[0];
    let vh_zenoh_path =  String::from(config["vehicle_status_zenoh_path"].as_str().unwrap());
    let cv_zenoh_url =  String::from(config["cv_zenoh_url"].as_str().unwrap());
    let interval = config["interval"].as_i64().unwrap();
    (vh_zenoh_path, cv_zenoh_url, interval as u64)
}

async fn send(cv_url: String, interval: u64){
    loop {
        let now = Instant::now();
        let mut vh_status_vec: Vec<CurrentPose> = vec![];

        {
            let mut vh_status_map = VEHICLESTATUSMAP.lock().unwrap();
            for (_, vh_status) in vh_status_map.iter_mut() {
                vh_status_vec.push(vh_status.clone());
            }
            vh_status_map.clear();
        }

            let vh_str = serde_json::to_string(&vh_status_vec).unwrap();

            let res = reqwest::Client::new()
                .put(&cv_url)
                .json(&serde_json::json!(vh_str))
                .send()
                .await.unwrap();


            if res.status() != 200 {
                println!("send CV vehicle status failed, {:?}", res)
            };
        
        tokio::time::sleep_until(now.checked_add(Duration::from_millis(interval)).unwrap()).await;
    }
}

async fn receive_vh_status(vh_path: String) {
    // env_logger::init();
    let mut config = Properties::default();
    config.insert(String::from("mode"), String::from("client"));
    let zenoh = Zenoh::new(config.into()).await.unwrap();

    println!("New workspace...");
    let workspace = zenoh.workspace(None).await.unwrap();

    println!("Subscribe to {} ...\n", vh_path);
    let mut change_stream = workspace
        .subscribe(&vh_path.try_into().unwrap())
        .await
        .unwrap();
    let vh_id: String = String::from("hello");
    loop {
        select!(
            change = change_stream.next().fuse() => {
                let change = change.unwrap();
               
                {
                    let vh_status: CurrentPose = serde_json::from_str(&change.value.unwrap().encode_to_string().2).unwrap();
                    // let vh_id = &vh_status.id;
                    let mut vh_status_map = VEHICLESTATUSMAP.lock().unwrap();
                    vh_status_map.insert(String::from(&vh_id), vh_status);
                }
            }

        );
    }

}


async fn plugin_main() {
    let current_path = env::current_exe().unwrap();
    let path_list = current_path.to_str().unwrap().split("target").collect::<Vec<_>>();
    let cfg_path = format!("{}/config/plugins/vehicle_status.yaml", path_list[0]);
    let (vh_zenoh_path, cv_zenoh_url, interval) = read_config(&cfg_path);

    tokio::spawn(receive_vh_status(vh_zenoh_path));
    
    send(cv_zenoh_url, interval).await;
    
}

async fn async_run(running_flag: Arc<Mutex<bool>>) {
    tokio::spawn(plugin_main());
    loop {
        task::sleep(time::Duration::from_secs(1)).await;
        let the_flag = running_flag.lock().unwrap();
        if !*the_flag {
            break;
        }
    }
}

#[no_mangle]
pub extern "C" fn run(running_flag: Arc<Mutex<bool>>) -> i32 {
    let rt = Runtime::new().unwrap();
    rt.block_on(async_run(running_flag));
    0
}
