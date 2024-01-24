use clap::Parser;
use config::Config;
use speedtest_controller::speedtest::SpeedTest;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "config")]
    config: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args = Args::parse();
    let settings = Config::builder()
        .add_source(config::File::with_name(&args.config))
        .build()?;
    let config: speedtest_controller::speedtest::Config = settings.try_deserialize()?;
    let speedtest = SpeedTest::new(config).await;
    Ok(())
}
