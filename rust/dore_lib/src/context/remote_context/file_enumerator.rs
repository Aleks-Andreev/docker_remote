use std::path::{Path, PathBuf};
use std::collections::HashMap;

use anyhow::Result;
use log::{debug, info};

use crate::config_files::RemoteConfig;
use crate::config_files::ComposeReferencedItems;

mod trie;
use trie::Trie;

pub(crate) fn enumerate_files_for_copy(
    from_cwd: &Path,
    remote_config: &RemoteConfig,
    compose_items: &ComposeReferencedItems
) -> Result<HashMap<PathBuf, PathBuf>>
{
    let mut additional_folders = vec![];

    let mut files_to_transfer= HashMap::new();

    let manual_items: Vec<_> = remote_config.included_files().map(PathBuf::from).collect();
    let files_or_folders = compose_items.volumes()
        .chain(compose_items.files())
        .chain(manual_items.iter());

    for item in files_or_folders {
        let fs_path = from_cwd.join(item);
        debug!("Checking item {item:?} by path {fs_path:?}");

        if fs_path.exists() {
            if fs_path.is_file() {
                let Ok(fs_path) = fs_path.canonicalize() else { unreachable!("file exists") };
                debug!("File {item:?} added");
                files_to_transfer.insert(fs_path, item.to_owned());
            }
            else if fs_path.is_dir() {
                debug!("Dir {item:?} will be added");
                additional_folders.push(item);
            }
        }
        else {
            info!("Item {item:?} is referenced by config but missing from disk");
        }
    }

    let mut file_filter = Trie::init(from_cwd, remote_config.excluded_files())?;
    let folders = compose_items.build_contexts()
        .chain(additional_folders);
    
    for folder in folders{
        if let Ok(file_filter) = file_filter.fold_path(folder) {
            let fs_path = from_cwd.join(folder);
            debug!("Search for files in dir {folder:?} by path {fs_path:?}");
            collect_files_from_folder(file_filter, folder, fs_path, &mut files_to_transfer)?;
        }
        else {
            info!("Folder '{folder:?}' is referenced by config but missing from disk");
        }
    }

    Ok(files_to_transfer)
}

fn collect_files_from_folder(
    filter: &mut Trie,
    folder: &PathBuf,
    fs_path: PathBuf,
    dst: &mut HashMap<PathBuf, PathBuf>
) -> Result<()>
{
    debug!("Search in {fs_path:?}, referenced as {folder:?}");

    for entry in fs_path.read_dir()? {
        let path = entry?.path();
        if let Some(fname) = path.file_name() {
            if filter.should_ignore(fname) {
                continue;
            }

            if path.is_file() {
                let Ok(fs_path) = path.canonicalize() else { unreachable!("is_file is true") };
                let fname = path.file_name().unwrap();
                let value = folder.join(fname);
                debug!("File '{path:?}' added as {value:?}");
                dst.insert(fs_path, value);
            }
            else if path.is_dir() {
                let Some(fname) = path.file_name() else { unreachable!("is_dir is true") };
                let folder = folder.join(fname);
                collect_files_from_folder(filter.append_dir(fname)?, &folder, path, dst)?
            }
        }
    }

    Ok(())
}
