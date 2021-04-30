///
/// reader cfg
/// 
use std::error::Error;
use log::{debug};
use std::fs;
use std::env;
use std::path::Path;
extern crate yaml_rust;
use yaml_rust::{YamlLoader, YamlEmitter};
use crate::light;
use light::{LightColor, LightStatus, LIGHTDURATION, LIGHTGROUP, LIGHTSTATUS};


fn generate_cfg(cfg_path: &str) -> Result<(), Box<dyn Error>>{
    let rsu_default = r#"---
port: '8081'
road_id: "1111111"  # 红绿灯路口的ID，和地图中的路口对应
light_id_group: {   # 红绿灯ID，和地图中的路口灯对应
                group1: ["light_1", "light_2"],
                group2: ["light_3", "light_4"],
                }

master: "group1"  # 启动服务时的依照计算的灯组

color: 1  #  1 红 2 绿 3 黄 0 灭灯

duration:
    green: 7
    yellow: 3
    red: 10
    unknown: -1

center_db_url: 'http://IP:PORT/rsu/rsu_id/traffic_light/status/'
"#;
    let docs = YamlLoader::load_from_str(&rsu_default)?;
        let doc = &docs[0];
        let mut writer = String::new();
        let mut emitter = YamlEmitter::new(&mut writer);
        emitter.dump(doc)?;
        fs::write(&cfg_path, writer)?;
        Ok(())
}

pub fn read_config(file_name: &str) -> Result<(String, String, String), Box<dyn Error>> {
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
    let light_group_cfg = &config["light_id_group"];
    let road_id =  String::from(config["road_id"].as_str().ok_or("get road_id from traffic light config failed".to_owned())?);
    let ip = env::var("HOST_IP").unwrap_or("127.0.0.1".to_string());
    let center_db_url =  String::from(config["center_db_url"].as_str().ok_or("get center_db_url from raffic light config tfailed".to_owned())?)
    .replace("127.0.0.1", &ip);
    let port = String::from(config["port"].as_str().ok_or("get port from traffic light config tfailed".to_owned())?);

    // 读取灯的变化时间
    {
        let mut light_duration = LIGHTDURATION.lock()?;
        light_duration.green = config["duration"]["green"].as_i64().ok_or("get duration-green from traffic light config tfailed".to_owned())?;
        light_duration.red = config["duration"]["red"].as_i64().ok_or("get duration-red from traffic light config tfailed".to_owned())?;
        light_duration.yellow = config["duration"]["yellow"].as_i64().ok_or("get duration-yellow from traffic light config tfailed".to_owned())?;
        light_duration.unknown = config["duration"]["unknown"].as_i64().ok_or("get duration-unkown from traffic light fconfig tailed".to_owned())?;
    }
    
    // 读取配置中的红绿灯颜色
    let default_color:LightColor;
    match config["color"].as_i64().ok_or("get color from traffic light config tfailed".to_owned())? {
        1 => default_color = LightColor::RED,
        2 => default_color = LightColor::GREEN,
        3 => default_color = LightColor::YELLOW,
        0 => default_color = LightColor::UNKNOWN,
        _ => default_color = LightColor::UNKNOWN,
    }
    let init_duration = light::get_duration(&default_color)?;

    // 红绿灯组
    let group_master = config["master"].as_str().ok_or("get master from traffic light config tfailed".to_owned())?;
    {
        let mut light_group = LIGHTGROUP.lock()?;
        let mut lgt_status_group_hash = LIGHTSTATUS.lock()?;

        // 读取配置中的红绿灯组
        for (group_name, lgt_id_list) in light_group_cfg.as_hash().ok_or("get light_group_cfg from traffic light config tfailed".to_owned())?.into_iter() {
            let group_name = String::from(group_name.as_str().ok_or("get group_name from traffic light config tfailed".to_owned())?);
            let mut g_id_list = vec![];
            for lgt_id in lgt_id_list.as_vec().ok_or("get lgt_id_list from traffic light config tfailed".to_owned())? {
                g_id_list.push(String::from(lgt_id.as_str().ok_or("get lgt_id from traffic light config tfailed".to_owned())?));
            }
            light_group.insert(group_name.clone(), g_id_list);

            // 初始化LIGHTSTATUS
            if group_name == group_master {
                lgt_status_group_hash.insert(group_name, LightStatus{color: default_color, counter: init_duration});
            } else {
                let in_color = light::inverse_color(&default_color, init_duration)?;
                let in_duration = light::get_duration(&in_color)?;
                lgt_status_group_hash.insert(group_name, LightStatus{color: in_color, counter: in_duration});
            }
        }

    }
    
    debug!("read traffic light config ok");
    Ok((road_id, center_db_url, port))
}
