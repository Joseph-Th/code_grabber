use std::fs;
use std::path::Path;

use crate::config::ScanConfig;
use crate::error::Result;

pub fn load_config(root: &Path) -> Result<ScanConfig> {
    let root = root.canonicalize()?;
    let profile_path = root.join(".codebundle.toml");
    let mut config = if profile_path.exists() {
        let contents = fs::read_to_string(profile_path)?;
        toml::from_str::<ScanConfig>(&contents)?
    } else {
        ScanConfig {
            root: root.clone(),
            ..ScanConfig::default()
        }
    };
    config.root = root.clone();
    if config.output.output_dir.is_relative() {
        config.output.output_dir = root.join(&config.output.output_dir);
    }
    config.finalize()
}

pub fn init_project_profile(root: &Path) -> Result<()> {
    let path = root.join(".codebundle.toml");
    let mut config = ScanConfig {
        root: Path::new(".").to_path_buf(),
        ..ScanConfig::default()
    };
    config.output.output_dir = Path::new(".").to_path_buf();
    config.exclude_globset = None;
    config.test_globset = None;
    let contents = toml::to_string_pretty(&config)?;
    fs::write(path, contents)?;
    Ok(())
}
