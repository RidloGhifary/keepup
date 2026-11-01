use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap_or_else(|_| "target".into());
    let is_release = env::var("PROFILE").map(|p| p == "release").unwrap_or(false)
        || env::var("FORCE_MINIFY").is_ok();

    println!("cargo:rerun-if-changed=static");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=FORCE_MINIFY");

    if is_release {
        let static_src = Path::new("static");
        let static_min = PathBuf::from(&out_dir).join("static_min");
        fs::create_dir_all(&static_min).expect("Failed to create static_min directory");

        process_directory(static_src, &static_min);

        // Tell rust-embed to use the minified directory in release builds
        println!("cargo:rustc-env=STATIC_DIR={}", static_min.display());
    } else {
        println!("cargo:rustc-env=STATIC_DIR=static");
    }
}

fn process_directory(src: &Path, dst: &Path) {
    let entries = fs::read_dir(src).expect("Failed to read static directory");
    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = path.file_name().unwrap();
        let target_path = dst.join(file_name);

        if path.is_dir() {
            fs::create_dir_all(&target_path).expect("Failed to create subdirectory");
            process_directory(&path, &target_path);
        } else if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            match ext {
                "html" => {
                    let content = fs::read_to_string(&path).expect("Failed to read HTML file");
                    let minified = minify_html(&content);
                    fs::write(&target_path, minified).expect("Failed to write minified HTML");
                }
                "css" => {
                    let content = fs::read_to_string(&path).expect("Failed to read CSS file");
                    let minified = minify_css(&content);
                    fs::write(&target_path, minified).expect("Failed to write minified CSS");
                }
                "js" => {
                    let content = fs::read_to_string(&path).expect("Failed to read JS file");
                    let minified = minify_js(&content);
                    fs::write(&target_path, minified).expect("Failed to write minified JS");
                }
                _ => {
                    fs::copy(&path, &target_path).expect("Failed to copy asset");
                }
            }
        } else {
            fs::copy(&path, &target_path).expect("Failed to copy file without extension");
        }
    }
}

// ── HTML Minifier ────────────────────────────────────────────────────────────

fn minify_html(input: &str) -> String {
    // 1. Strip HTML comments <!-- ... -->
    let mut without_comments = String::with_capacity(input.len());
    let mut remaining = input;
    while let Some(start) = remaining.find("<!--") {
        without_comments.push_str(&remaining[..start]);
        if let Some(end) = remaining[start..].find("-->") {
            remaining = &remaining[start + end + 3..];
        } else {
            remaining = "";
            break;
        }
    }
    without_comments.push_str(remaining);

    // 2. Process inline <style> and <script> tags
    let mut output = String::with_capacity(without_comments.len());
    let mut slice = without_comments.as_str();

    while !slice.is_empty() {
        let next_tag = [
            slice.find("<style").map(|i| (i, "style")),
            slice.find("<script").map(|i| (i, "script")),
            slice.find("<pre").map(|i| (i, "pre")),
        ]
        .into_iter()
        .flatten()
        .min_by_key(|&(i, _)| i);

        if let Some((start_idx, tag)) = next_tag {
            let before = &slice[..start_idx];
            output.push_str(&collapse_html_whitespace(before));

            let tag_content = &slice[start_idx..];
            if let Some(open_tag_end) = tag_content.find('>') {
                let open_tag = &tag_content[..=open_tag_end];
                output.push_str(open_tag);

                let close_tag = format!("</{}>", tag);
                let inner_start = open_tag_end + 1;
                let rest = &tag_content[inner_start..];

                if let Some(close_idx) = rest.to_lowercase().find(&close_tag) {
                    let inner = &rest[..close_idx];
                    let minified_inner = match tag {
                        "style" => minify_css(inner),
                        "script" => {
                            if open_tag.contains("src=") {
                                inner.to_string()
                            } else {
                                minify_js(inner)
                            }
                        }
                        _ => inner.to_string(),
                    };
                    output.push_str(&minified_inner);
                    output.push_str(&close_tag);
                    slice = &rest[close_idx + close_tag.len()..];
                } else {
                    output.push_str(rest);
                    break;
                }
            } else {
                output.push_str(tag_content);
                break;
            }
        } else {
            output.push_str(&collapse_html_whitespace(slice));
            break;
        }
    }

    output
}

fn collapse_html_whitespace(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut in_whitespace = false;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            if !in_whitespace {
                let prev_is_tag = result.ends_with('>');
                let next_is_tag = chars.peek().map(|&nc| nc == '<').unwrap_or(false);
                if !(prev_is_tag || next_is_tag) {
                    result.push(' ');
                }
                in_whitespace = true;
            }
        } else {
            if c == '<' && result.ends_with(' ') {
                result.pop();
            }
            result.push(c);
            in_whitespace = false;
        }
    }
    result
}

// ── CSS Minifier ─────────────────────────────────────────────────────────────

