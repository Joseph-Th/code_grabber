use code_grabber_core::classify::{FileClassification, FileKind};
use code_grabber_core::config::ScanConfig;
use code_grabber_core::discover::FileCandidate;
use code_grabber_core::sanitize::sanitize_content;
use std::path::PathBuf;

#[test]
fn strips_rust_test_functions_without_touching_production_code() {
    let config = ScanConfig::default();
    let classification = FileClassification {
        kind: FileKind::Code,
        language: Some("rust".to_string()),
        include: true,
        priority: 70,
        reasons: vec![],
    };
    let candidate = FileCandidate {
        abs_path: PathBuf::from("src/lib.rs"),
        rel_path: "src/lib.rs".to_string(),
        size_bytes: 0,
        extension: Some("rs".to_string()),
        file_name: "lib.rs".to_string(),
        hidden: false,
    };
    let input = r#"
pub fn keep() -> bool {
    true
}

#[test]
fn removes_me() {
    assert!(keep());
}
"#;

    let (output, transformations) = sanitize_content(&config, input, &classification, &candidate);

    assert!(output.contains("pub fn keep()"));
    assert!(!output.contains("fn removes_me()"));
    assert!(
        transformations
            .iter()
            .any(|item| item == "stripped rust inline tests")
    );
}
