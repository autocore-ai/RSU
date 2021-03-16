# 需求分析

## 平台功能

### 远程管理

1. 远程使能 -- CloudViewer可以通过HTTP请求，控制本平台的插件状态
2. 配置更新 -- CloudViewer请求后，更新配置中插件状态
3. 状态上报 -- 上报插件状态给CloudViewer
   
### 插件管理

1. 配置读取 -- 启动平台时，根据插件配置，决定启动那些插件
2. 插件使能 -- 拥有启停某个插件的功能
 
## API

URL： ip:port/plugin
   
描述：  接口根据请求中的参数，开启或关闭插件，并将插件状态更新到配置文件中

请求类型： POST

例子： curl ip:port/plugin -d '{"name":"traffic_light", "active": true}'

请求内容：
|  字段   | 是否必须  | 类型  | 描述  |
|  ----  | ----  | ----  | ----  |
| name        | 是| string | 插件名字 |
| active      | 是|  i32    | 插件状态，1开 0 关|


响应消息：
|  字段    | 类型    | 描述  |
|  ----   | ----    | ----  |
| status  | i32     | 插件装填，1 成功，-1 失败 |
| message | string  | 信息描述|


URL： ip:port/plugin/add
   
描述：  添加插件

请求类型： POST

例子： curl ip:port/plugin/add -d '{"name":"traffic_light", "path":"/home/duan/RSU/plugins/traffic_light/target/debug/libtraffic_light.so", "active": true}'

请求内容：
|  字段   | 是否必须  | 类型  | 描述  |
|  ----  | ----  | ----  | ----  |
| name        | 是| string | 插件名字 |
| path      | 是|  string    | 插件路径|


响应消息：
|  字段    | 类型    | 描述  |
|  ----   | ----    | ----  |
| status  | i32     | 插件装填，1 成功，-1 失败 |
| message | string  | 信息描述|


URL： ip:port/plugin/remove
   
描述：  删除插件

请求类型： POST

例子： curl ip:port/plugin/remove -d '{"name":"traffic_light"}'

请求内容：
|  字段   | 是否必须  | 类型  | 描述  |
|  ----  | ----  | ----  | ----  |
| name        | 是| string | 插件名字 |


响应消息：
|  字段    | 类型    | 描述  |
|  ----   | ----    | ----  |
| status  | i32     | 插件装填，1 成功，-1 失败 |
| message | string  | 信息描述|
