use std::env::current_dir;
use std::fs::read_dir;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::Result;

mod remote_config;
use log::error;
pub(crate) use remote_config::{RemoteConfig, RemoteAuth};

mod compose_items;
pub(crate) use compose_items::ComposeReferencedItems;

pub(crate) enum ConfigSearchResult {
    Ok(ConfigFilesInfo),
    NoRemote,
    NoDocker,
}

pub(crate) struct ConfigFilesInfo {
    remote_cfg_path: PathBuf,
    compose_cfg_path: PathBuf,
    relative_path: PathBuf,
    compose_path: PathBuf,
    up: PathBuf,

    remote_config: OnceLock<RemoteConfig>,
    compose_config: OnceLock<ComposeReferencedItems>,
}

impl ConfigFilesInfo {
    pub(crate) fn compose_folder(&self) -> &Path { &self.compose_path }
    pub(crate) fn compose_config(&self) -> &Path { &self.compose_cfg_path }
    pub(crate) fn relative_path(&self) -> &Path { &self.relative_path }
    pub(crate) fn up(&self) -> &Path { &self.up }
}

impl ConfigFilesInfo {
    fn new(remote_cfg: PathBuf, compose_cfg: PathBuf, up: PathBuf) -> Result<Self> {
        let relative_path = {
            let prefix_len = remote_cfg.iter().count() - 1;
            let mut relative_path: PathBuf = compose_cfg.iter().skip(prefix_len).collect();
            relative_path.pop();
            relative_path
        };

        let mut compose_path = compose_cfg.clone();
        compose_path.pop();

        Ok(Self {
            remote_cfg_path: remote_cfg,
            compose_cfg_path: compose_cfg,
            relative_path,
            compose_path,
            up,
            remote_config: OnceLock::new(),
            compose_config: OnceLock::new(),
        })
    }

    pub fn search_from_cwd() -> Result<ConfigSearchResult> {
        const REMOTE_CFG_NAME: &str = "docker-remote.yaml";
        const COMPOSE_CFG_NAME: &str = "compose.yaml";

        let mut remote: Option<PathBuf> = None;
        let mut compose: Option<PathBuf> = None;

        let mut cwd = current_dir()?;
        let mut up = PathBuf::new();
        'a: loop {
            for entry in read_dir(&cwd)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    let fname = path.file_name().expect("file should have a name");
                    if fname.to_ascii_lowercase() == REMOTE_CFG_NAME {
                        remote = Some(path);
                    }
                    else if fname.to_ascii_lowercase() == COMPOSE_CFG_NAME {
                        compose = Some(path);
                    }
                }

                if remote.is_some() && compose.is_some() {
                    break 'a;
                }
            }

            if cwd.parent().is_some() {
                cwd.pop();
            }
            else {
                break;
            }
            up.push("..");
        }

        // TOFIX: check if remote is above (or on the same level) as compose
        let search_result = match (remote, compose) {
            (Some(remote), Some(compose)) => ConfigSearchResult::Ok(ConfigFilesInfo::new(remote, compose, up)?),
            (None, Some(_)) => ConfigSearchResult::NoRemote,
            _ => {
                error!("Could not find docker compose file");
                ConfigSearchResult::NoDocker
            }
        };
        
        Ok(search_result)
    }

    pub fn get_remote_config(&self) -> Result<&RemoteConfig> {
        if let Some(remote_config) = self.remote_config.get() {
            Ok(remote_config)
        }
        else {
            let _ = self.remote_config.set(RemoteConfig::parse_config(&self.remote_cfg_path)?);
            let Some(remote_config) = self.remote_config.get() else { unreachable!() };
            Ok(remote_config)
        }
    }

    pub async fn get_compose_references(&self) -> Result<&ComposeReferencedItems> {
        if let Some(compose_items) = self.compose_config.get() {
            Ok(compose_items)
        }
        else {
           let _ = self.compose_config.set(ComposeReferencedItems::parse_config(self).await?);
           let Some(compose_items) = self.compose_config.get() else { unreachable!() };
           Ok(compose_items)
        }
    }
}
