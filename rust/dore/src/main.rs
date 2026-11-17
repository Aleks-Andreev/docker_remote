use anyhow::Result;

use dore_lib::{create_context};
use log::info;

mod logger;

#[tokio::main]
async fn main() -> Result<()> {
    logger::setup_logger();
    info!("App started. CWD: {:?}", std::env::current_dir());

    if let Ok(ctx) = create_context() {
        ctx.up(false, None).await?;
    } // else -> outside of docker project folder, nothing to do

    Ok(())
}

