use log::{info, error, debug};
use std::sync::Arc;
use std::sync::Mutex;
use async_std::task;
use std::time;
use tokio;
use tokio::runtime::Runtime;
mod config;
use config::read_config;
mod light;
mod http_server;


async fn plugin_main(error_flag: Arc<Mutex<bool>>) -> Result<i32, String> {
    let cfg_path = "./config/plugins/traffic_light.yaml";

    let (road_id, center_db_url, port) = match read_config(&cfg_path){
        Ok((road_id, center_db_url, port)) => (road_id, center_db_url, port),
        Err(e) => {
            {
                let mut error_flag = error_flag.lock().map_err(|e| {
                    error!("lock traffic light error_flag failed: {:?}", e);
                    format!("lock traffic light error_flag failed: {:?}", e)
                })?;
                *error_flag = true;
            }
            error!("read traffic light config failed: {:?}", e.to_string());
            return Err(format!("read traffic light config failed: {:?}", e.to_string()))
        },
    };
    
    let error_flag_clone = Arc::clone(&error_flag);
    tokio::spawn(http_server::serve_http(port, error_flag_clone));

    match light::light_loop(road_id, center_db_url).await {
        Ok(_) => {
            info!("traffic light is looping...");
        },
        Err(e) => {
            {
                let mut error_flag = error_flag.lock().map_err(|e| {
                    error!("lock traffic light error_flag failed: {:?}", e);
                    format!("lock traffic light error_flag failed: {:?}", e)
                })?;
                *error_flag = true;
            }
            error!("traffic light loop failed: {:?}", e.to_string());

            return Err(format!("traffic light loop light failed: {:?}", e.to_string()))
        }
    };

    Ok(0)
}

async fn async_run(running_flag: Arc<Mutex<bool>>, error_flag: Arc<Mutex<bool>>) -> i32 {
    tokio::spawn(plugin_main(error_flag));
    
    loop {
        task::sleep(time::Duration::from_secs(1)).await;
        let the_flag = match running_flag.lock(){
            Ok(f) => f,
            Err(e) => {
                debug!("lock running_flag failed: {:?}", e);
                return -1
            },
        };

        if !*the_flag {
            info!("plugin traffic light stopped");
            return 0
            // break;
        }
    }
}

#[no_mangle]
pub extern "C" fn run(running_flag: Arc<Mutex<bool>>, error_flag: Arc<Mutex<bool>>) -> i32 {
    let _ = env_logger::try_init().map_err(|e| {
        error!("traffic light init env log failed: {:?}", e);
    });
    
    let rt = match Runtime::new() {
        Ok(r) => r,
        Err(e) => {
            debug!("traffic light new runtime failed: {:?}", e);
            return -1
        },
    };

    rt.block_on(async_run(running_flag, error_flag))
}
