use std::{fmt::Debug, path::Path};
use std::io::Read;
use std::fs::File;
use std::collections::HashSet;

use anyhow::{Result, bail};
use yaml_rust2::{Yaml, YamlLoader};

pub(crate) enum RemoteAuth {
    PrivateKeyPath(String),
    PlainPassword(String),
}

impl Debug for RemoteAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PrivateKeyPath(key) => write!(f, "key {key}"),
            Self::PlainPassword(_) => write!(f, "password"),
        }
    }
}

#[derive(Default, Debug)]
struct FileRules {
    included: HashSet<String>,
    excluded: HashSet<String>,
}

#[derive(Debug)]
pub(crate) struct RemoteConfig {
    host: String,
    port: u16,
    user: String,
    path: String,
    auth: RemoteAuth,
    files: FileRules,
}

impl RemoteConfig {
    pub fn host(&self) -> &str { &self.host }
    pub fn port(&self) -> u16 { self.port }
    pub fn user(&self) -> &str { &self.user }
    pub fn path(&self) -> &str { &self.path }
    pub fn auth(&self) -> &RemoteAuth { &self.auth }

    pub fn excluded_files(&self) -> impl Iterator<Item = &String> {
        self.files.excluded.iter()
    }

    pub fn included_files(&self) -> impl Iterator<Item = &String> {
        self.files.included.iter()
    }
}

impl RemoteConfig {
    pub fn parse_config(path: &Path) -> Result<Self> {
        let mut file = File::open(path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;

        let config = YamlLoader::load_from_str(&content)?;
        let Some(root) = config.first() else {
            bail!("Invalid docker-remote config");
        };

        let target = &root["target"];
        if target.is_badvalue() {
            bail!("target is required");
        }

        let Some(user) = target["user"].as_str() else {
            bail!("'target::user' is required");
        };
        let Some(host) = target["host"].as_str() else {
            bail!("'target::host' is required");
        };
        let Some(path) = target["path"].as_str() else {
            bail!("'target::path' is required");
        };
        let port = target["port"].as_i64().unwrap_or(22) as u16;

        let auth = &root["auth"];
        if auth.is_badvalue() {
            bail!("'auth' is required");
        }

        let auth = if let Yaml::String(key) = &auth["key"] {
            let expanded = shellexpand::full( key)?;
            RemoteAuth::PrivateKeyPath(expanded.to_string())
        }
        else if let Yaml::String(password) = &auth["password"] {
            RemoteAuth::PlainPassword(password.to_string())
        }
        else {
            bail!("either 'auth::key' or 'auth::password' is required");
        };

        let y_to_string = |y: &Yaml| y.as_str().map(|s| s.to_string());

        let mut files = FileRules::default();
        if let Some(excluded) = root["files"]["excluded"].as_vec() {
            files.excluded.extend(excluded.iter().filter_map(y_to_string));
        }
        if let Some(included) = root["files"]["included"].as_vec() {
            files.included.extend(included.iter().filter_map(y_to_string)); // TOFIX: disallow wildcards
        }

        Ok(Self {
            host: host.to_string(),
            user: user.to_string(),
            port,
            path: path.to_string(),
            auth,
            files,
        })
    }
}
