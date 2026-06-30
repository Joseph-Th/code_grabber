use std::path::PathBuf;

use ignore::{DirEntry, WalkBuilder};

use crate::config::ScanConfig;
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct FileCandidate {
    pub abs_path: PathBuf,
    pub rel_path: String,
    pub size_bytes: u64,
    pub extension: Option<String>,
    pub file_name: String,
    pub hidden: bool,
}

pub fn discover_files(config: &ScanConfig) -> Result<Vec<FileCandidate>> {
    let mut entries = Vec::new();
    let root = config.root.clone();

    let mut walker = WalkBuilder::new(&root);
    walker
        .standard_filters(true)
        .hidden(!config.include_rules.hidden_files)
        .filter_entry({
            let excluded_dirs = config.exclude_rules.dirs.clone();
            move |entry| should_descend(entry, &excluded_dirs)
        });

    for result in walker.build() {
        let entry = result?;
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let metadata = entry.metadata()?;
        let abs_path = entry.path().to_path_buf();
        let rel_path = abs_path
            .strip_prefix(&config.root)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");
        let file_name = entry.file_name().to_string_lossy().to_string();
        let extension = entry
            .path()
            .extension()
            .map(|ext| ext.to_string_lossy().to_ascii_lowercase());
        let hidden = rel_path.split('/').any(|part| part.starts_with('.'));
        entries.push(FileCandidate {
            abs_path,
            rel_path,
            size_bytes: metadata.len(),
            extension,
            file_name,
            hidden,
        });
    }

    entries.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(entries)
}

fn should_descend(entry: &DirEntry, excluded_dirs: &[String]) -> bool {
    if !entry.file_type().is_some_and(|ft| ft.is_dir()) {
        return true;
    }
    let Some(name) = entry.file_name().to_str() else {
        return true;
    };
    !excluded_dirs.iter().any(|dir| dir == name)
}
