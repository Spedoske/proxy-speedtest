use std::collections::HashMap;
use std::sync::Arc;

use futures::future::join_all;
use futures::Future;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use tokio::process::{Child, Command};
use url::Url;

use crate::plugin::json_rpc::JSONRPCPlugin;
use crate::plugin::{Plugin, PluginType, ProtocolDescriptor, TestDescriptor};
use crate::plugin_loader::{PluginLoaderError, Result};
use crate::process::create_process_and_wait_for_pattern;

#[derive(Debug, Deserialize)]
pub struct PluginConfig {
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

type PluginMap = HashMap<String, Arc<dyn Plugin>>;
type ProxyProviderMap = HashMap<String, (Arc<dyn Plugin>, Vec<ProtocolDescriptor>)>;
type TestProviderMap = HashMap<String, (Arc<dyn Plugin>, Vec<TestDescriptor>)>;

async fn get_provider_map<Content, F, FR, Args, Err>(
    plugin_map: &PluginMap,
    transform: F,
    args: &Args,
) -> HashMap<String, (Arc<dyn Plugin>, Vec<Content>)>
where
    Args: Clone,
    FR: Future<Output = std::result::Result<Vec<Content>, Err>>,
    F: Fn(String, Arc<dyn Plugin>, Args) -> FR,
{
    let providers: Vec<(_, _, _)> = join_all(plugin_map.clone().into_iter().map(
        |(plugin_name, plugin)| async {
            (
                plugin_name.clone(),
                plugin.clone(),
                (transform(plugin_name, plugin, args.clone())).await,
            )
        },
    ))
    .await;

    providers
        .into_iter()
        .filter_map(|(plugin_name, plugin, result)| match result {
            Ok(vec) => Some((plugin_name, (plugin, vec))),
            Err(_) => None,
        })
        .collect()
}

pub struct SpeedTest {
    plugin_map: PluginMap,
}

struct FileJSONRPCPlugin {
    inner: JSONRPCPlugin,
    #[allow(dead_code)]
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
            let inner = JSONRPCPlugin::new(&endpoint, config.config).await?;
            Ok(Arc::new(FileJSONRPCPlugin { inner, process }))
        }
        _ => Err(PluginLoaderError::UnexpectedScheme(config.source.into())),
    }
}

impl SpeedTest {
    pub async fn new(plugins: HashMap<String, PluginConfig>) -> Self {
        let plugin_map: Vec<(_, _)> = join_all(
            plugins
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

    pub async fn get_proxy_provider(&self, connection_string: &str) -> ProxyProviderMap {
        get_provider_map(
            &self.plugin_map,
            |_, plugin, connection_string| async move {
                plugin.parse_protocol(&connection_string).await
            },
            &connection_string.to_owned(),
        )
        .await
    }

    pub async fn get_test_provider(&self) -> TestProviderMap {
        get_provider_map(
            &self.plugin_map,
            |_, plugin, _| async move { plugin.tests().await },
            &(),
        )
        .await
    }
}
