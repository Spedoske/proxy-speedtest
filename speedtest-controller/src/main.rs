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
                                        
                                        Ok(proxy_connection) => {
                                            // Initialize a HashMap to store results for each test provider.
                                            let mut all_test_results = HashMap::new();
                                        
                                            // Iterate over each test provider along with their plugins and tests.
                                            for (test_provider, (plugin, tests)) in test_providers.iter() {
                                                // Initialize a HashMap to store results for each test.
                                                let mut test_results = HashMap::new();
                                        
                                                // Iterate over each test.
                                                for test in tests {
                                                    // Attempt to run the test, handling potential errors with '?'
                                                    // Errors are logged and the failed test is skipped.
                                                    let test_result = plugin.run_test(test, &proxy_connection).await;
                                                    match test_result {
                                                        Ok(p) => {
                                                            // On success, store the test result.
                                                            test_results.insert(test.name.clone(), p);
                                                        }
                                                        Err(e) => {
                                                            // Log the error and skip this test.
                                                            log::error!(
                                                                "Failed to run test {test:?} given {proxy_connection:?}. {e}"
                                                            );
                                                            continue;
                                                        }
                                                    }
                                                }
                                        
                                                // Store the results for this test provider.
                                                all_test_results.insert(test_provider.clone(), test_results);
                                            }
                                        
                                            // Return the proxy name and all the test results.
                                            Some((proxy.name.clone(), all_test_results))
                                        }
                                        
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
