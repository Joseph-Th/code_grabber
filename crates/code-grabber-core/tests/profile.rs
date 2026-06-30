use std::fs;

use code_grabber_core::{BundleMode, init_project_profile, load_config};

#[test]
fn partial_profile_uses_defaults_for_missing_fields() {
    let root = unique_temp_dir("partial-profile");
    fs::write(
        root.join(".codebundle.toml"),
        r#"
mode = "complete"
token_budget = 12345

[include_rules]
docs = true

[output]
output_filename = "custom_bundle.txt"
"#,
    )
    .unwrap();

    let config = load_config(&root).unwrap();

    assert_eq!(config.mode, BundleMode::Complete);
    assert_eq!(config.token_budget, Some(12345));
    assert!(config.include_rules.docs);
    assert!(!config.include_rules.tests);
    assert_eq!(config.output.output_filename, "custom_bundle.txt");
    assert!(config.output.write_to_file);
    assert!(config.exclude_globset.is_some());
    assert!(config.test_globset.is_some());
}

#[test]
fn invalid_profile_returns_an_error() {
    let root = unique_temp_dir("invalid-profile");
    fs::write(root.join(".codebundle.toml"), "mode = \"not-a-mode\"\n").unwrap();

    let err = load_config(&root).unwrap_err().to_string();

    assert!(err.contains("TOML parse error"));
}

#[test]
fn initialized_profile_uses_repo_relative_output_dir() {
    let root = unique_temp_dir("init-profile");

    init_project_profile(&root).unwrap();

    let contents = fs::read_to_string(root.join(".codebundle.toml")).unwrap();
    assert!(contents.contains("root = \".\""));
    assert!(contents.contains("output_dir = \".\""));
    assert!(!contents.contains("Downloads"));
    assert!(!contents.contains(&root.display().to_string()));
}

#[test]
fn profile_relative_output_dir_resolves_from_repo_root() {
    let root = unique_temp_dir("relative-output");
    fs::write(
        root.join(".codebundle.toml"),
        r#"
[output]
output_dir = "bundles"
output_filename = "repo.txt"
"#,
    )
    .unwrap();

    let config = load_config(&root).unwrap();

    assert_eq!(
        config.output_path(),
        root.canonicalize()
            .unwrap()
            .join("bundles")
            .join("repo.txt")
    );
}

fn unique_temp_dir(label: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "code-grabber-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}
