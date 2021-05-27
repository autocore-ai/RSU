use futures::prelude::*;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::error::Error;
use std::path::Path;
use std::time::Duration;
use reqwest;
use serde::{Deserialize, Serialize};
use zenoh::net::*;
use zenoh::Properties;
use yaml_rust::{YamlLoader, YamlEmitter};
use async_std::task;
use std::time;
use log::{info, error, debug};
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
    pub fn new(buf_vec: &Vec<u8>) -> Result<CurrentPose, Box<dyn Error>> {
        let frame_id_len: i32 = bincode::deserialize(&buf_vec[12..16])?;
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
     
        Ok(
            CurrentPose {
                position: 
                    Position{
                        x: bincode::deserialize(&buf_vec[position_start..position_x_end])?, 
                        y: bincode::deserialize(&buf_vec[position_x_end..position_y_end])?, 
                        z: bincode::deserialize(&buf_vec[position_y_end..position_z_end])?,
                    }, 
                orientation:Orientation{
                    x: bincode::deserialize(&buf_vec[position_z_end..ori_x_end])?, 
                    y: bincode::deserialize(&buf_vec[ori_x_end..ori_y_end])?, 
                    z: bincode::deserialize(&buf_vec[ori_y_end..ori_z_end])?, 
                    w: bincode::deserialize(&buf_vec[ori_z_end..ori_w_end])?}
            }
        )
    }

}

lazy_static! {
    static ref VEHICLESTATUSMAP:Mutex<HashMap<String, CurrentPose>> = {
        Mutex::new(HashMap::new())
    };
}


fn read_config(file_name: &str) -> Result<(String, String, u64), Box<dyn Error>> {
    if !Path::new(file_name).exists() {
        let mut dir_path_vec:Vec<&str> = file_name.split('/').collect();
        dir_path_vec.pop();
        fs::create_dir_all(dir_path_vec.join("/"))?;
        fs::File::create(file_name)?;
        generate_cfg(file_name)?;
    }
    let config_str = fs::read_to_string(file_name)?;
    let config_docs = YamlLoader::load_from_str(config_str.as_str())?;
    let config = &config_docs[0];
    let vh_zenoh_path =  String::from(config["vehicle_status_zenoh_path"].as_str()
    .ok_or("get vehicle_status_zenoh_path from vehicle status config failed".to_owned())?);
    let ip = env::var("HOST_IP").unwrap_or("127.0.0.1".to_string());
    let center_db_url =  String::from(config["center_db_url"].as_str()
    .ok_or("get center_db_url from vehicle status config failed".to_owned())?)
    .replace("127.0.0.1", &ip);
    let interval = config["interval"].as_i64()
    .ok_or("get interval from vehicle status config failed".to_owned())?;
    Ok((vh_zenoh_path, center_db_url, interval as u64))
}


fn generate_cfg(cfg_path: &str) -> Result<(), Box<dyn Error>>{
    let rsu_default = r#"---
    vehicle_status_zenoh_path: '/demo/dds/rt/current_pose'
    center_db_url: 'http://ip:port/rsu/rsu_id/vehicle/status/'
    interval: 1000"#;
    let docs = YamlLoader::load_from_str(&rsu_default)?;
        let doc = &docs[0];
        let mut writer = String::new();
        let mut emitter = YamlEmitter::new(&mut writer);
        emitter.dump(doc)?;
        fs::write(&cfg_path, writer)?;
        Ok(())
}

async fn send(center_db_url: String, interval: u64) -> Result<(), Box<dyn Error>>{
    loop {
        let now = Instant::now();
        let mut vh_status_vec: Vec<CurrentPose> = vec![];

        {
            let mut vh_status_map = VEHICLESTATUSMAP.lock()?;
            for (_, vh_status) in vh_status_map.iter_mut() {
                vh_status_vec.push(vh_status.clone());
            }
            vh_status_map.clear();
        }

        match reqwest::Client::new()
            .put(&center_db_url)
            .json(&serde_json::json!(vh_status_vec))
            .send()
            .await {
                Ok(res) => {
                    if res.status() != 200 {
                        error!("send vehicle status to center db failed, url:{}, reason {:?}", center_db_url, res);
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        continue;
                    }},
                Err(e) => {
                    error!("send vehicle status to center db failed, url:{}, reason {:?}", center_db_url, e);
                }
            }
        
        tokio::time::sleep_until(now.checked_add(Duration::from_millis(interval))
        .ok_or(format!("vehicle status loop check time return None"))?).await;
    }
}

