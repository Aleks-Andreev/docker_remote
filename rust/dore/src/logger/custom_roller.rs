use std::path::Path;
use std::fs::File;

use flate2::write::GzEncoder;
use flate2::Compression;
use log4rs::append::rolling_file::policy::compound::roll::Roll;

#[derive(Debug)]
pub(crate) struct CustomFileRoller {
    base_path: String,
    max_uncompressed: usize,
    max_total: usize,
}

impl CustomFileRoller {
    pub fn new(base_path: &str) -> Self {
        Self {
            base_path: base_path.to_string(),
            max_uncompressed: 3,
            max_total: 6,
        }
    }

    fn get_file_name(&self, i: usize) -> String {
        let current_ext = if i <= self.max_uncompressed { "log" } else { "log.gz" };
        let current_idx = if i > 1 { format!(".{}", i - 1) } else { String::new() };
        let current_path = format!("{}{}.{}", self.base_path, current_idx, current_ext);
        current_path
    }

    pub fn get_default_file_name(&self) -> String {
        self.get_file_name(1)
    }
}

impl Roll for CustomFileRoller {
    fn roll(&self, _file: &Path) -> anyhow::Result<()> {
        for i in (1..=self.max_total).rev() {
            let current_path = self.get_file_name(i);
            let current_path = Path::new(&current_path);

            if !current_path.exists() {
                continue;
            }

            if i == self.max_total {
                std::fs::remove_file(current_path)?; // drop last archive
                continue;
            }
            
            let next_path = self.get_file_name(i + 1);
            let next_path = Path::new(&next_path);

            if i == self.max_uncompressed {
                let mut text_file = File::open(current_path)?;
                let gz_file = File::create(next_path)?;
                let mut encoder = GzEncoder::new(gz_file, Compression::default());
                std::io::copy(&mut text_file, &mut encoder)?;
                std::fs::remove_file(current_path)?;
            }
            else {
                std::fs::rename(current_path, next_path)?;
            }
        }

        Ok(())
    }
}