use anyhow::Result;
use clap::Parser;

use dore_lib::{create_context};
use log::info;

mod logger;
mod cli;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    logger::setup_logger(cli.verbose);
    info!("App started. CWD: {:?}", std::env::current_dir());

    if let Ok(ctx) = create_context() {
        match cli.commands {
            cli::Commands::Up { detach, services } => {
                ctx.up(detach, &services).await?;
            },
            cli::Commands::Down { timeout, drop_volumes, services } => {
                ctx.down(timeout, drop_volumes, &services).await?
            },
            cli::Commands::Build { services } => {
                ctx.build(&services).await?;
            },
        }
    } // else -> outside of docker project folder, nothing to do

    Ok(())
}

