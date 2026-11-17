use anyhow::{Result, bail};
use async_trait::async_trait;
use log::info;

use crate::config_files::{ConfigFilesInfo, ConfigSearchResult};

mod local_context;
mod remote_context;

#[async_trait]
pub trait Context {
    async fn build(&self, service: Option<&str>) -> Result<()>;
    async fn up(&self, detach: bool, service: Option<&str>) -> Result<()>;
    async fn down(&self, timeout_sec: Option<u32>, drop_volumes: bool) -> Result<()>;
}

pub fn create_context() -> Result<Box<dyn Context + Send + Sync>> {
    match ConfigFilesInfo::search_from_cwd()? {
        ConfigSearchResult::Ok(config_files) => {
            info!("Using remote context");
            Ok(Box::new(remote_context::RemoteContext::new(config_files)))
        },
        ConfigSearchResult::NoRemote => {
            info!("Fallback to local docker installation"); // not implemented yet
            Ok(Box::new(local_context::LocalContext {}))
        },
        ConfigSearchResult::NoDocker => {
            bail!("Outside of docker project");
        }
    }
}
