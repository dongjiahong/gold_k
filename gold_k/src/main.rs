use anyhow::Result;
use clap::{Parser, Subcommand};
use gold_k::{config, web};
use tokio::fs;
use validator::Validate;

#[derive(Parser)]
#[clap(
    version = utils::version::get_version(),
    about = "Gold K line",
)]
#[clap(propagate_version = true)]
struct Cli {
    #[arg(short, long, default_value = "app.toml")]
    config: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Web,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    utils::log::init_tracing();

    let cli = Cli::parse();

    let config_path = cli.config;
    let c: config::Config = fs::read_to_string(&config_path).await?.parse()?;
    c.validate()?;

    match cli.command {
        Commands::Web => {
            tracing::info!("Starting web server...");
            // Start the web server
            web::start().await?;
        }
    }

    Ok(())
}
