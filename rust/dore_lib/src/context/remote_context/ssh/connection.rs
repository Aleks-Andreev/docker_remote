use std::sync::Arc;

use anyhow::Result;
use log::debug;
use russh::{Channel, client::{self, Handle, Msg}};

use crate::config_files::{RemoteConfig, RemoteAuth};

struct ClientHandler;

impl client::Handler for ClientHandler {
    type Error = anyhow::Error;

    async fn check_server_key(&mut self, _server_public_key: &russh::keys::PublicKeyOrCertificate) -> Result<bool, Self::Error>
    {
        Ok(true) // TODO: check server key
    }
}

pub(crate) struct SshSession {
    session: Handle<ClientHandler>,
}

impl SshSession {
    pub async fn create(remote_config: &RemoteConfig) -> Result<Arc<Self>> {
        let config = Arc::new(client::Config::default());
        let handler = ClientHandler;

        let mut session = client::connect(config, (remote_config.host(), remote_config.port()), handler).await?;
        let auth_result = match remote_config.auth() {
            RemoteAuth::PrivateKeyPath(path) => {
                let pem_content = tokio::fs::read(path).await?;
                let key = russh::keys::PrivateKey::from_openssh(pem_content)?;
                let key: russh::keys::PrivateKeyWithHashAlg = russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key), Some(russh::keys::HashAlg::Sha512));
                session.authenticate_publickey(remote_config.user(), key).await
            },
            RemoteAuth::PlainPassword(pass) => {
                session.authenticate_password(remote_config.user(), pass).await
            },
        }?;

        if !auth_result.success() {
            anyhow::bail!("Auth failed");
        }

        Ok(
            Arc::new(
                Self {
                    session
                }
            )
        )
    }

    pub async fn create_channel(&self) -> Result<Channel<Msg>> {
        Ok(self.session.channel_open_session().await?)
    }

    pub async fn disconnect(self) {
        debug!("Disconnecting from remote server");
        if !self.session.is_closed() {
            let _ = self.session.disconnect(russh::Disconnect::ByApplication, "", "en").await;
            let _ = self.session.await;
        }
    }
}