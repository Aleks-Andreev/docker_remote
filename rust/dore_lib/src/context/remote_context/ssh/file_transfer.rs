use std::collections::HashMap;
use std::sync::Arc;
use std::path::{PathBuf, Path};

use anyhow::{Result, bail};
use futures::stream;
use futures::stream::StreamExt;
use log::debug;
use russh_sftp::client::SftpSession;
use russh_sftp::client::error::Error;
use russh_sftp::protocol::Status;
use russh_sftp::protocol::StatusCode::PermissionDenied;

use crate::config_files::RemoteConfig;
use super::SshSession;

pub(crate) async fn sftp_file_transfer(
    session: Arc<SshSession>,
    remote_config: &RemoteConfig,
    files: HashMap<PathBuf, PathBuf>
) -> Result<()> 
{
    let target_path = Arc::new({
        let channel = session.create_channel().await?;
        channel.request_subsystem(true, "sftp").await?;

        let sftp = SftpSession::new(channel.into_stream()).await?;
        let path = get_or_create_target_dir(&sftp, remote_config.path()).await?;
        sftp.close().await?;
        path
    });

    let concurrency = 4;
    stream::iter(files)
        .map(|(local, remote)| {
            debug!("Transferring {:?} to {remote:?}", local.file_name());

            let session = Arc::clone(&session);
            let target_path = Arc::clone(&target_path);
            async move {
                let channel = session.create_channel().await?;
                channel.request_subsystem(true, "sftp").await?;

                let sftp = SftpSession::new(channel.into_stream()).await?;
                if let Some(parent) = remote.parent() {
                    create_target_dir(&sftp, target_path.as_str(), parent).await?;
                } // else -> it's a single file that goes directly to target dir that's already exists

                let remote_fname = remote.file_name().expect("name should exists").to_string_lossy();
                let mut remote_file = sftp.create(remote_fname).await?;
                let local_data = tokio::fs::read(local).await?;
                tokio::io::AsyncWriteExt::write_all(&mut remote_file, &local_data).await?;
                
                tokio::io::AsyncWriteExt::shutdown(&mut remote_file).await?;
                sftp.close().await?;

                Ok::<_, anyhow::Error>(())
            }

        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await;

    Ok(())
}

async fn create_target_dir(
    sftp: &SftpSession,
    exist_path_part: impl AsRef<Path>,
    unknown_path_part: impl AsRef<Path>
) -> Result<String>
{
    let mut target_path = exist_path_part.as_ref().to_path_buf();
    for dir in unknown_path_part.as_ref().iter() {
        target_path.push(dir);

        let current_path = target_path.to_string_lossy();
        match sftp.create_dir(current_path.clone()).await {
            Err(Error::Status( Status { status_code, ..})) => {
                if status_code == PermissionDenied {
                    bail!("Can't access required path: '{:?}'", unknown_path_part.as_ref());
                }
            },
            Err(_) => {
                if let Ok(true) = sftp.try_exists(current_path).await {
                    continue;
                }
            }
            _ => { continue; }
        }
    }

    Ok(target_path.to_string_lossy().to_string())
}

async fn get_or_create_target_dir(sftp: &SftpSession, target: &str) -> Result<String> {
    if target.starts_with("/") {
        if let Ok(x) = sftp.canonicalize(target).await {
            return Ok(x);
        }

        let exist = PathBuf::from("/");
        let target = PathBuf::from(target);
        let unknown = target.strip_prefix("/").unwrap().to_owned();
        
        return create_target_dir(sftp, exist, unknown).await
    }
    
    let mut target = target;
    if target.starts_with("~/") || target.starts_with("./") {
        target = &target[2..];
    }

    if let Ok(x) = sftp.canonicalize(target).await {
        return Ok(x);
    }

    let exist = PathBuf::from(sftp.canonicalize(".").await?);
    let unknown = PathBuf::from(target);

    create_target_dir(sftp, exist, unknown).await
}