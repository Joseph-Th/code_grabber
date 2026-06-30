# Code Grabber

Code Grabber (`cg`) builds a structured, token-aware plaintext bundle from a local repository for use with LLM chat and coding assistants.

The current implementation is a Rust CLI MVP backed by a reusable core library. It uses denylist-first discovery, respects `.gitignore`, excludes common dependency/build/test/generated/binary/secret noise by default, includes unknown text files, estimates OpenAI-style tokens with `tiktoken-rs`, and writes a delimited plaintext bundle.

## Desktop UI

Code Grabber includes a native desktop interface for scanning repositories, reviewing included and excluded files, previewing bundle output, initializing `.codebundle.toml`, writing bundles, and copying bundle contents to the clipboard.

```powershell
cargo run -p code-grabber-gui
```

## Commands

```powershell
cargo run -p code-grabber-cli -- bundle .
cargo run -p code-grabber-cli -- inspect .
cargo run -p code-grabber-cli -- init .
cargo run -p code-grabber-cli -- --help
```

Useful bundle flags:

```powershell
cargo run -p code-grabber-cli -- bundle . --mode core --budget 250000 --output "$HOME\Downloads\codebase_bundle.txt"
cargo run -p code-grabber-cli -- bundle . --include-tests --include-docs --include-lockfiles
cargo run -p code-grabber-cli -- bundle . --dry-run
cargo run -p code-grabber-cli -- bundle . --copy
```

The CLI is designed around two fast feedback loops:

- `inspect` previews included/excluded file counts, estimated tokens, budget status, and the largest included/excluded files without writing output.
- `bundle --dry-run` shows the final bundle summary with the same formatting used by a real bundle run.

## Output Format

The default output starts with metadata, a repository map, and a manifest, then writes each file with stable delimiters:

```text
# CODEBASE BUNDLE v1
project: my-project
mode: Core
tokenizer: o200k_base
estimated_tokens: 184230

# FILE MANIFEST
src/main.rs | rust | 7421 bytes | 1853 tokens | included

<<<FILE path="src/main.rs" lang="rust" bytes=7421 tokens=1853>>>
...
<<<END_FILE>>>
```

## Configuration

Run `cg init .` to create `.codebundle.toml`. The CLI loads this profile when present, then applies command-line overrides.

## Current Scope

Implemented:

- Core library plus CLI workspace layout.
- Native `egui/eframe` desktop UI.
- `.gitignore`-aware walking through `ignore`.
- Default exclusions for dependency folders, build outputs, tests, lockfiles, generated paths, binaries, and secret-like files.
- Broad frontend/Rust/config/text inclusion instead of extension-only Rust filtering.
- Plain delimited bundle rendering with manifest and repository map.
- Token estimation with `o200k_base` by default.
- Polished CLI help, readable scan summaries, largest-file tables, dry-run feedback, output size reporting, and optional clipboard copy.
- Project profile initialization.

Planned next:

- Parser-backed Rust inline test stripping.
- Chunked output and budget-aware file reduction.
- Diff-focused and pinned-file modes.
