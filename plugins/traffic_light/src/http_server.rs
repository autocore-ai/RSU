use log::{info, debug, error};
use tide::{Request, Response};
use tide::utils::{After};
use std::sync::Arc;
use std::sync::Mutex;
use percent_encoding::{percent_decode};
use serde_json::{Value, json};
use serde::{Deserialize, Serialize};
use crate::light;
use light::{LightColor};

#[derive(Deserialize, Serialize)]
struct ResponseData {
    status: i32,
    message: String,
}

pub async fn serve_http(port: String, error_flag: Arc<Mutex<bool>>) -> Result<(), String> {
    let mut app = tide::new();

    app.at("/").get(|_| async { Ok("Traffic Light OK") });
    
    app.with(After(|mut res: Response| async move {
        res.insert_header("Access-Control-Allow-Origin", "*");
        Ok(res)
    }));

    app.at("/rule_change").post(|mut req: Request<()>| async move {
        let req_mess = match req.body_string().await{
            Ok(p) => p,
            Err(e) => return Ok(json!({ "status": -1, "message": format!("{:?}", e)}))
        };

        let req_mess_decoded = match percent_decode(req_mess.as_bytes()).decode_utf8(){
            Ok(v) => v,
            Err(e) => return Ok(json!({ "status": -1, "message": format!("param decode error: {:?}", e)}))
        };

        let req_mess_obj: Value = match serde_json::from_str(&req_mess_decoded) {
            Ok(v) => v,
            Err(e) => return Ok(json!({ "status": -1, "message": format!("param parse into json error: {:?}", e)}))
        };

        let remain = match req_mess_obj["remain"].as_u64() {
            Some(v) => v,
            None => { return Ok(json!({ "status": -1, "message": format!("get param error, remain is None")}))}
        };

        let color = match req_mess_obj["color"].as_u64() {
            Some(v) => v,
            None => { return Ok(json!({ "status": -1, "message": format!("get param error, color is None")}))}
        };

        let lgt_id = match req_mess_obj["lgt_id"].as_str() {
            Some(v) => v,
            None => { return Ok(json!({ "status": -1, "message": format!("get param error, lgt_id is None")}))}
        };
        debug!("rule cheange, message: light_id: {}, color: {}, remain: {}", lgt_id, color, remain);

        let init_color = match color {
            1 => LightColor::RED,
            2 => LightColor::GREEN,
            3 => LightColor::YELLOW,
            0 => LightColor::UNKNOWN,
            _ => LightColor::UNKNOWN,
        };

        match light::init_light_duration(color as i32, remain as i64){
            Ok(_) => {
                debug!("[rule change] init light duration successful");
            },
            Err(e) => { 
                return Ok(json!({ "status": -1, "message": format!("init light duration error: {:?}", e)}))
            },
        };
        match light::init_lgt_status(&lgt_id, init_color, remain as i64) {
            Ok(_) => {
                debug!("[rule change] init light status successful");
            },
            Err(e) => { 
                return Ok(json!({ "status": -1, "message": format!("init llight status error: {:?}", e)}))
            },
        };

        Ok(json!({ "status": 1, "message": String::from("change traffic light successful")}))
    });

    
    match app.listen(format!("0.0.0.0:{}", port)).await {
        Ok(_) => {
            info!("start traffic light server OK ......");
        },
        Err(e) => {
            error!("start traffic light error: {:?}", e);
            let mut error_flag = error_flag.lock().map_err(|e| {
                error!("lock traffic light error_flag failed: {:?}", e);
                format!("lock traffic light error_flag failed: {:?}", e)
            })?;

            *error_flag = true;
        }

    };
    Ok(())
}