async fn receive_vh_status(vh_path: String, error_flag: Arc<Mutex<bool>>) -> Result<(), String>  {
    let config = Properties::default();
    // config.insert(String::from("mode"), String::from("client"));
    debug!("Opening session...");
    let session = match open(config.into()).await{
        Ok(se) => se,
        Err(e) => {
            error!("get zenoh net session error: {:?}", e);

            let mut error_flag = error_flag.lock().map_err(|e| {
                error!("lock vehicle status error_flag failed: {:?}", e);
                format!("lock vehicle status error_flag failed: {:?}", e)
            })?;

            *error_flag = true;
            return Ok(())
        }
    };

    debug!("Declaring Subscriber on {}", vh_path);

    let sub_info = SubInfo {
        reliability: Reliability::Reliable,
        mode: SubMode::Push,
        period: None,
    };

    let mut subscriber = match session
        .declare_subscriber(&vh_path.into(), &sub_info)
        .await {
            Ok(sub) => sub,
            Err(e) => {
                error!("declare_subscriber error: {:?}", e);

                let mut error_flag = error_flag.lock().map_err(|e| {
                    error!("lock vehicle status error_flag failed: {:?}", e);
                    format!("lock vehicle status error_flag failed: {:?}", e)
                })?;
    
                *error_flag = true;
                return Ok(())
            },
        };

    let stream = subscriber.stream();
    let id:String = String::from("car_id");
    while let Some(d) = stream.next().await{
        let bs = d.payload.to_vec();
        {
            let vh_status: CurrentPose = match CurrentPose::new(&bs) {
                Ok(cp) => cp,
                Err(e) => {
                    error!("new CurrentPose failed: {:?}", e);
                    let mut error_flag = error_flag.lock().map_err(|e| {
                        error!("lock error_flag failed: {:?}", e);
                        format!("lock error_flag failed: {:?}", e)
                    })?;
        
                    *error_flag = true;
                    return Ok(())
                }
            };

            // debug!("receive vehicle state: {:?}", vh_status);
            // let vh_id = &vh_status.id;
            let mut vh_status_map = match VEHICLESTATUSMAP.lock(){
                Ok(map) => map,
                Err(e) => {
                    error!("lock vehicle status map failed: {:?}", e);
                    let mut error_flag = error_flag.lock().map_err(|e| {
                        error!("lock error_flag failed: {:?}", e);
                        format!("lock error_flag failed: {:?}", e)
                    })?;
        
                    *error_flag = true;
                    return Ok(())
                }
            };
            vh_status_map.insert(String::from(&id), vh_status);
        }
    }
    Ok(())
}


async fn plugin_main(error_flag: Arc<Mutex<bool>>) -> Result<(), String>{
    let cfg_path = "./config/plugins/vehicle_status.yaml";
    let (vh_zenoh_path, center_db_url, interval) = match read_config(&cfg_path){
        Ok((vh_zenoh_path, center_db_url, interval)) => (vh_zenoh_path, center_db_url, interval),
        Err(e) => {
            {
                let mut error_flag = error_flag.lock().map_err(|e| {
                    error!("lock vehicle status error_flag failed: {:?}", e);
                    format!("lock vehicle status error_flag failed: {:?}", e)
                })?;
                *error_flag = true;
            }
            error!("read vehicle status config failed: {:?}", e.to_string());
            return Err(format!("read vehicle status config failed: {:?}", e.to_string()))
        },
    };

    let error_flag_clone = Arc::clone(&error_flag);
    tokio::spawn(receive_vh_status(vh_zenoh_path, error_flag_clone));
    
    match send(center_db_url, interval).await{
        Ok(_) => {
            info!("vehicle status plugin server start successful");
        },
        Err(e) => {
            error!("vehicle status plugin server failed: {:?}", e);
            info!("sleep 2s ...");
            {
                let mut error_flag = error_flag.lock().map_err(|e| {
                    error!("lock vehicle status error_flag failed: {:?}", e);
                    format!("lock vehicle status error_flag failed: {:?}", e)
                })?;
                *error_flag = true;
            }
        }

    };
    Ok(())
}

async fn async_run(running_flag: Arc<Mutex<bool>>, error_flag: Arc<Mutex<bool>>) -> i32 {
    tokio::spawn(plugin_main(error_flag));

    loop {
        task::sleep(time::Duration::from_secs(1)).await;
        let the_flag = match running_flag.lock(){
            Ok(f) => f,
            Err(e) => {
                debug!("lock vehicle status running_flag failed: {:?}", e);
                return -1
            },
        };

        if !*the_flag {
            info!("plugin vehicle status stopped");
            return 0
        }
    }
}

#[no_mangle]
pub extern "C" fn run(running_flag: Arc<Mutex<bool>>, error_flag: Arc<Mutex<bool>>) -> i32 {
    let _ = env_logger::try_init().map_err(|e| {
        error!("vehicle status init env log failed: {:?}", e);
    });

    let rt = match Runtime::new() {
        Ok(r) => r,
        Err(e) => {
            debug!("vehicle status new runtime failed: {:?}", e);
            return -1
        },
    };

    rt.block_on(async_run(running_flag, error_flag))
}
