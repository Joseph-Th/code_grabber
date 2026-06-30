use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{OutputFormat, ScanConfig};
use crate::error::Result;
use crate::extract::ExtractedFile;
use crate::report::BundleReport;

#[derive(Debug, Clone)]
pub struct BundleOutput {
    pub contents: String,
}

pub fn format_bundle(
    config: &ScanConfig,
    files: &[ExtractedFile],
    report: &BundleReport,
) -> Result<BundleOutput> {
    match config.output.output_format {
        OutputFormat::PlainDelimited => Ok(BundleOutput {
            contents: format_plain(config, files, report),
        }),
        OutputFormat::Markdown => Ok(BundleOutput {
            contents: format_plain(config, files, report),
        }),
    }
}

fn format_plain(config: &ScanConfig, files: &[ExtractedFile], report: &BundleReport) -> String {
    let mut out = String::new();
    let project = config
        .root
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();

    writeln!(out, "# CODEBASE BUNDLE v1").ok();
    writeln!(out, "project: {project}").ok();
    writeln!(out, "root: {}", display_path(&config.root)).ok();
    writeln!(out, "generated_unix: {}", now_unix()).ok();
    writeln!(out, "mode: {:?}", config.mode).ok();
    writeln!(out, "tokenizer: {}", config.tokenizer).ok();
    writeln!(out, "estimated_tokens: {}", report.total_tokens).ok();
    writeln!(out, "included_files: {}", report.included_files.len()).ok();
    writeln!(out, "excluded_files: {}", report.excluded_files.len()).ok();
    if let Some(budget) = config.token_budget {
        writeln!(out, "token_budget: {budget}").ok();
        if report.total_tokens > budget {
            writeln!(out, "budget_overflow: {}", report.total_tokens - budget).ok();
        }
    }
    writeln!(out).ok();

    writeln!(out, "# REPOSITORY MAP").ok();
    out.push_str(&repository_map(report));
    writeln!(out).ok();

    writeln!(out, "# FILE MANIFEST").ok();
    for file in &report.included_files {
        let lang = file.language.as_deref().unwrap_or("text");
        writeln!(
            out,
            "{} | {} | {} bytes | {} tokens | included",
            file.path, lang, file.bytes, file.tokens
        )
        .ok();
    }
    for file in &report.excluded_files {
        writeln!(
            out,
            "{} | {} bytes | excluded: {}",
            file.path,
            file.bytes,
            file.reason.as_deref().unwrap_or("excluded")
        )
        .ok();
    }
    writeln!(out).ok();

    for file in files {
        let lang = file.language.as_deref().unwrap_or("text");
        writeln!(
            out,
            "<<<FILE path=\"{}\" lang=\"{}\" bytes={} tokens={}>>>",
            escape_attr(&file.rel_path),
            lang,
            file.output_bytes,
            file.estimated_tokens
        )
        .ok();
        out.push_str(&file.content);
        if !file.content.ends_with('\n') {
            out.push('\n');
        }
        writeln!(out, "<<<END_FILE>>>").ok();
        writeln!(out).ok();
    }

    out
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn escape_attr(value: &str) -> String {
    value.replace('"', "&quot;")
}

fn display_path(path: &Path) -> String {
    let raw = path.display().to_string();
    if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = raw.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        raw
    }
}

fn repository_map(report: &BundleReport) -> String {
    let mut top_level: BTreeMap<String, (usize, usize, Option<String>)> = BTreeMap::new();
    for file in &report.included_files {
        let key = top_key(&file.path);
        let entry = top_level.entry(key).or_insert((0, 0, None));
        entry.0 += 1;
    }
    for file in &report.excluded_files {
        let key = top_key(&file.path);
        let entry = top_level.entry(key).or_insert((0, 0, file.reason.clone()));
        entry.1 += 1;
        if entry.2.is_none() {
            entry.2 = file.reason.clone();
        }
    }

    let mut out = String::new();
    for (path, (included, excluded, reason)) in top_level {
        match (included, excluded) {
            (0, excluded) => {
                writeln!(
                    out,
                    "{path} excluded: {} ({excluded} files)",
                    reason.unwrap_or_else(|| "excluded".to_string())
                )
                .ok();
            }
            (included, 0) => {
                writeln!(out, "{path} included: {included} files").ok();
            }
            (included, excluded) => {
                writeln!(
                    out,
                    "{path} included: {included} files, excluded: {excluded} files"
                )
                .ok();
            }
        }
    }
    out
}

fn top_key(path: &str) -> String {
    path.split('/')
        .next()
        .map(|part| {
            if path.contains('/') {
                format!("{part}/")
            } else {
                part.to_string()
            }
        })
        .unwrap_or_else(|| path.to_string())
}
