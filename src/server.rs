/// RSU
/// 1. 启动或停止插件
/// 2. 添加或删除插件
/// 3. 上报插件状态，1s 一次

use std::env;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use tide::{Request, Response};
use tide::prelude::*;
use tide::utils::{After};
use serde_json::{Result, Value};
use percent_encoding::{percent_decode};
use tokio::time::Instant;
use reqwest;
extern crate lazy_static;
use lazy_static::lazy_static;
use crate::plugin;
use plugin::{PluginMgr};


lazy_static! {
    pub static ref PM: Arc<Mutex<Result<PluginMgr>>> = {
        let current_path = env::current_exe().unwrap();
        let path_list = current_path.to_str().unwrap().split("target").collect::<Vec<_>>();
        let cfg_path = format!("{}/config/plugins.yaml", path_list[0]);
        let pm = PluginMgr::new(&cfg_path).unwrap();
        Arc::new(Mutex::new(Ok(pm)))
    };
}

pub async fn server(port: String) -> tide::Result<()> {

    tide::log::start();
    let mut app = tide::new();

    app.at("/").get(|_| async { Ok("RSU OK") });

    app.with(After(|mut res: Response| async move {
        res.insert_header("Access-Control-Allow-Origin", "*");
        Ok(res)
    }));

    app.at("/plugin").post(|mut req: Request<()>| async move {
        let plugin = req.body_string().await?;
        let plugin_decoded = percent_decode(plugin.as_bytes()).decode_utf8()?;
        let plugin_obj: Value = serde_json::from_str(&plugin_decoded)?;
        
        if !plugin_obj.is_object() {
            return Ok(json!({ "status": -1, "message": "params are wrong, ex: {\"name\": \"traffic_light\", \"active\": true}"}))
        }

        let name = match plugin_obj.get("name") {
            Some(na) => na.as_str().unwrap(),
            None => return Ok(json!({ "status": -1, "message": "need param: name"}))
        };

        let active = match plugin_obj.get("active") {
            Some(ac) => ac.as_bool().unwrap(),
            None => return Ok(json!({ "status": -1, "message": "need param: active"}))
        };
        let mut pm_locked = PM.lock().unwrap();
        let pm = pm_locked.as_mut().unwrap();
        if active {
            match pm.start_plugin(&name){
                Ok(_) => return Ok(json!({ "status": 1, "message": format!("start plugin {} successful", name)})),
                Err(message) => return Ok(json!({ "status": 1, "message": message})),
            };
        } else {
            match pm.stop_plugin(&name) {
                Ok(_) => return Ok(json!({ "status": 1, "message": format!("stop plugin {} successful", name)})),
                Err(message) => return Ok(json!({ "status": 1, "message": message})),
            };
        }
    });
    
    app.at("/plugin/remove").post(|mut req: Request<()>| async move {
        let plugin = req.body_string().await?;
        let plugin_decoded = percent_decode(plugin.as_bytes()).decode_utf8()?;
        let plugin_obj: Value = serde_json::from_str(&plugin_decoded)?;
        println!("{:?}", plugin_obj);
        if !plugin_obj.is_object() {
            return Ok(json!({ "status": -1, "message": "params are wrong, ex: {\"name\": \"traffic_light\"}"}))
        }

        let name = match plugin_obj.get("name") {
            Some(na) => na.as_str().unwrap(),
            None => return Ok(json!({ "status": -1, "message": "need param: name"}))
        };
        
        let mut pm_locked = PM.lock().unwrap();
        let pm = pm_locked.as_mut().unwrap();
        match pm.remove_plugin(&name) {
            Ok(_) => return Ok(json!({ "status": 1, "message": format!("remove plugin {} successful", name)})),
            Err(e) => return Ok(json!({ "status": 1, "message": format!("remove plugin {} failed, error: {:?}", name, e)})),
        }
        
    });

    app.at("/plugin/add").post(|mut req: Request<()>| async move {
        let plugin = req.body_string().await?;
        let plugin_decoded = percent_decode(plugin.as_bytes()).decode_utf8()?;
        let plugin_obj: Value = serde_json::from_str(&plugin_decoded)?;

        if !plugin_obj.is_object() {
            return Ok(json!({ "status": -1, "message": "params are wrong, ex: {\"name\": \"traffic_light\", \"path\": \"/home/traffic_light\", \"active\": true}"}))
        }

        let name = match plugin_obj.get("name") {
            Some(na) => na.as_str().unwrap(),
            None => return Ok(json!({ "status": -1, "message": "need param: name"}))
        };
        
        let path = match plugin_obj.get("path") {
            Some(pa) => pa.as_str().unwrap(),
            None => return Ok(json!({ "status": -1, "message": "need param: path"}))
        };
        
        let active = match plugin_obj.get("active") {
            Some(ac) => ac.as_bool().unwrap(),
            None => return Ok(json!({ "status": -1, "message": "need param: active"}))
        };

        let mut pm_locked = PM.lock().unwrap();
        let pm = pm_locked.as_mut().unwrap();
        match pm.add_plugin(&name, &path, active) {
            Ok(_) => Ok(json!({ "status": 1, "message": format!("add plugin {} successful", name)})),
            Err(e) => return Ok(json!({ "status": 1, "message": format!("add plugin {} failed, error: {:?}", name, e)})),
        }
        
    });
    println!("start PM server ......");
    app.listen(format!("0.0.0.0:{}", port)).await?;
    Ok(())
}


// 1s发送一次红绿灯结果
pub async fn send(cv_zenoh_url: String, duration: u64) {

    loop {
        let now = Instant::now();
        {
            let mut pm_locked = PM.lock().unwrap();
            let pm = pm_locked.as_mut().unwrap();
            let plugin_cfg = serde_json::to_string(&pm.plugin_cfg).unwrap();

            let res = reqwest::Client::new()
                .put(&cv_zenoh_url)
                .json(&serde_json::json!(plugin_cfg))
                .send()
                .await.unwrap();

            if res.status() != 200 {
                println!("send CV plugins status failed, {:?}", res)
            };
        }
        tokio::time::sleep_until(now.checked_add(Duration::from_secs(duration)).unwrap()).await;
    }
    
    
}
