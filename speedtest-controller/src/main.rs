use std::collections::HashMap;

use clap::Parser;
use config::Config;
use futures::stream;
use futures::StreamExt;
use serde::{ Deserialize, Serialize };
use serde_json::Value;
use speedtest_controller::speedtest::{ PluginConfig, SpeedTest };
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args = Args::parse();
    let settings = Config::builder().add_source(config::File::with_name(&args.config)).build()?;
    let config: ControllerConfig = settings.try_deserialize()?;
    let speedtest = SpeedTest::new(config.plugins).await;
    let proxy_providers = speedtest.get_proxy_provider(&config.connection_string).await;
    let test_providers = speedtest.get_test_provider().await;
    let output: Output = Output {
        test_results: stream
            ::iter(&proxy_providers)
            .then(|(provider, (plugin, proxies))| {
                let test_providers = &test_providers;
                async move {
                    (
                        provider.clone(),
                        stream
                            ::iter(proxies)
                            .filter_map(|proxy| {
                                let test_providers = &test_providers;
                                async move {
                                    let proxy_connection = plugin.configure(
                                        proxy.content.clone()
                                    ).await;
                                    match proxy_connection {
                                        Err(e) => {
                                            log::error!(
                                                "Cannot setup proxy for provider {provider}. {e}"
                                            );
                                            None
                                        }
                                        Ok(proxy_connection) =>
                                            Some((
                                                proxy.name.clone(),
                                                stream
                                                    ::iter(test_providers.iter())
                                                    .then(|(test_provider, (plugin, tests))| {
                                                        let proxy_connection = &proxy_connection;
                                                        async move {
                                                            (
                                                                test_provider.clone(),
                                                                stream
                                                                    ::iter(tests)
                                                                    .filter_map(|test| async move {
                                                                        let test_result =
                                                                            plugin.run_test(
                                                                                test,
                                                                                proxy_connection
                                                                            ).await;
                                                                        match test_result {
                                                                            Ok(p) =>
                                                                                Some((
                                                                                    test.name.clone(),
                                                                                    p,
                                                                                )),
                                                                            Err(e) => {
                                                                                log::error!(
                                                                                    "Failed to run test {test:?} given {proxy_connection:?}. {e}"
                                                                                );
                                                                                None
                                                                            }
                                                                        }
                                                                    })
                                                                    .collect::<
                                                                        HashMap<_, _>
                                                                    >().await,
                                                            )
                                                        }
                                                    })
                                                    .collect::<HashMap<_, _>>().await,
                                            )),
                                    }
                                }
                            })
                            .collect::<HashMap<_, _>>().await,
                    )
                }
            })
            .collect::<HashMap<_, _>>().await,
    };
    println!("{:?}", output);
    Ok(())
}
