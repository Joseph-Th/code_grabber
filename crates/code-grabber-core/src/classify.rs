use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::config::{BundleMode, ScanConfig};
use crate::discover::FileCandidate;
use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileKind {
    Code,
    Config,
    Manifest,
    Documentation,
    Test,
    Generated,
    Dependency,
    Binary,
    Lockfile,
    Secret,
    UnknownText,
}

#[derive(Debug, Clone)]
pub struct FileClassification {
    pub kind: FileKind,
    pub language: Option<String>,
    pub include: bool,
    pub priority: i32,
    pub reasons: Vec<String>,
}

pub fn classify_candidate(
    config: &ScanConfig,
    candidate: &FileCandidate,
) -> Result<FileClassification> {
    let rel = candidate.rel_path.as_str();
    let mut reasons = Vec::new();
    let language = language_for(candidate);

    if is_configured_output_file(config, candidate) {
        return Ok(excluded(
            FileKind::Generated,
            language,
            "configured output file",
        ));
    }

    if candidate.hidden && !config.include_rules.hidden_files {
        return Ok(excluded(FileKind::Config, language, "hidden file"));
    }

    if matches_exclude(config, rel) {
        let kind = glob_exclusion_kind(candidate, rel);
        let allowed_override = matches!(kind, FileKind::Lockfile) && config.include_rules.lockfiles
            || matches!(kind, FileKind::Generated) && config.mode == BundleMode::Complete;
        if !allowed_override {
            return Ok(excluded(kind, language, "excluded by default glob"));
        }
    }

    if is_test_path(config, rel) && !config.include_rules.tests {
        return Ok(excluded(FileKind::Test, language, "test path or filename"));
    }

    if candidate.size_bytes > config.exclude_rules.max_file_bytes {
        return Ok(excluded(FileKind::Binary, language, "too large"));
    }

    if is_binary(candidate)? {
        return Ok(excluded(
            FileKind::Binary,
            language,
            "binary or non-text file",
        ));
    }

    if is_lockfile(candidate) && !config.include_rules.lockfiles {
        return Ok(excluded(FileKind::Lockfile, language, "lockfile"));
    }

    if is_generated_path(rel) && config.mode != BundleMode::Complete {
        return Ok(excluded(FileKind::Generated, language, "generated file"));
    }

    if is_secret_like(candidate) {
        return Ok(excluded(FileKind::Secret, language, "secret-like filename"));
    }

    let kind = classify_text(candidate);
    match kind {
        FileKind::Manifest if !config.include_rules.manifests => {
            Ok(excluded(kind, language, "manifests disabled"))
        }
        FileKind::Config if !config.include_rules.configs => {
            Ok(excluded(kind, language, "configs disabled"))
        }
        FileKind::Documentation if !config.include_rules.docs => {
            reasons.push("docs disabled in current mode".to_string());
            Ok(FileClassification {
                kind,
                language,
                include: false,
                priority: 10,
                reasons,
            })
        }
        FileKind::UnknownText if !config.include_rules.unknown_text_files => {
            Ok(excluded(kind, language, "unknown text files disabled"))
        }
        FileKind::Code if is_script(candidate) && !config.include_rules.scripts => {
            Ok(excluded(kind, language, "scripts disabled"))
        }
        FileKind::Code if is_frontend(candidate) && !config.include_rules.frontend => {
            Ok(excluded(kind, language, "frontend files disabled"))
        }
        _ => Ok(FileClassification {
            priority: priority_for(candidate, &kind),
            kind,
            language,
            include: true,
            reasons: vec!["included".to_string()],
        }),
    }
}

fn excluded(kind: FileKind, language: Option<String>, reason: &str) -> FileClassification {
    FileClassification {
        kind,
        language,
        include: false,
        priority: 0,
        reasons: vec![reason.to_string()],
    }
}

fn matches_exclude(config: &ScanConfig, rel: &str) -> bool {
    config
        .exclude_globset
        .as_ref()
        .is_some_and(|globset| globset.is_match(rel))
}

fn is_configured_output_file(config: &ScanConfig, candidate: &FileCandidate) -> bool {
    let output_path = config.output_path();
    let Ok(rel_path) = output_path.strip_prefix(&config.root) else {
        return false;
    };
    normalize_rel_path(rel_path) == candidate.rel_path
}

fn normalize_rel_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn glob_exclusion_kind(candidate: &FileCandidate, rel: &str) -> FileKind {
    if is_lockfile(candidate) {
        FileKind::Lockfile
    } else if is_secret_like(candidate) {
        FileKind::Secret
    } else if is_generated_path(rel) {
        FileKind::Generated
    } else {
        FileKind::Dependency
    }
}

fn is_test_path(config: &ScanConfig, rel: &str) -> bool {
    config
        .test_globset
        .as_ref()
        .is_some_and(|globset| globset.is_match(rel))
}

fn is_binary(candidate: &FileCandidate) -> Result<bool> {
    if is_binary_extension(candidate.extension.as_deref()) {
        return Ok(true);
    }
    let mut file = File::open(&candidate.abs_path)?;
    let mut buf = [0_u8; 8192];
    let bytes = file.read(&mut buf)?;
    if bytes == 0 {
        return Ok(false);
    }
    if buf[..bytes].contains(&0) {
        return Ok(true);
    }
    let invalid = std::str::from_utf8(&buf[..bytes]).is_err();
    Ok(invalid)
}

