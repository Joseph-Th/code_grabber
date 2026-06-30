use std::fs;

use crate::classify::FileClassification;
use crate::config::ScanConfig;
use crate::discover::FileCandidate;
use crate::error::Result;
use crate::sanitize::sanitize_content;
use crate::tokenize::estimate_tokens;

#[derive(Debug, Clone)]
pub struct ExtractedFile {
    pub rel_path: String,
    pub language: Option<String>,
    pub original_bytes: usize,
    pub output_bytes: usize,
    pub estimated_tokens: usize,
    pub transformations: Vec<String>,
    pub content: String,
}

pub fn extract_file(
    config: &ScanConfig,
    candidate: &FileCandidate,
    classification: &FileClassification,
) -> Result<ExtractedFile> {
    let raw = fs::read(&candidate.abs_path)?;
    let original_bytes = raw.len();
    let content = String::from_utf8_lossy(&raw).to_string();
    let (content, transformations) = sanitize_content(config, &content, classification, candidate);
    let output_bytes = content.len();
    let estimated_tokens = estimate_tokens(&config.tokenizer, &content)?;

    Ok(ExtractedFile {
        rel_path: candidate.rel_path.clone(),
        language: classification.language.clone(),
        original_bytes,
        output_bytes,
        estimated_tokens,
        transformations,
        content,
    })
}
