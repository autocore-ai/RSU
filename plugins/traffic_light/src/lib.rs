use std::sync::Arc;
use std::sync::Mutex;
use async_std::task;
use std::time;
use std::env;
use tokio;
use tokio::runtime::Runtime;
mod config;
use config::read_config;
mod light;
mod http_server;


async fn plugin_main() {
    let current_path = env::current_exe().unwrap();
    let path_list = current_path.to_str().unwrap().split("target").collect::<Vec<_>>();
    let cfg_path = format!("{}/config/plugins/traffic_light.yaml", path_list[0]);
    let (road_id, zenoh_url, port) = read_config(&cfg_path);
    
    tokio::spawn(http_server::serve_http(port));

    light::light_loop(road_id, zenoh_url).await;
    
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