fn is_binary_extension(ext: Option<&str>) -> bool {
    matches!(
        ext,
        Some(
            "png"
                | "jpg"
                | "jpeg"
                | "gif"
                | "webp"
                | "ico"
                | "bmp"
                | "tiff"
                | "ttf"
                | "otf"
                | "woff"
                | "woff2"
                | "mp3"
                | "wav"
                | "mp4"
                | "mov"
                | "webm"
                | "zip"
                | "tar"
                | "gz"
                | "bz2"
                | "xz"
                | "7z"
                | "rar"
                | "exe"
                | "dll"
                | "so"
                | "dylib"
                | "class"
                | "jar"
                | "wasm"
                | "sqlite"
                | "db"
                | "duckdb"
                | "pdf"
        )
    )
}

fn is_lockfile(candidate: &FileCandidate) -> bool {
    matches!(
        candidate.file_name.as_str(),
        "Cargo.lock" | "package-lock.json" | "pnpm-lock.yaml" | "yarn.lock" | "bun.lockb"
    ) || candidate.extension.as_deref() == Some("lock")
}

fn is_secret_like(candidate: &FileCandidate) -> bool {
    let name = candidate.file_name.as_str();
    name == ".env"
        || name.starts_with(".env.")
        || matches!(
            candidate.extension.as_deref(),
            Some("pem" | "key" | "p12" | "pfx")
        )
        || matches!(name, "id_rsa" | "id_ed25519" | "credentials.json")
        || name.starts_with("secrets.")
}

fn is_generated_path(rel: &str) -> bool {
    rel.contains("/generated/")
        || rel.contains("/__generated__/")
        || rel.contains(".generated.")
        || rel.contains(".gen.")
        || rel.contains(".pb.")
        || rel.contains("openapi-generated")
        || rel.contains("schema.generated")
}

fn classify_text(candidate: &FileCandidate) -> FileKind {
    let name = candidate.file_name.as_str();
    if matches!(
        name,
        "Cargo.toml" | "package.json" | "pyproject.toml" | "go.mod" | "go.sum"
    ) {
        return FileKind::Manifest;
    }
    if is_config_name(name) {
        return FileKind::Config;
    }
    match candidate.extension.as_deref() {
        Some("md" | "markdown" | "mdx" | "rst" | "adoc") => FileKind::Documentation,
        Some(
            "rs" | "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "vue" | "svelte" | "astro"
            | "html" | "css" | "scss" | "sass" | "less" | "py" | "go" | "java" | "kt" | "kts" | "c"
            | "h" | "cpp" | "hpp" | "cs" | "rb" | "php" | "swift" | "sql" | "graphql" | "gql"
            | "sh" | "bash" | "ps1" | "bat" | "cmd" | "lua",
        ) => FileKind::Code,
        Some("json" | "yaml" | "yml" | "toml" | "ini" | "xml") => FileKind::Config,
        _ => FileKind::UnknownText,
    }
}

fn is_frontend(candidate: &FileCandidate) -> bool {
    matches!(
        candidate.extension.as_deref(),
        Some(
            "ts" | "tsx"
                | "js"
                | "jsx"
                | "mjs"
                | "cjs"
                | "vue"
                | "svelte"
                | "astro"
                | "html"
                | "css"
                | "scss"
                | "sass"
                | "less"
        )
    )
}

fn is_script(candidate: &FileCandidate) -> bool {
    matches!(
        candidate.extension.as_deref(),
        Some("sh" | "bash" | "ps1" | "bat" | "cmd")
    )
}

fn is_config_name(name: &str) -> bool {
    name.starts_with("vite.config.")
        || name.starts_with("webpack.config.")
        || name.starts_with("rollup.config.")
        || name.starts_with("next.config.")
        || name.starts_with("nuxt.config.")
        || name.starts_with("svelte.config.")
        || name.starts_with("astro.config.")
        || name.starts_with("tailwind.config.")
        || name.starts_with("postcss.config.")
        || name.starts_with("eslint.config.")
        || name.starts_with("prettier.config.")
        || matches!(
            name,
            "tsconfig.json" | "jsconfig.json" | "Dockerfile" | "Makefile"
        )
}

fn priority_for(candidate: &FileCandidate, kind: &FileKind) -> i32 {
    let name = candidate.file_name.as_str();
    if matches!(name, "main.rs" | "lib.rs" | "Cargo.toml" | "package.json") {
        100
    } else {
        match kind {
            FileKind::Manifest => 90,
            FileKind::Config => 80,
            FileKind::Code => 70,
            FileKind::UnknownText => 50,
            FileKind::Documentation => 20,
            _ => 10,
        }
    }
}

fn language_for(candidate: &FileCandidate) -> Option<String> {
    let lang = match candidate.extension.as_deref()? {
        "rs" => "rust",
        "toml" => "toml",
        "ts" => "typescript",
        "tsx" => "typescriptreact",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "javascriptreact",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "py" => "python",
        "go" => "go",
        "html" => "html",
        "css" => "css",
        "scss" | "sass" => "scss",
        "sql" => "sql",
        "md" | "mdx" => "markdown",
        "sh" | "bash" => "shell",
        "ps1" => "powershell",
        other => other,
    };
    Some(lang.to_string())
}
