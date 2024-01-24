use std::collections::HashMap;
use std::sync::Arc;

use futures::future::join_all;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use tokio::process::{Child, Command};
use url::Url;

use crate::plugin::json_rpc::JSONRPCPlugin;
use crate::plugin::{Plugin, PluginType};
use crate::plugin_loader::{PluginLoaderError, Result};
use crate::process::create_process_and_wait_for_pattern;

#[derive(Debug, Deserialize)]
struct PluginConfig {
    /// Available format:
    /// ```
    /// docker://image:tag
    /// file://path/to/plugin/executable
    /// ```
    /// TODO: Parse the source
    source: Url,
    #[serde(default)]
    plugin_type: PluginType,
    #[serde(default)]
    config: Value,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    plugins: HashMap<String, PluginConfig>,
}

pub struct SpeedTest {
    plugin_map: HashMap<String, Arc<dyn Plugin>>,
}

struct FileJSONRPCPlugin {
    inner: JSONRPCPlugin,
    process: Child,
}

impl std::ops::Deref for FileJSONRPCPlugin {
    type Target = JSONRPCPlugin;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

async fn load_json_rpc_plugin(config: PluginConfig) -> Result<Arc<dyn Plugin>> {
    assert_eq!(config.plugin_type, PluginType::JSONRPC);
    match config.source.scheme() {
        "file" => {
            let command = Command::new(config.source.path());
            let regex = Regex::new(r"Listen on (.+)").unwrap();
            let (endpoint, process) =
                create_process_and_wait_for_pattern(command, regex, |[endpoint]| {
                    endpoint.to_owned()
                })
                .await;
            let inner = JSONRPCPlugin::new(&endpoint).await?;
            Ok(Arc::new(FileJSONRPCPlugin { inner, process }))
        }
        _ => Err(PluginLoaderError::UnexpectedScheme(config.source.into())),
    }
}

impl SpeedTest {
    pub async fn new(c: Config) -> Self {
        let plugin_map: Vec<(_, _)> = join_all(
            c.plugins
                .into_iter()
                .map(|(k, v)| async { (k, load_json_rpc_plugin(v).await) }),
        )
        .await;

        let plugin_map: HashMap<_, _> = plugin_map
            .into_iter()
            .filter_map(|(k, v)| match v {
                Ok(v) => {
                    log::info!("Plugin {} loaded", k);
                    Some((k, v))
                }
                Err(e) => {
                    log::error!("Unable to load plugin {}, {}", k, e);
                    None
                }
            })
            .collect();
        SpeedTest { plugin_map }
    }
}
