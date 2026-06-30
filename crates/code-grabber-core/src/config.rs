use std::path::PathBuf;

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleMode {
    Complete,
    Core,
    Compact,
    Map,
    Diff,
    Pinned,
}

impl Default for BundleMode {
    fn default() -> Self {
        Self::Core
    }
}

impl std::str::FromStr for BundleMode {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "complete" => Ok(Self::Complete),
            "core" => Ok(Self::Core),
            "compact" => Ok(Self::Compact),
            "map" => Ok(Self::Map),
            "diff" => Ok(Self::Diff),
            "pinned" => Ok(Self::Pinned),
            _ => Err(format!("unknown mode '{value}'")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    PlainDelimited,
    Markdown,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::PlainDelimited
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncludeRules {
    pub unknown_text_files: bool,
    pub manifests: bool,
    pub configs: bool,
    pub frontend: bool,
    pub scripts: bool,
    pub docs: bool,
    pub tests: bool,
    pub lockfiles: bool,
    pub hidden_files: bool,
}

impl Default for IncludeRules {
    fn default() -> Self {
        Self {
            unknown_text_files: true,
            manifests: true,
            configs: true,
            frontend: true,
            scripts: true,
            docs: false,
            tests: false,
            lockfiles: false,
            hidden_files: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcludeRules {
    pub dirs: Vec<String>,
    pub globs: Vec<String>,
    pub test_globs: Vec<String>,
    pub max_file_bytes: u64,
}

impl Default for ExcludeRules {
    fn default() -> Self {
        Self {
            dirs: [
                ".git",
                "node_modules",
                "target",
                "dist",
                "build",
                ".next",
                ".nuxt",
                ".svelte-kit",
                ".cache",
                "coverage",
                "venv",
                ".venv",
                "__pycache__",
                "vendor",
                "fixtures",
                "mocks",
                "snapshots",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            globs: [
                "**/*.lock",
                "**/package-lock.json",
                "**/pnpm-lock.yaml",
                "**/yarn.lock",
                "**/bun.lockb",
                "**/*.map",
                "**/*.min.js",
                "**/*.min.css",
                "**/generated/**",
                "**/__generated__/**",
                "**/*.generated.*",
                "**/*.gen.*",
                "**/.env",
                "**/.env.*",
                "**/*.pem",
                "**/*.key",
                "**/*.p12",
                "**/*.pfx",
                "**/id_rsa",
                "**/id_ed25519",
                "**/credentials.json",
                "**/secrets.*",
                "**/.gitignore",
                "**/codebase_bundle*.txt",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            test_globs: [
                "**/test/**",
                "**/tests/**",
                "**/__tests__/**",
                "**/testing/**",
                "**/spec/**",
                "**/specs/**",
                "**/e2e/**",
                "**/integration_tests/**",
                "**/test_*",
                "**/*_test.*",
                "**/*_tests.*",
                "**/*.test.*",
                "**/*.spec.*",
                "**/*_spec.*",
                "**/*_fixture.*",
                "**/*.fixture.*",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            max_file_bytes: 1_000_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripRules {
    pub rust_inline_tests: bool,
    pub debug_assertion_cfg: bool,
    pub comments: bool,
    pub blank_lines: bool,
}

impl Default for StripRules {
    fn default() -> Self {
        Self {
            rust_inline_tests: true,
            debug_assertion_cfg: true,
            comments: false,
            blank_lines: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub output_dir: PathBuf,
    pub output_filename: String,
    pub output_format: OutputFormat,
    pub copy_to_clipboard: bool,
    pub write_to_file: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            output_dir: dirs::download_dir().unwrap_or_else(|| PathBuf::from(".")),
            output_filename: "codebase_bundle.txt".to_string(),
            output_format: OutputFormat::PlainDelimited,
            copy_to_clipboard: false,
            write_to_file: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    pub root: PathBuf,
    pub mode: BundleMode,
    pub token_budget: Option<usize>,
    pub tokenizer: String,
    pub include_rules: IncludeRules,
    pub exclude_rules: ExcludeRules,
    pub strip_rules: StripRules,
    pub output: OutputConfig,

    #[serde(skip)]
    pub exclude_globset: Option<GlobSet>,
    #[serde(skip)]
    pub test_globset: Option<GlobSet>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            mode: BundleMode::default(),
            token_budget: Some(250_000),
            tokenizer: "o200k_base".to_string(),
            include_rules: IncludeRules::default(),
            exclude_rules: ExcludeRules::default(),
            strip_rules: StripRules::default(),
            output: OutputConfig::default(),
            exclude_globset: None,
            test_globset: None,
        }
    }
}

impl ScanConfig {
    pub fn finalize(mut self) -> Result<Self> {
        self.root = self.root.canonicalize()?;
        self.exclude_globset = Some(build_globset(&self.exclude_rules.globs)?);
        self.test_globset = Some(build_globset(&self.exclude_rules.test_globs)?);
        Ok(self)
    }

    pub fn output_path(&self) -> PathBuf {
        self.output.output_dir.join(&self.output.output_filename)
    }
}

#[derive(Debug, Default)]
pub struct ScanConfigBuilder {
    config: ScanConfig,
}

impl ScanConfigBuilder {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            config: ScanConfig {
                root: root.into(),
                ..ScanConfig::default()
            },
        }
    }

    pub fn mode(mut self, mode: BundleMode) -> Self {
        self.config.mode = mode;
        self
    }

    pub fn token_budget(mut self, budget: Option<usize>) -> Self {
        self.config.token_budget = budget;
        self
    }

    pub fn output_path(mut self, path: Option<PathBuf>) -> Self {
        if let Some(path) = path {
            self.config.output.output_dir =
                path.parent().unwrap_or_else(|| ".".as_ref()).to_path_buf();
            self.config.output.output_filename = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "codebase_bundle.txt".to_string());
        }
        self
    }

    pub fn include_tests(mut self, value: bool) -> Self {
        self.config.include_rules.tests = value;
        self
    }

    pub fn include_docs(mut self, value: bool) -> Self {
        self.config.include_rules.docs = value;
        self
    }

    pub fn include_lockfiles(mut self, value: bool) -> Self {
        self.config.include_rules.lockfiles = value;
        self
    }

    pub fn include_hidden(mut self, value: bool) -> Self {
        self.config.include_rules.hidden_files = value;
        self
    }

    pub fn strip_comments(mut self, value: bool) -> Self {
        self.config.strip_rules.comments = value;
        self
    }

    pub fn copy_to_clipboard(mut self, value: bool) -> Self {
        self.config.output.copy_to_clipboard = value;
        self
    }

    pub fn write_to_file(mut self, value: bool) -> Self {
        self.config.output.write_to_file = value;
        self
    }

    pub fn build(self) -> Result<ScanConfig> {
        self.config.finalize()
    }
}

pub fn build_globset(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern)?);
    }
    Ok(builder.build()?)
}
