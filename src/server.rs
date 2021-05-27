use log::{info, error, debug};
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
        let cfg_path = "./config/plugins.yaml";
        let pm = PluginMgr::new(&cfg_path).unwrap();
        Arc::new(Mutex::new(Ok(pm)))
    };
}

pub async fn server(port: String) -> tide::Result<()> {
    let mut app = tide::new();

    app.at("/").get(|_| async { Ok("RSU OK") });

    app.with(After(|mut res: Response| async move {
        res.insert_header("Access-Control-Allow-Origin", "*");
        Ok(res)
    }));

    app.at("/plugin").post(|mut req: Request<()>| async move {
        let plugin = match req.body_string().await {
            Ok(p) => p,
            Err(e) => return Ok(json!({ "status": -1, "message": format!("{:?}", e)}))
        };
        let plugin_decoded = percent_decode(plugin.as_bytes()).decode_utf8()?;
        let plugin_obj: Value = match serde_json::from_str(&plugin_decoded) {
            Ok(v) => v,
            Err(e) => return Ok(json!({ "status": -1, "message": format!("param parse into json wrong: {:?}", e)}))
        };
        info!("received update plugin message: {:?}", plugin_decoded);

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

        debug!("update plugin state ...");
        {
            let mut pm_locked = PM.lock().unwrap();
            let pm = pm_locked.as_mut().unwrap();
            if active {
                match pm.start_plugin(&name){
                    Ok(res) => return Ok(json!({ "status": 1, "message": res})),
                    Err(err) => return Ok(json!({ "status": -1, "message": err})),
                };
            } else {
                debug!("begin to stop plugin {}", name);
                match pm.stop_plugin(&name) {
                    Ok(res) => return Ok(json!({ "status": 1, "message": res})),
                    Err(err) => return Ok(json!({ "status": -1, "message": err})),
                };
            }
        }
        
    });
    
    app.at("/plugin/remove").post(|mut req: Request<()>| async move {
        let plugin = match req.body_string().await {
            Ok(p) => p,
            Err(e) => return Ok(json!({ "status": -1, "message": format!("{:?}", e)}))
        };
        let plugin_decoded = percent_decode(plugin.as_bytes()).decode_utf8()?;
        let plugin_obj: Value = match serde_json::from_str(&plugin_decoded) {
            Ok(v) => v,
            Err(e) => return Ok(json!({ "status": -1, "message": format!("param parse into json wrong: {:?}", e)}))
        };

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
        let plugin = match req.body_string().await {
            Ok(p) => p,
            Err(e) => return Ok(json!({ "status": -1, "message": format!("param parse into json wrong: {:?}", e)}))
        };
        let plugin_decoded = percent_decode(plugin.as_bytes()).decode_utf8()?;
        let plugin_obj: Value = match serde_json::from_str(&plugin_decoded) {
            Ok(v) => v,
            Err(e) => return Ok(json!({ "status": -1, "message": format!("{:?}", e)}))
        };

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
    info!("start RSU server ......");
    app.listen(format!("0.0.0.0:{}", port)).await?;
    Ok(())
}


// send plugin state
pub async fn send(center_db_url: String, duration: u64) {
    if center_db_url == "" {
        error!("center_db_url is empty ......");
        return
    }
    
    loop {
        let now = Instant::now();
        {
            let mut pm_locked = PM.lock().unwrap();
            let pm = pm_locked.as_mut().unwrap();
            match pm.check_plugin() {
                Ok(_) => {
                    debug!("plugins checked successfully");
                },
                Err(e) => {
                    error!("plugins checked failed: {:?}", e);
                },
            };
            
            match reqwest::Client::new()
            .put(&center_db_url)
            .json(&serde_json::json!(pm.plugin_cfg))
            .send()
            .await {
                Ok(res) => {
                    if res.status() != 200 {
                        error!("send plugins status to center db failed, url:{}, reason {:?}", center_db_url, res);
                    } else {
                        debug!("send plugin state successfully");
                    }
                },
                Err(e) => {
                    error!("send plugins status to center db failed, url:{}, reason {:?}", center_db_url, e);
                }
            };
        }
        tokio::time::sleep_until(now.checked_add(Duration::from_secs(duration)).unwrap()).await;
    }
    
}
