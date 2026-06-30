use std::fs;
use std::path::Path;

use crate::config::ScanConfig;
use crate::error::Result;

pub fn load_config(root: &Path) -> Result<ScanConfig> {
    let profile_path = root.join(".codebundle.toml");
    let mut config = if profile_path.exists() {
        let contents = fs::read_to_string(profile_path)?;
        toml::from_str::<ScanConfig>(&contents)?
    } else {
        ScanConfig {
            root: root.to_path_buf(),
            ..ScanConfig::default()
        }
    };
    config.root = root.to_path_buf();
    config.finalize()
}

pub fn init_project_profile(root: &Path) -> Result<()> {
    let path = root.join(".codebundle.toml");
    let mut config = ScanConfig {
        root: root.to_path_buf(),
        ..ScanConfig::default()
    };
    config.exclude_globset = None;
    config.test_globset = None;
    let contents = toml::to_string_pretty(&config)?;
    fs::write(path, contents)?;
    Ok(())
}
