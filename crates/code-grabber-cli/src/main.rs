use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use code_grabber_core::{
    BundleMode, ScanConfig, ScanConfigBuilder, generate_bundle, init_project_profile, load_config,
    write_bundle_file,
};

#[derive(Debug, Parser)]
#[command(
    name = "cg",
    version,
    about = "Build LLM-ready codebase bundles.",
    long_about = "Code Grabber scans a local repository, filters noisy files, estimates tokens, and writes a clean bundle for LLM chat or coding assistants.",
    after_help = "Examples:\n  cg bundle . --mode core --budget 250000\n  cg inspect . --include-tests\n  cg bundle . --output codebase_bundle.txt --copy\n  cg init ."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Generate a codebase bundle and write it to disk.
    Bundle(BundleArgs),
    /// Preview what would be included, excluded, and token-heavy.
    Inspect(BundleArgs),
    /// Create a .codebundle.toml profile in the target repository.
    Init {
        /// Repository root where the profile should be created.
        root: PathBuf,
    },
}

#[derive(Debug, Parser)]
struct BundleArgs {
    /// Repository root to scan.
    #[arg(default_value = ".")]
    root: PathBuf,

    /// Bundle profile to use: complete, core, compact, map, diff, or pinned.
    #[arg(long, default_value = "core")]
    mode: BundleMode,

    /// Token budget used for the summary and overflow warnings.
    #[arg(long)]
    budget: Option<usize>,

    /// Output file path. Defaults to the configured output path.
    #[arg(long)]
    output: Option<PathBuf>,

    /// Include test files that are excluded by default.
    #[arg(long)]
    include_tests: bool,

    /// Include documentation files that are excluded by default.
    #[arg(long)]
    include_docs: bool,

    /// Include lockfiles that are excluded by default.
    #[arg(long)]
    include_lockfiles: bool,

    /// Include hidden files that are excluded by default.
    #[arg(long)]
    include_hidden: bool,

    /// Strip comments from supported files where possible.
    #[arg(long)]
    strip_comments: bool,

    /// Copy the generated bundle to the clipboard.
    #[arg(long)]
    copy: bool,

    /// Show the scan summary without writing a bundle file.
    #[arg(long)]
    dry_run: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Bundle(args) => run_bundle(args),
        Command::Inspect(args) => run_inspect(args),
        Command::Init { root } => {
            init_project_profile(&root).context("failed to initialize .codebundle.toml")?;
            print_section("Profile initialized");
            println!("  path  {}", root.join(".codebundle.toml").display());
            println!("  next  cg inspect {}", root.display());
            Ok(())
        }
    }
}

fn run_bundle(args: BundleArgs) -> Result<()> {
    let config = build_config(&args)?;
    print_section("Scanning repository");
    println!("  root  {}", config.root.display());
    println!("  mode  {}", mode_label(config.mode));
    let result = generate_bundle(&config).context("failed to generate bundle")?;
    print_summary(&config, &result.report, SummaryIntent::Bundle);

    if !args.dry_run && config.output.write_to_file {
        let output_path = config.output_path();
        write_bundle_file(&output_path, &result.output.contents)
            .context("failed to write bundle")?;
        print_section("Output");
        println!("  wrote  {}", output_path.display());
        println!("  size   {}", format_bytes(result.output.contents.len()));
    } else if args.dry_run {
        print_section("Output");
        println!("  dry run  no bundle file written");
    }

    if args.copy {
        let mut clipboard = arboard::Clipboard::new().context("failed to open clipboard")?;
        clipboard
            .set_text(result.output.contents)
            .context("failed to copy bundle to clipboard")?;
        println!("  copied   bundle copied to clipboard");
    }

    Ok(())
}

fn run_inspect(args: BundleArgs) -> Result<()> {
    let mut args = args;
    args.dry_run = true;
    let config = build_config(&args)?;
    print_section("Inspecting repository");
    println!("  root  {}", config.root.display());
    println!("  mode  {}", mode_label(config.mode));
    let result = generate_bundle(&config).context("failed to inspect repository")?;
    print_summary(&config, &result.report, SummaryIntent::Inspect);

    let mut included = result.report.included_files.clone();
    included.sort_by(|a, b| b.tokens.cmp(&a.tokens));
    print_file_table("Largest included files", &included, FileTableKind::Included);

    let mut excluded = result.report.excluded_files.clone();
    excluded.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    print_file_table("Largest excluded files", &excluded, FileTableKind::Excluded);
    Ok(())
}

