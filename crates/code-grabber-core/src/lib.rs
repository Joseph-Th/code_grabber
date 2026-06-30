pub mod classify;
pub mod config;
pub mod discover;
pub mod error;
pub mod extract;
pub mod format;
pub mod profile;
pub mod report;
pub mod sanitize;
pub mod tokenize;

use std::fs;
use std::path::Path;

use crate::classify::classify_candidate;
use crate::discover::discover_files;
use crate::extract::extract_file;
use crate::format::format_bundle;
use crate::report::{BundleReport, FileReport};

pub use crate::config::{BundleMode, OutputFormat, ScanConfig, ScanConfigBuilder};
pub use crate::error::{CodeGrabberError, Result};
pub use crate::format::BundleOutput;
pub use crate::profile::{init_project_profile, load_config};
pub use crate::report::GenerationResult;

pub fn generate_bundle(config: &ScanConfig) -> Result<GenerationResult> {
    let candidates = discover_files(config)?;
    let mut report = BundleReport::default();
    let mut extracted = Vec::new();

    for candidate in candidates {
        let classification = classify_candidate(config, &candidate)?;
        if !classification.include {
            report.excluded_files.push(FileReport::excluded(
                candidate.rel_path,
                candidate.size_bytes as usize,
                classification.language,
                classification.reasons.join("; "),
            ));
            continue;
        }

        match extract_file(config, &candidate, &classification) {
            Ok(file) => {
                report.total_bytes += file.output_bytes;
                report.total_tokens += file.estimated_tokens;
                report.included_files.push(FileReport::included(
                    file.rel_path.clone(),
                    file.output_bytes,
                    file.estimated_tokens,
                    file.language.clone(),
                ));
                extracted.push(file);
            }
            Err(err) => {
                report.excluded_files.push(FileReport::excluded(
                    candidate.rel_path,
                    candidate.size_bytes as usize,
                    classification.language,
                    err.to_string(),
                ));
            }
        }
    }

    extracted.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    report.included_files.sort_by(|a, b| a.path.cmp(&b.path));
    report.excluded_files.sort_by(|a, b| a.path.cmp(&b.path));

    let output = format_bundle(config, &extracted, &report)?;
    Ok(GenerationResult { output, report })
}

pub fn write_bundle_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;
    Ok(())
}
