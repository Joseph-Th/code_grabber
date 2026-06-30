use crate::format::BundleOutput;

#[derive(Debug, Clone, Default)]
pub struct BundleReport {
    pub included_files: Vec<FileReport>,
    pub excluded_files: Vec<FileReport>,
    pub total_bytes: usize,
    pub total_tokens: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FileReport {
    pub path: String,
    pub bytes: usize,
    pub tokens: usize,
    pub language: Option<String>,
    pub status: FileStatus,
    pub reason: Option<String>,
}

impl FileReport {
    pub fn included(path: String, bytes: usize, tokens: usize, language: Option<String>) -> Self {
        Self {
            path,
            bytes,
            tokens,
            language,
            status: FileStatus::Included,
            reason: None,
        }
    }

    pub fn excluded(path: String, bytes: usize, language: Option<String>, reason: String) -> Self {
        Self {
            path,
            bytes,
            tokens: 0,
            language,
            status: FileStatus::Excluded,
            reason: Some(reason),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Included,
    Excluded,
}

#[derive(Debug, Clone)]
pub struct GenerationResult {
    pub output: BundleOutput,
    pub report: BundleReport,
}
