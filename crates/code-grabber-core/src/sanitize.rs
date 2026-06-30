use crate::classify::FileClassification;
use crate::config::ScanConfig;
use crate::discover::FileCandidate;

pub fn sanitize_content(
    config: &ScanConfig,
    content: &str,
    classification: &FileClassification,
    candidate: &FileCandidate,
) -> (String, Vec<String>) {
    let mut transformations = Vec::new();
    let mut output = content.replace("\r\n", "\n").replace('\r', "\n");
    if output != content {
        transformations.push("normalized newlines".to_string());
    }

    let trimmed = trim_trailing_whitespace(&output);
    if trimmed != output {
        transformations.push("trimmed trailing whitespace".to_string());
        output = trimmed;
    }

    if config.strip_rules.rust_inline_tests && candidate.extension.as_deref() == Some("rs") {
        let stripped = strip_rust_test_blocks(&output);
        if stripped != output {
            transformations.push("stripped rust inline tests".to_string());
            output = stripped;
        }
    }

    if config.strip_rules.blank_lines {
        let collapsed = collapse_blank_lines(&output);
        if collapsed != output {
            transformations.push("collapsed blank lines".to_string());
            output = collapsed;
        }
    }

    if config.strip_rules.comments
        && classification
            .language
            .as_deref()
            .is_some_and(can_strip_comments)
    {
        let stripped = strip_simple_line_comments(&output);
        if stripped != output {
            transformations.push("stripped simple comments".to_string());
            output = stripped;
        }
    }

    (output, transformations)
}

fn trim_trailing_whitespace(content: &str) -> String {
    content
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

fn collapse_blank_lines(content: &str) -> String {
    let mut output = String::with_capacity(content.len());
    let mut blank_count = 0;
    for line in content.lines() {
        if line.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 2 {
                output.push('\n');
            }
        } else {
            blank_count = 0;
            output.push_str(line);
            output.push('\n');
        }
    }
    output.trim_end_matches('\n').to_string()
}

fn can_strip_comments(language: &str) -> bool {
    matches!(
        language,
        "rust"
            | "typescript"
            | "typescriptreact"
            | "javascript"
            | "javascriptreact"
            | "go"
            | "java"
            | "c"
            | "cpp"
            | "cs"
    )
}

fn strip_simple_line_comments(content: &str) -> String {
    content
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_rust_test_blocks(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut output = Vec::with_capacity(lines.len());
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim_start();
        if is_rust_test_attr(trimmed) {
            let start = i;
            i += 1;
            while i < lines.len() && lines[i].trim_start().starts_with("#[") {
                i += 1;
            }
            if i < lines.len() && (lines[i].contains("fn ") || lines[i].contains("mod ")) {
                i = skip_rust_item(&lines, i);
                continue;
            }
            output.extend_from_slice(&lines[start..i]);
            continue;
        }

        output.push(lines[i]);
        i += 1;
    }

    output.join("\n")
}

fn is_rust_test_attr(trimmed: &str) -> bool {
    trimmed.starts_with("#[test")
        || trimmed.starts_with("#[tokio::test")
        || trimmed.starts_with("#[wasm_bindgen_test")
        || trimmed.starts_with("#[rstest")
        || trimmed.starts_with("#[cfg(test")
}

fn skip_rust_item(lines: &[&str], mut i: usize) -> usize {
    let mut brace_depth = 0_i32;
    let mut saw_open = false;
    while i < lines.len() {
        for ch in lines[i].chars() {
            match ch {
                '{' => {
                    brace_depth += 1;
                    saw_open = true;
                }
                '}' => brace_depth -= 1,
                _ => {}
            }
        }
        i += 1;
        if saw_open && brace_depth <= 0 {
            break;
        }
    }
    i
}
