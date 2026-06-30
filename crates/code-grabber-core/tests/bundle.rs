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
