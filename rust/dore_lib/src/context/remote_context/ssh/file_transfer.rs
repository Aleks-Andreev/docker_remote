use std::collections::HashMap;
use std::sync::Arc;
use std::path::{PathBuf, Path};

use anyhow::{Result, bail};
use futures::stream;
use futures::stream::StreamExt;
use log::{debug, info, warn};
use russh_sftp::client::SftpSession;
use russh_sftp::client::error::Error;
use russh_sftp::protocol::Status;
use russh_sftp::protocol::StatusCode::PermissionDenied;

use super::SshSession;

pub(crate) async fn sftp_file_transfer(
    session: Arc<SshSession>,
    target_path: Arc<String>,
    files: HashMap<PathBuf, PathBuf>
) -> Result<()> 
{
    let concurrency = 4;
    info!("Start transferring files to target location: {target_path}");
    stream::iter(files)
        .map(|(local, remote)| {
            let session = Arc::clone(&session);
            let target_path = Arc::clone(&target_path);
            async move {
                let channel = session.create_channel().await?;
                channel.request_subsystem(true, "sftp").await?;

                let sftp = SftpSession::new(channel.into_stream()).await?;
                let mut remote_file_path = if let Some(parent) = remote.parent() {
                    extend_existing_path(&sftp, target_path.as_str(), parent).await?
                }
                else {
                    target_path.as_str().to_owned()
                };

                let remote_fname = remote.file_name().expect("name should exists").to_string_lossy();
                remote_file_path.push('/');
                remote_file_path.push_str(&remote_fname);
                
                debug!("Opening {remote_file_path} as copy of {local:?}");
                let mut remote_file = sftp.create(&remote_file_path).await?;
                let local_data = tokio::fs::read(local).await?; // TODO: use buffering for large files
                tokio::io::AsyncWriteExt::write_all(&mut remote_file, &local_data).await?;
                debug!("Stored {} bytes to {remote_file_path}", local_data.len());
                
                tokio::io::AsyncWriteExt::shutdown(&mut remote_file).await?;
                sftp.close().await?;

                Ok::<_, anyhow::Error>(())
            }

        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await;

    info!("All files copied");
    Ok(())
}

async fn extend_existing_path(
    sftp: &SftpSession,
    exist_path_part: &str,
    unknown_path_part: impl AsRef<Path>
) -> Result<String>
{
    debug!("Extend existing path '{exist_path_part}' to required {:?}", unknown_path_part.as_ref().to_str());

    let mut target_path = exist_path_part.to_string();
    for dir in unknown_path_part.as_ref().iter() {
        target_path.push('/');
        target_path.push_str(&dir.to_string_lossy());

        if sftp.try_exists(&target_path).await? {
            debug!("Path '{target_path}' already exist");
            continue;
        }

        debug!("Creating directory '{target_path}'");
        match sftp.create_dir(&target_path).await {
            Err(Error::Status( Status { status_code, ..})) => {
                if status_code == PermissionDenied {
                    bail!("Can't access required path: '{:?}'", unknown_path_part.as_ref());
                }
            },
            Err(e) => {
                warn!("Failed to create folder at '{target_path}' because of {e}");
                return Err(anyhow::anyhow!(e));
            }
            Ok(_) => {
                debug!("Directory '{target_path} created'");
            }
        }
    }

    Ok(target_path)
}

pub(crate) async fn create_target_dir(session: Arc<SshSession>, target: &str) -> Result<String> {
    debug!("Creating target dir '{target}'");

    let channel = session.create_channel().await?;
    channel.request_subsystem(true, "sftp").await?;

    let sftp = SftpSession::new(channel.into_stream()).await?;

    if target.starts_with("/") {
        let target = PathBuf::from(&target[1..]);
        let exist = "";
        debug!("Will create target path {target:?} starting from '{exist}'");
        return extend_existing_path(&sftp, exist, target).await
    }

    let exist = sftp.canonicalize(".").await?;
    if target.starts_with("~/") || target.starts_with("./") {
        let target = PathBuf::from(&target[2..]);
        debug!("Will create target path {target:?} starting from home directory '{exist}'");
        return extend_existing_path(&sftp, &exist, target).await;
    }

    let target = PathBuf::from(target);
    debug!("Will create target path {target:?} starting from home directory '{exist}'");
    return extend_existing_path(&sftp, &exist, target).await;
}