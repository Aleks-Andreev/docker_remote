use std::path::{Path, PathBuf};
use std::collections::HashSet;

use anyhow::Result;
use yaml_rust2::{Yaml, YamlLoader};
use log::warn;

use crate::config_files::ConfigFilesInfo;

#[derive(Debug)]
pub(crate) struct ComposeReferencedItems {
    files: HashSet<PathBuf>,
    build_contexts: HashSet<PathBuf>,
    volumes: HashSet<PathBuf>,
}

impl ComposeReferencedItems {
    fn new() -> Self {
        Self {
            files: HashSet::new(),
            build_contexts: HashSet::new(),
            volumes: HashSet::new(),
        }
    }

    pub fn build_contexts(&self) -> impl Iterator<Item = &PathBuf> {
        self.build_contexts.iter()
    }

    pub fn volumes(&self) -> impl Iterator<Item = &PathBuf> {
        self.volumes.iter()
    }

    pub async fn parse_config(configs: &ConfigFilesInfo) -> Result<Self> {
        let get_content = async |cfg: &Path| {
            let content = tokio::fs::read_to_string(cfg).await?;
            Ok::<_, anyhow::Error>(content)
        };

        let mut this = Self::new();
        this.parse_internal(configs.compose_config(), configs, &get_content).await?;
        Ok(this)
    }

    async fn parse_internal<F>(&mut self, compose: &Path, configs: &ConfigFilesInfo, f: &F) -> Result<()> 
         where F: AsyncFn(&Path) -> Result<String>
    {
        if self.add_file(compose, configs) {
            let content = f(compose).await?;
            self.parse_content(content, configs, f).await?;
        }

        Ok(())
    }

    fn add_file(&mut self, file: impl AsRef<Path>, configs: &ConfigFilesInfo) -> bool {
        let path = PathBuf::from(file.as_ref());
        if path.is_absolute() {
            if !path.starts_with(configs.compose_folder()) {
                warn!("File '{path:?}' is outside of root dir '{:?}'", configs.compose_folder());
                return false;
            }

            let path = path.strip_prefix(configs.compose_folder()).unwrap();
            let path = configs.relative_path().join(path);
            if self.files.insert(path) {
                return true;
            }
        }
        else {
            let combined = configs.relative_path().join(path);
            if self.files.insert(combined.clone()) {
                return true;
            }
        }

        false
    }

    fn process_env_section(&mut self, configs: &ConfigFilesInfo, node: &Yaml) {
        match &node["env_file"] {
            Yaml::String(file_ref) => {
                self.add_file(file_ref, configs);
            },
            Yaml::Array(files) => {
                for file_ref in files {
                    if let Yaml::String(file_ref) = file_ref {
                        self.add_file(file_ref, configs);
                    }
                    else {
                        if let Yaml::Hash(env_file) = file_ref {
                            if let Some(Yaml::String(path)) = env_file.get(&Yaml::from_str("path")) {
                                self.add_file(path, configs);
                            }
                        }
                    }
                }
            },
            Yaml::BadValue => {},
            _ => warn!("Unexpected 'env_file' content"),
        }
    }
    
    async fn process_config_ref<F>(&mut self, configs: &ConfigFilesInfo, file_ref: &str, f: &F) -> Result<()>
        where F: AsyncFn(&Path) -> Result<String>
    {
        if let Err(e) = Box::pin(self.parse_internal(Path::new(file_ref), configs, f)).await {
            warn!("Failed to parse referenced config in {file_ref}: {e}");
        }

        Ok(())
    }

