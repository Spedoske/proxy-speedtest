use jsonrpsee::{server::Server, types::ErrorObject, RpcModule};
use regex::Regex;
use serde::Deserialize;
use speedtest_controller::plugin::{ConnectionDescriptor, PluginMetaData, ProtocolDescriptor};
use speedtest_controller::process::create_process_and_wait_for_pattern;
use std::sync::{Arc, Mutex};
use tokio::{
    process::{Child, Command},
    signal::ctrl_c,
};

#[derive(Debug, Default)]
struct HelloPlugin {
    process: Option<Child>,
}

#[derive(Debug, Deserialize, Default)]
struct HelloPluginConfig {
    display_string: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = Server::builder().build("127.0.0.1:0").await?;
    let hello_plugin: Arc<Mutex<HelloPlugin>> = Default::default();
    let mut module = RpcModule::new(());
    module.register_method("metadata", |_, _| PluginMetaData {
        name: "hello".to_owned(),
    })?;
    module.register_method("parse_protocol", |_, _| -> Result<_, ErrorObject> {
        Ok(vec![ProtocolDescriptor {
            name: "hello-dummy".to_owned(),
            content: serde_json::Value::Null,
        }])
    })?;
    module.register_method("init", |params, _| {
        println!("init with {:?}", params);
    })?;
    {
        module.register_async_method("setup_proxy", move |params, _| {
            let hello_plugin = Arc::clone(&hello_plugin);
            async move {
                let (params,): (serde_json::Value,) = params.parse()?;
                assert_eq!(params, serde_json::Value::Null);
                let mut command = Command::new("gost");
                command.arg("-L").arg("socks5://:0");
                let re = Regex::new(r"socks5:\/\/:0 on \[::\]:(\d+)").unwrap();
                let (connection_string, child) =
                    create_process_and_wait_for_pattern(command, re, |[port]| {
                        format!("socks5://127.0.0.1:{}", port)
                    })
                    .await;
                let mut guard = hello_plugin.lock().unwrap();
                std::mem::swap(&mut guard.process, &mut Some(child));
                Result::<_, ErrorObject>::Ok(ConnectionDescriptor {
                    http: None,
                    socks5: Some(connection_string),
                    tun: false,
                })
            }
        })?;
    }
    let addr = server.local_addr()?;
    println!("Listen on {}", addr);
    let handle = server.start(module);
    ctrl_c().await?;
    handle.stop().unwrap();
    Ok(())
}