fn build_config(args: &BundleArgs) -> Result<ScanConfig> {
    let root = args
        .root
        .canonicalize()
        .context("invalid repository root")?;
    let mut config = load_config(&root).unwrap_or_else(|_| {
        ScanConfigBuilder::new(&root)
            .build()
            .expect("default scan config should build")
    });

    config.mode = args.mode;
    if args.budget.is_some() {
        config.token_budget = args.budget;
    }
    if let Some(output) = &args.output {
        config.output.output_dir = output
            .parent()
            .unwrap_or_else(|| ".".as_ref())
            .to_path_buf();
        config.output.output_filename = output
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "codebase_bundle.txt".to_string());
    }
    if args.include_tests {
        config.include_rules.tests = true;
    }
    if args.include_docs {
        config.include_rules.docs = true;
    }
    if args.include_lockfiles {
        config.include_rules.lockfiles = true;
    }
    if args.include_hidden {
        config.include_rules.hidden_files = true;
    }
    if args.strip_comments {
        config.strip_rules.comments = true;
    }
    if args.copy {
        config.output.copy_to_clipboard = true;
    }
    Ok(config.finalize()?)
}

#[derive(Debug, Clone, Copy)]
enum SummaryIntent {
    Bundle,
    Inspect,
}

#[derive(Debug, Clone, Copy)]
enum FileTableKind {
    Included,
    Excluded,
}

fn print_summary(
    config: &ScanConfig,
    report: &code_grabber_core::report::BundleReport,
    intent: SummaryIntent,
) {
    print_section(match intent {
        SummaryIntent::Bundle => "Bundle summary",
        SummaryIntent::Inspect => "Scan summary",
    });
    println!(
        "  {:<13} {}",
        "included",
        plural(report.included_files.len(), "file")
    );
    println!(
        "  {:<13} {}",
        "excluded",
        plural(report.excluded_files.len(), "file")
    );
    println!(
        "  {:<13} {}",
        "source bytes",
        format_bytes(report.total_bytes)
    );
    println!("  {:<13} {}", "tokens", format_number(report.total_tokens));
    if let Some(budget) = config.token_budget {
        if report.total_tokens > budget {
            println!(
                "  {:<13} {} over budget",
                "budget",
                format_number(report.total_tokens - budget)
            );
        } else {
            println!(
                "  {:<13} {} remaining",
                "budget",
                format_number(budget - report.total_tokens)
            );
        }
    }

    if !report.warnings.is_empty() {
        print_section("Warnings");
        for warning in &report.warnings {
            println!("  - {warning}");
        }
    }
}

fn print_file_table(
    title: &str,
    files: &[code_grabber_core::report::FileReport],
    kind: FileTableKind,
) {
    print_section(title);
    if files.is_empty() {
        println!("  none");
        return;
    }

    match kind {
        FileTableKind::Included => {
            println!("  {:<46} {:>12} {:>10}", "path", "tokens", "size");
            println!(
                "  {:<46} {:>12} {:>10}",
                "-".repeat(46),
                "-".repeat(12),
                "-".repeat(10)
            );
            for file in files.iter().take(10) {
                println!(
                    "  {:<46} {:>12} {:>10}",
                    truncate_middle(&file.path, 46),
                    format_number(file.tokens),
                    format_bytes(file.bytes)
                );
            }
        }
        FileTableKind::Excluded => {
            println!("  {:<38} {:>10}  reason", "path", "size");
            println!(
                "  {:<38} {:>10}  {}",
                "-".repeat(38),
                "-".repeat(10),
                "-".repeat(22)
            );
            for file in files.iter().take(10) {
                println!(
                    "  {:<38} {:>10}  {}",
                    truncate_middle(&file.path, 38),
                    format_bytes(file.bytes),
                    truncate_end(file.reason.as_deref().unwrap_or("excluded"), 36)
                );
            }
        }
    }
}

fn print_section(title: &str) {
    println!();
    println!("{title}");
    println!("{}", "-".repeat(title.len()));
}

fn mode_label(mode: BundleMode) -> &'static str {
    match mode {
        BundleMode::Complete => "complete",
        BundleMode::Core => "core",
        BundleMode::Compact => "compact",
        BundleMode::Map => "map",
        BundleMode::Diff => "diff",
        BundleMode::Pinned => "pinned",
    }
}

fn plural(count: usize, unit: &str) -> String {
    let suffix = if count == 1 { "" } else { "s" };
    format!("{} {unit}{suffix}", format_number(count))
}

fn format_number(value: usize) -> String {
    let raw = value.to_string();
    let mut out = String::with_capacity(raw.len() + raw.len() / 3);
    for (index, ch) in raw.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

fn format_bytes(bytes: usize) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} B")
    } else if value >= 10.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn truncate_middle(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }
    if width <= 3 {
        return ".".repeat(width);
    }

    let keep = width - 3;
    let head = keep / 2;
    let tail = keep - head;
    let start: String = value.chars().take(head).collect();
    let end: String = value
        .chars()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{start}...{end}")
}

fn truncate_end(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        value.to_string()
    } else if width <= 3 {
        ".".repeat(width)
    } else {
        format!("{}...", value.chars().take(width - 3).collect::<String>())
    }
}