fn minify_css(input: &str) -> String {
    // 1. Strip CSS comments /* ... */
    let mut no_comments = String::with_capacity(input.len());
    let mut remaining = input;
    while let Some(start) = remaining.find("/*") {
        no_comments.push_str(&remaining[..start]);
        if let Some(end) = remaining[start..].find("*/") {
            remaining = &remaining[start + end + 2..];
        } else {
            remaining = "";
            break;
        }
    }
    no_comments.push_str(remaining);

    // 2. Collapse whitespace & remove around symbols
    let mut output = String::with_capacity(no_comments.len());
    let mut in_str: Option<char> = None;
    let mut prev_char: Option<char> = None;
    let mut chars = no_comments.chars().peekable();

    while let Some(c) = chars.next() {
        if let Some(quote) = in_str {
            output.push(c);
            if c == quote && prev_char != Some('\\') {
                in_str = None;
            }
            prev_char = Some(c);
            continue;
        }

        if c == '"' || c == '\'' {
            in_str = Some(c);
            output.push(c);
            prev_char = Some(c);
            continue;
        }

        if c.is_whitespace() {
            let next_is_punct = chars.peek().map(|&nc| "{}:;,>~+".contains(nc)).unwrap_or(true);
            let prev_is_punct = prev_char.map(|pc| "{}:;,>~+".contains(pc)).unwrap_or(true);
            if !prev_is_punct && !next_is_punct {
                output.push(' ');
                prev_char = Some(' ');
            }
            continue;
        }

        output.push(c);
        prev_char = Some(c);
    }

    output
}

// ── JS Minifier ──────────────────────────────────────────────────────────────

fn minify_js(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string: Option<char> = None;
    let mut in_template = false;
    let mut template_depth = 0;
    let mut at_line_start = true;
    let mut prev_non_ws: Option<char> = None;
    let mut last_word = String::new();

    while let Some(c) = chars.next() {
        // Inside string '...' or "..."
        if let Some(quote) = in_string {
            output.push(c);
            at_line_start = false;
            if c == '\\' {
                if let Some(next_c) = chars.next() {
                    output.push(next_c);
                }
            } else if c == quote {
                in_string = None;
                prev_non_ws = Some(quote);
                last_word.clear();
            }
            continue;
        }

        // Inside template literal `...`
        if in_template {
            output.push(c);
            at_line_start = c == '\n';
            if c == '\\' {
                if let Some(next_c) = chars.next() {
                    output.push(next_c);
                }
            } else if c == '$' && chars.peek() == Some(&'{') {
                output.push(chars.next().unwrap());
                template_depth += 1;
            } else if c == '}' && template_depth > 0 {
                template_depth -= 1;
            } else if c == '`' && template_depth == 0 {
                in_template = false;
                prev_non_ws = Some('`');
                last_word.clear();
            }
            continue;
        }

        // Comments & Regex
        if c == '/' {
            if chars.peek() == Some(&'/') {
                chars.next();
                while let Some(&nc) = chars.peek() {
                    if nc == '\n' {
                        break;
                    }
                    chars.next();
                }
                continue;
            } else if chars.peek() == Some(&'*') {
                chars.next();
                while let Some(nc) = chars.next() {
                    if nc == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        break;
                    }
                }
                continue;
            } else if is_regex_start(prev_non_ws, &last_word) {
                // Regular Expression Literal!
                output.push(c);
                let mut in_bracket = false;
                while let Some(rc) = chars.next() {
                    output.push(rc);
                    if rc == '\\' {
                        if let Some(nrc) = chars.next() {
                            output.push(nrc);
                        }
                    } else if rc == '[' {
                        in_bracket = true;
                    } else if rc == ']' {
                        in_bracket = false;
                    } else if rc == '/' && !in_bracket {
                        while let Some(&fc) = chars.peek() {
                            if fc.is_ascii_alphabetic() {
                                output.push(chars.next().unwrap());
                            } else {
                                break;
                            }
                        }
                        break;
                    }
                }
                prev_non_ws = Some('/');
                last_word.clear();
                at_line_start = false;
                continue;
            }
        }

        // Check for string starts
        if c == '\'' || c == '"' {
            in_string = Some(c);
            output.push(c);
            at_line_start = false;
            prev_non_ws = Some(c);
            last_word.clear();
            continue;
        }

        if c == '`' {
            in_template = true;
            template_depth = 0;
            output.push(c);
            at_line_start = false;
            prev_non_ws = Some(c);
            last_word.clear();
            continue;
        }

        if c == '\n' {
            if !at_line_start && !output.ends_with('\n') {
                output.push('\n');
                at_line_start = true;
            }
            continue;
        }

        if c.is_whitespace() {
            if !at_line_start && !output.ends_with(' ') && !output.ends_with('\n') {
                output.push(' ');
            }
            continue;
        }

        output.push(c);
        at_line_start = false;
        prev_non_ws = Some(c);
        if c.is_ascii_alphanumeric() || c == '_' || c == '$' {
            last_word.push(c);
        } else {
            last_word.clear();
        }
    }

    output.trim().to_string()
}

fn is_regex_start(last_char: Option<char>, last_word: &str) -> bool {
    if let Some(c) = last_char {
        if "=([{:;,!?&|^~+-%<>".contains(c) {
            return true;
        }
    }
    matches!(
        last_word,
        "return" | "case" | "typeof" | "void" | "delete" | "yield" | "await" | "throw"
    )
}
