use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use log::{debug, error, info};

use crate::Context;
use crate::config_files::ConfigFilesInfo;
use crate::context::remote_context::ssh::SshSession;
use crate::config_files::RemoteConfig;

mod file_enumerator;
mod ssh;

pub(crate) struct RemoteContext {
    config_files: ConfigFilesInfo,
}

impl RemoteContext {
    pub fn new(config_files: ConfigFilesInfo) -> Self {
        Self {
            config_files,
        }
    }

    async fn collect_files(&self) -> Result<HashMap<PathBuf, PathBuf>> {
        let compose_items = self.config_files.get_compose_references().await?;
        debug!("Collected items from compose: {compose_items:?}");

        let remote_config = self.config_files.get_remote_config()?;
        let files = file_enumerator::enumerate_files_for_copy(
            self.config_files.up(),
            remote_config,
            compose_items)?;
        debug!("Files ready to transfer: {files:?}");

        Ok(files)
    }

    async fn create_ssh_session(&self, remote_config: &RemoteConfig) -> Result<Option<Arc<SshSession>>> {
        debug!("Connecting server with config: {remote_config:?}");

        // Message to user terminal
        println!("Connecting to ssh://{}@{}:{}{}{}",
            remote_config.user(),
            remote_config.host(),
            remote_config.port(),
            if remote_config.path().starts_with('/') { "" } else { "/" },
            remote_config.path()
        );

        match ssh::SshSession::create(remote_config).await {
            Ok(s) => {
                debug!("Connection established");
                Ok(Some(s))
            },
            Err(e) => {
                error!("Failed connect to {}:{}. {e}", remote_config.host(), remote_config.port());
                Ok(None) // do not propagate error to main
            },
        }
    }

    async fn sync_and_exec(&self, do_sync: bool, command_to_exec: &str) -> Result<()> {
        let remote_config = self.config_files.get_remote_config()?;
        let Some(session) = self.create_ssh_session(remote_config).await? else {
            return Ok(());
        };

        if do_sync {
            let files = self.collect_files().await?;
            let target_path = ssh::create_target_dir(Arc::clone(&session), remote_config.path()).await?;
            let target_path = Arc::new(target_path);
            ssh::sftp_file_transfer(Arc::clone(&session), target_path, files).await?;
        }

        let path_to_compose_unix = self.config_files.relative_path().iter()
            .fold(String::new(), |mut path, next_dir| {
                path.push_str(&next_dir.to_string_lossy());
                path.push('/');
                path
            });
        let command_with_work_dir = format!("cd {}/{} && {command_to_exec}", remote_config.path(), path_to_compose_unix);

        ssh::run_interactive_command(Arc::clone(&session), &command_with_work_dir).await?;
        if let Ok(session) = Arc::try_unwrap(session) {
            debug!("Close SSH session");
            session.disconnect().await;
        }
        else {
            info!("App error: session in use");
        }
        Ok(())
    }
}

#[async_trait]
impl Context for RemoteContext {
    async fn build(&self, services: &[String]) -> anyhow::Result<()> {
        let mut command = "docker compose build".to_string();
        for service in services {
            command.push(' ');
            command.push_str(service);
        }
        self.sync_and_exec(true, &command).await
    }

    async fn up(&self, detach: bool, services: &[String]) -> anyhow::Result<()> {
        let mut cmd = "docker compose up".to_string();
        if detach {
            cmd.push_str(" -d");
        }
        for service in services {
            cmd.push(' ');
            cmd.push_str(service);
        }

        self.sync_and_exec(true, &cmd).await
    }

    async fn down(&self, timeout_sec: Option<u32>, drop_volumes: bool, services: &[String]) -> anyhow::Result<()> {
        let mut cmd = "docker compose down".to_string();
        if let Some(timeout_sec) = timeout_sec {
            cmd.push_str(&format!(" --timeout {timeout_sec}"));
        }
        if drop_volumes {
            cmd.push_str(" --volumes");
        }
        for service in services {
            cmd.push(' ');
            cmd.push_str(service);
        }
        
        self.sync_and_exec(false, &cmd).await
    }
}