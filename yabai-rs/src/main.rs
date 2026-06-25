mod config;
mod ipc;
mod layout;
mod wm;

use anyhow::Result;
use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "yabai-rs", about = "A fast macOS tiling window manager in Rust")]
struct Args {
    /// Path to config file
    #[arg(long, short)]
    config: Option<String>,

    #[arg(long, short)]
    version: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("yabai_rs=info".parse()?))
        .init();

    let args = Args::parse();

    if args.version {
        println!("yabai-rs {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let config_path = args.config.unwrap_or_else(|| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        format!("{home}/.config/yabai-rs/yabai-rs.toml")
    });

    let cfg = config::Config::load(&config_path)?;
    info!("Loaded config from {config_path}");

    let mut manager = wm::WindowManager::new(cfg)?;
    manager.run().await
}
