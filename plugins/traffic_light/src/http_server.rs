use tide::{Request, Response};
use tide::utils::{After};
use percent_encoding::{percent_decode};
use serde_json::{Value};
use serde::{Deserialize, Serialize};
use crate::light;
use light::{LightColor};

#[derive(Deserialize, Serialize)]
struct ResponseData {
    status: i32,
    message: String,
}

pub async fn serve_http(port: String) -> tide::Result<()> {
    tide::log::start();
    let mut app = tide::new();

    app.at("/").get(|_| async { Ok("OK") });
    
    app.with(After(|mut res: Response| async move {
        res.insert_header("Access-Control-Allow-Origin", "*");
        Ok(res)
    }));

    app.at("/rule_change").post(|mut req: Request<()>| async move {
        let req_mess = req.body_string().await?;
        let req_mess_decoded = percent_decode(req_mess.as_bytes()).decode_utf8()?;
        let req_mess_obj: Value = serde_json::from_str(&req_mess_decoded)?;
        let remain = req_mess_obj["remain"].as_u64().unwrap();
        let color = req_mess_obj["color"].as_u64().unwrap();
        let lgt_id = req_mess_obj["light_id"].as_str().unwrap();
        println!("rule cheange, message: light_id: {}, color: {}, remain: {}", lgt_id, color, remain);

        let init_color = match color {
            1 => LightColor::RED,
            2 => LightColor::GREEN,
            3 => LightColor::YELLOW,
            0 => LightColor::UNKNOWN,
            _ => LightColor::UNKNOWN,
        };

        light::init_light_duration(color as i32, remain as i64);
        light::init_lgt_status(&lgt_id, init_color, remain as i64);

        let body_data =  ResponseData {status: 1, message: String::from("")};
        let response = Response::builder(200)
         .body(serde_json::json!(&body_data))
         .build();
        Ok(response)
    });

    println!("start traffic light server OK ......");
    app.listen(format!("0.0.0.0:{}", port)).await?;
    Ok(())
}