    async fn parse_content<F>(&mut self, content: String, configs: &ConfigFilesInfo, f: &F) -> Result<()> 
        where F: AsyncFn(&Path) -> Result<String>
    {
        let config = YamlLoader::load_from_str(&content)?;
        let Some(root) = config.first() else {
            // assume that empty file is ok
            return Ok(());
        };

        if let Yaml::Hash(docker_configs) = &root["configs"] {
            for sub_config in docker_configs.values() {
                if let Yaml::String(file_ref) = &sub_config["file"] {
                    self.add_file(file_ref, configs);
                }
            }
        }

        if let Yaml::Hash(docker_secrets) = &root["secrets"] {
            for secret in docker_secrets.values() {
                if let Yaml::String(file_ref) = &secret["file"] {
                    self.add_file(file_ref, configs);
                }
            }
        }

        // parse volumes names, to distinct virtual volumes from local folders
        let mut top_level_volumes = HashSet::new();
        if let Yaml::Hash(virtual_volumes) = &root["volumes"] {
            for vol_name in virtual_volumes.keys() {
                if let Yaml::String(name) = &vol_name {
                    top_level_volumes.insert(name.as_str());
                }
            }
        }

        if let Yaml::Array(docker_includes) = &root["include"] {
            for include in docker_includes {
                if let Yaml::String(file_ref) = include {
                    self.process_config_ref(configs, file_ref, f).await?;
                }
                else {
                    if let Yaml::Hash(include) = include {
                        match include.get(&Yaml::from_str("path")) {
                            Some(Yaml::String(file_ref)) => {
                                self.process_config_ref(configs, file_ref, f).await?;
                            },
                            Some(Yaml::Array(refs)) => {
                                for file_ref in refs {
                                    if let Yaml::String(file_ref) = file_ref {
                                        self.process_config_ref(configs, file_ref, f).await?;
                                    }
                                }
                            },
                            _ => {}
                        }

                        if let Some(Yaml::String(file_ref)) = include.get(&Yaml::from_str("path")) {
                            self.process_config_ref(configs, file_ref, f).await?;
                        }
                    }

                    self.process_env_section(configs, include);
                }
            }
        }

        if let Yaml::Hash(services) = &root["services"] {
            for service in services.values() {
                if let Yaml::Hash(credentials) = &service["credential_spec"] {
                    if let Some(Yaml::String(file_ref)) = credentials.get(&Yaml::from_str("file")) {
                        self.add_file(file_ref, configs);
                    }
                }

                self.process_env_section(configs, &service);

                if let Yaml::Hash(extends) = &service["extends"] {
                    if let Some(Yaml::String(file_ref)) = extends.get(&Yaml::from_str("file")) {
                        self.process_config_ref(configs, file_ref, f).await?;
                    }
                }

                if let Yaml::Array(service_volumes) = &service["volumes"] {
                    for volume in service_volumes {
                        if let Yaml::String(volume_spec) = volume {
                            let parts: Vec<_> = volume_spec.split(':').collect();
                            let volume_name = parts[0];
                            if top_level_volumes.contains(volume_name) {
                                continue;
                            }

                            let volume = configs.relative_path().join(volume_name);
                            self.volumes.insert(volume);
                        }
                    }
                }

                match &service["build"] {
                    Yaml::String(path) => {
                        if !path.starts_with("http://") && !path.starts_with("https://") {
                            let build_context = configs.relative_path().join(path);
                            let docker_file = build_context.join("Dockerfile");

                            self.build_contexts.insert(build_context);
                            self.files.insert(docker_file);
                        }
                    },
                    Yaml::Hash(build_spec) => {
                        if let Some(Yaml::String(context_path)) = build_spec.get(&Yaml::from_str("context")) {
                            if !context_path.starts_with("http://") && !context_path.starts_with("https://") {
                                let context_folder = configs.relative_path().join(context_path);
                                self.build_contexts.insert(context_folder.clone());

                                if let Some(Yaml::String(docker)) = build_spec.get(&Yaml::from_str("dockerfile")) {
                                    self.files.insert(context_folder.join(docker));
                                }
                            }
                        }
                    },
                    Yaml::BadValue => {},
                    _ => warn!("Unsupported build section"),
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    const COMPOSE: &str = 
r#"
name: parsing_test

configs:
  conf_name:
    file: ./ref_file_1.txt
  conf_name2:
    file: ./folder/ref_file_2.txt

secrets:
  named_secret:
    file: .\win_style\folder\ref_file_3.txt

volumes:
  vol1:
  vol3:

include:
  - ref_comp_1.yaml
  - path: ref_comp_2.yaml
    env_file:
      - env_1.env
      - env_2.env
  - ref_comp_3.yaml
  - path:
      - ref_comp_4.yaml
      - ref_comp_5.yaml
    env_file: ./sub_dir/env_4.env

services:
  srv_a:
    image: "a"
    volumes:
      - vol1:/var/lib/srv_a
      - vol2:/app/data # vol2 is not in volumes
      - ./ref_file_4.txt:/etc/config.conf
    credential_spec:
      file: ./cred/ref_file_5.txt

  srv_b:
    extends:
      file: .\other\service\ref_comp_6.yaml
      service: some_srv

  srv_c:
    build: ./app_folder/ctx_dir_1
    env_file:
      - ./other/dir/env_5.env      

  srv_d:
    build:
      context: app/folder/ctx_dir_2
      dockerfile: dock_file_1
    env_file: .\env_dir\env_6.env

  srv_e:
    build: https://ignore.this.path/

  srv_f:
    build: .
"#;

    use std::str::FromStr;
    use std::sync::{Once, OnceLock};
    use super::*;

    #[tokio::test]
    async fn parse_file() {
        let mut items_parser = ComposeReferencedItems::new();
        let initial = Once::new();
        let get_file_content = async |_: &Path| {
            let mut text = String::new();
            initial.call_once(|| {
                text = COMPOSE.to_owned()
            });
            Ok::<String, anyhow::Error>(text)
        };

        let compose = PathBuf::from_str("dir1/dir2/compose.yaml").unwrap();
        let configs = ConfigFilesInfo {
            remote_cfg_path: PathBuf::from_str("/root").unwrap(), 
            compose_cfg_path: compose.clone(), 
            relative_path: PathBuf::from_str("rel/path").unwrap(), 
            compose_path: compose.parent().unwrap().to_owned(),
            up: PathBuf::new(),
            compose_config: OnceLock::new(),
            remote_config: OnceLock::new(),
        };

        items_parser.parse_internal(&compose, &configs, &get_file_content).await.unwrap();

        let expected_files = vec![
            "compose.yaml", // root file

            "ref_file_1.txt",
            "ref_file_2.txt",
            "ref_file_3.txt",
            "ref_file_4.txt",
            "ref_file_5.txt",

            "ref_comp_1.yaml",
            "ref_comp_2.yaml",
            "ref_comp_3.yaml",
            "ref_comp_4.yaml",
            "ref_comp_5.yaml",
            "ref_comp_6.yaml",

            "env_1.env",
            "env_2.env",
            "env_4.env",
            "env_5.env",
            "env_6.env",

            "dock_file_1",
        ];

        let expected_dirs = vec![
            "ctx_dir_1",
            "ctx_dir_2",
            ".",
        ];

        let expected_volumes = vec![
            "vol2",
        ];

        println!("Parsed files:");
        for act_file in &items_parser.files {
            println!("  {act_file:?}");
        }
        println!();

        println!("Parsed dirs:");
        for act_dir in &items_parser.build_contexts {
            println!("  {act_dir:?}");
        }
        println!();

        'a: for exp_file in &expected_files {
            for act_file in items_parser.files.iter() {
                if act_file.ends_with(exp_file) {
                    continue 'a;
                }
            }

            println!("File {exp_file} was not parsed");
        }
        println!();

        'a: for exp_dir in &expected_dirs {
            for act_dir in &items_parser.build_contexts {
                if act_dir.ends_with(exp_dir) {
                    continue 'a;
                }
            }

            println!("Dir {exp_dir} was not parsed");
        }

        'a: for exp_vol in &expected_volumes {
            for act_vol in items_parser.volumes() {
                if act_vol.ends_with(exp_vol) {
                    continue 'a;
                }
            }
        }
    }
}
