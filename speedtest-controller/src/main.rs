#![feature(impl_trait_in_fn_trait_return)]

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use clap::Parser;
use config::Config;
use futures::stream;
use futures::Future;
use futures::FutureExt;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use speedtest_controller::plugin::ConnectionDescriptor;
use speedtest_controller::plugin::Plugin;
use speedtest_controller::plugin::ProtocolDescriptor;
use speedtest_controller::plugin::TestDescriptor;
use speedtest_controller::speedtest::ProxyProviderMap;
use speedtest_controller::speedtest::TestProviderMap;
use speedtest_controller::speedtest::{PluginConfig, SpeedTest};
// use url::Url;

#[derive(Debug, Deserialize)]
pub struct ControllerConfig {
    plugins: HashMap<String, PluginConfig>,
    connection_string: String, //Url
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "config")]
    config: String,
}

#[derive(Debug, Serialize, Default)]
struct Output {
    test_results: HashMap<String, HashMap<String, HashMap<String, HashMap<String, Value>>>>,
}

async fn collect_test_results(
    plugin: Arc<dyn Plugin>,
    proxy_connection: ConnectionDescriptor,
    tests: Vec<TestDescriptor>,
) -> HashMap<String, Value> {
    let run_test_future = try_run_test(plugin, proxy_connection);
    stream::iter(tests)
        .filter_map(run_test_future)
        .collect::<HashMap<_, _>>()
        .await
}

async fn collect_test_results_from_test_providers(
    test_providers: TestProviderMap,
    proxy_connection: ConnectionDescriptor,
) -> HashMap<String, HashMap<String, Value>> {
    stream::iter(test_providers)
        .then(|(test_provider, (plugin, tests))| {
            let proxy_connection: ConnectionDescriptor = proxy_connection.clone();
            async move {
                (
                    test_provider,
                    collect_test_results(plugin, proxy_connection, tests).await,
                )
            }
        })
        .collect()
        .await
}

async fn perform_speedtest_for_proxies(
    plugin: Arc<dyn Plugin>,
    proxies: Vec<ProtocolDescriptor>,
    test_providers: TestProviderMap,
) -> HashMap<String, HashMap<String, HashMap<String, Value>>> {
    let setup_proxy_future = try_set_up_proxy(plugin);
    stream::iter(proxies)
        .filter_map(|proxy| {
            let proxy_name = proxy.name.clone();
            let proxy_connection = setup_proxy_future(proxy);
            let test_providers = test_providers.clone();
            async move {
                let proxy_connection = proxy_connection.await;
                match proxy_connection {
                    None => None,
                    Some(proxy_connection) => Some((
                        proxy_name,
                        collect_test_results_from_test_providers(test_providers, proxy_connection)
                            .await,
                    )),
                }
            }
        })
        .collect()
        .await
}

async fn perform_speedtest_for_proxy_providers(
    proxy_providers: ProxyProviderMap,
    test_providers: TestProviderMap,
) -> HashMap<String, HashMap<String, HashMap<String, HashMap<String, Value>>>> {
    stream::iter(proxy_providers)
        .then(|(provider, (plugin, proxies))| {
            let test_providers = test_providers.clone();
            async move {
                (
                    provider,
                    perform_speedtest_for_proxies(plugin, proxies, test_providers).await,
                )
            }
        })
        .collect()
        .await
}

fn try_run_test(
    plugin: Arc<dyn Plugin>,
    proxy_connection: ConnectionDescriptor,
) -> impl Fn(TestDescriptor) -> Pin<Box<dyn Future<Output = Option<(String, Value)>>>> {
    move |test| {
        let plugin = plugin.clone();
        let proxy_connection = proxy_connection.clone();
        async move {
            let test_result: Result<Value, speedtest_controller::plugin::PluginError> =
                plugin.run_test(&test, &proxy_connection).await;
            match test_result {
                Ok(p) => Some((test.name.clone(), p)),
                Err(e) => {
                    log::error!("Failed to run test {test:?} given {proxy_connection:?}. {e}");
                    None
                }
            }
        }
        .boxed()
    }
}

fn try_set_up_proxy(
    plugin: Arc<dyn Plugin>,
) -> impl Fn(ProtocolDescriptor) -> Pin<Box<dyn Future<Output = Option<ConnectionDescriptor>>>> {
    move |proxy| {
        let plugin = plugin.clone();
        async move {
            let proxy_connection = plugin.setup_proxy(proxy.content).await;
            match proxy_connection {
                Err(e) => {
                    log::error!("Cannot setup proxy. {e}");
                    None
                }
                Ok(proxy_connection) => Some(proxy_connection),
            }
        }
        .boxed()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    println!("{:?}", std::env::current_dir()?);
    let args = Args::parse();
    let settings = Config::builder()
        .add_source(config::File::with_name(&args.config))
        .build()?;
    let config: ControllerConfig = settings.try_deserialize()?;
    let speedtest = SpeedTest::new(config.plugins).await;
    let proxy_providers = speedtest
        .get_proxy_provider(&config.connection_string)
        .await;
    let test_providers = speedtest.get_test_provider().await;
    let output: Output = Output {
        test_results: perform_speedtest_for_proxy_providers(proxy_providers, test_providers).await,
    };
    println!("{:?}", output);
    Ok(())
}
