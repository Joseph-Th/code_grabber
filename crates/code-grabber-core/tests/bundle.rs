use std::fs;

use code_grabber_core::{BundleMode, ScanConfigBuilder, generate_bundle};

#[test]
fn bundle_excludes_tests_and_lockfiles_by_default() {
    let root = unique_temp_dir();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(root.join("Cargo.lock"), "lock noise").unwrap();
    fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
    fs::write(
        root.join("tests").join("smoke.rs"),
        "#[test]\nfn smoke() {}\n",
    )
    .unwrap();

    let config = ScanConfigBuilder::new(&root)
        .mode(BundleMode::Core)
        .build()
        .unwrap();
    let result = generate_bundle(&config).unwrap();

    assert!(result.output.contents.contains("src/main.rs"));
    assert!(result.output.contents.contains("Cargo.toml"));
    assert!(result.output.contents.contains("Cargo.lock |"));
    assert!(result.output.contents.contains("tests/smoke.rs |"));
    assert!(
        !result
            .output
            .contents
            .contains("<<<FILE path=\"Cargo.lock\"")
    );
    assert!(
        !result
            .output
            .contents
            .contains("<<<FILE path=\"tests/smoke.rs\"")
    );
}

#[test]
fn include_rule_toggles_exclude_supported_file_groups() {
    let root = unique_temp_dir();
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();
    fs::write(root.join("settings.toml"), "enabled = true\n").unwrap();
    fs::write(
        root.join("app.tsx"),
        "export function App() { return null; }\n",
    )
    .unwrap();
    fs::write(root.join("build.ps1"), "Write-Output build\n").unwrap();
    fs::write(root.join("notes.txt"), "plain text\n").unwrap();

    let mut config = ScanConfigBuilder::new(&root).build().unwrap();
    config.include_rules.manifests = false;
    config.include_rules.configs = false;
    config.include_rules.frontend = false;
    config.include_rules.scripts = false;

    let result = generate_bundle(&config).unwrap();

    assert!(
        !result
            .output
            .contents
            .contains("<<<FILE path=\"Cargo.toml\"")
    );
    assert!(
        !result
            .output
            .contents
            .contains("<<<FILE path=\"settings.toml\"")
    );
    assert!(!result.output.contents.contains("<<<FILE path=\"app.tsx\""));
    assert!(
        !result
            .output
            .contents
            .contains("<<<FILE path=\"build.ps1\"")
    );
    assert!(
        result
            .output
            .contents
            .contains("<<<FILE path=\"notes.txt\"")
    );
    assert!(
        result
            .output
            .contents
            .contains("Cargo.toml | 24 bytes | excluded: manifests disabled")
    );
    assert!(
        result
            .output
            .contents
            .contains("settings.toml | 15 bytes | excluded: configs disabled")
    );
    assert!(
        result
            .output
            .contents
            .contains("app.tsx | 39 bytes | excluded: frontend files disabled")
    );
    assert!(
        result
            .output
            .contents
            .contains("build.ps1 | 19 bytes | excluded: scripts disabled")
    );
}

fn unique_temp_dir() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "code-grabber-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}
