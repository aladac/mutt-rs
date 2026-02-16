//! HTML to terminal rendering

use anyhow::Result;
use html_to_markdown_rs::{ConversionOptions, convert};
use regex::Regex;

/// Render HTML content to clean markdown (for piping to glow/bat)
pub fn render(html: &str, strip_urls: bool) -> Result<String> {
    // Detect if input is HTML
    let is_html = html.to_lowercase().contains("<html")
        || html.to_lowercase().contains("<body")
        || html.to_lowercase().contains("<!doctype");

    let output = if is_html {
        render_html(html, strip_urls)?
    } else {
        render_plain(html, strip_urls)
    };

    Ok(output)
}

fn render_html(html: &str, strip_urls: bool) -> Result<String> {
    // Use w3m for clean HTML→text conversion (handles complex email layouts well)
    let text = match convert_with_w3m(html) {
        Ok(text) => text,
        Err(_) => {
            // Fallback to html-to-markdown-rs if w3m not available
            let md = convert(html, Some(ConversionOptions::default()))?;
            clean_markdown(&md, strip_urls)
        }
    };

    // Clean up w3m output
    let cleaned = clean_text(&text, strip_urls);
    Ok(cleaned)
}

fn convert_with_w3m(html: &str) -> Result<String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new("w3m")
        .args(["-dump", "-T", "text/html", "-cols", "120"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(html.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        anyhow::bail!("w3m failed")
    }
}

// ANSI color codes
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const CYAN: &str = "\x1b[36m";
const YELLOW: &str = "\x1b[33m";
const DIM: &str = "\x1b[2m";

fn clean_text(text: &str, strip_urls: bool) -> String {
    let mut output = text.to_string();

    if strip_urls {
        // Remove long URLs
        let long_url_re = Regex::new(r"https?://[^\s]{40,}").unwrap();
        output = long_url_re.replace_all(&output, "").to_string();
    }

    // Remove zero-width spaces and other invisible chars
    output = output.replace('\u{034F}', "");
    output = output.replace('\u{200B}', "");
    output = output.replace('\u{200C}', "");
    output = output.replace('\u{200D}', "");
    output = output.replace('\u{FEFF}', "");

    // Clean excessive newlines
    let newline_re = Regex::new(r"\n{3,}").unwrap();
    output = newline_re.replace_all(&output, "\n\n").to_string();

    // Add colors and formatting
    output = add_colors(&output);

    output.trim().to_string()
}

fn add_colors(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut result = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // Detect table-like structures (lines with multiple columns separated by spaces)
        if is_table_row(line) {
            // Collect consecutive table rows
            let mut table_lines = vec![line];
            let mut j = i + 1;
            while j < lines.len() && (is_table_row(lines[j]) || lines[j].trim().is_empty()) {
                if !lines[j].trim().is_empty() {
                    table_lines.push(lines[j]);
                }
                j += 1;
            }

            if table_lines.len() >= 2 {
                // Format as a table with borders
                let formatted = format_table(&table_lines);
                result.push(formatted);
                i = j;
                continue;
            }
        }

        // Color headers (centered text, ALL CAPS, or short bold-looking lines)
        if is_header(line) {
            result.push(format!("{}{}{}{}", BOLD, CYAN, line, RESET));
        }
        // Color section titles (lines ending with :)
        else if line.trim().ends_with(':') && line.trim().len() < 50 && !line.contains("  ") {
            result.push(format!("{}{}{}{}", BOLD, YELLOW, line, RESET));
        } else {
            result.push(line.to_string());
        }

        i += 1;
    }

    result.join("\n")
}

fn is_table_row(line: &str) -> bool {
    // A table row has key:value pairs with whitespace alignment
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.len() < 15 {
        return false;
    }

    // Must have a label (word followed by :) and a value after whitespace
    // Pattern: "Label:    Value" with significant gap
    let colon_pos = trimmed.find(':');
    if let Some(pos) = colon_pos {
        // Check there's content after the colon with whitespace gap
        let after_colon = &trimmed[pos + 1..];
        let has_gap = after_colon.starts_with("  ") || after_colon.starts_with(" \t");
        let has_value = !after_colon.trim().is_empty();

        // Label shouldn't be too short or contain URL-like content
        let label = &trimmed[..pos];
        let valid_label =
            label.len() >= 3 && label.len() <= 30 && !label.contains("//") && !label.contains("@");

        return has_gap && has_value && valid_label;
    }

    false
}

fn is_header(line: &str) -> bool {
    let trimmed = line.trim();

    // Empty or too long = not a header
    if trimmed.is_empty() || trimmed.len() > 60 {
        return false;
    }

    // Centered text (significant leading whitespace)
    let leading_spaces = line.len() - line.trim_start().len();
    let is_centered = leading_spaces > 10 && trimmed.len() < 50;

    // ALL CAPS (at least 3 words)
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    let is_all_caps = words.len() >= 2
        && words.iter().all(|w| {
            let letters: String = w.chars().filter(|c| c.is_alphabetic()).collect();
            letters.len() >= 2 && letters == letters.to_uppercase()
        });

    is_centered || is_all_caps
}

fn format_table(lines: &[&str]) -> String {
    // Find the max visual width (Unicode-aware)
    let max_len = lines.iter().map(|l| visual_width(l)).max().unwrap_or(0);
    let box_width = max_len + 2; // Add padding

    let mut result = Vec::new();

    // Add top border
    result.push(format!("{}┌{}┐{}", DIM, "─".repeat(box_width), RESET));

    for line in lines {
        // Format the row content with colors
        let formatted = format_table_row(line);
        // Pad to align right border (use visual width for proper alignment)
        let vis_len = visual_width(line);
        let padding = box_width - vis_len - 1;
        result.push(format!(
            "{}│{} {}{}{}│{}",
            DIM,
            RESET,
            formatted,
            " ".repeat(padding.max(0)),
            DIM,
            RESET
        ));
    }

    // Add bottom border
    result.push(format!("{}└{}┘{}", DIM, "─".repeat(box_width), RESET));

    result.join("\n")
}

/// Calculate visual width of a string (Unicode-aware)
fn visual_width(s: &str) -> usize {
    s.chars().count()
}

fn format_table_row(line: &str) -> String {
    // Color labels (words ending with :) in yellow
    let mut result = String::new();
    let mut current_word = String::new();

    for c in line.chars() {
        current_word.push(c);

        // Check if this ends a label (word followed by :)
        if c == ':' && !current_word.trim().is_empty() {
            let word = current_word.trim();
            if word.len() > 1
                && word
                    .chars()
                    .next()
                    .map(|ch| ch.is_alphabetic())
                    .unwrap_or(false)
            {
                // It's a label - color it
                result.push_str(&format!("{}{}{}", YELLOW, word, RESET));
                current_word.clear();
                continue;
            }
        }

        // Flush on whitespace
        if c == ' ' || c == '\t' {
            result.push_str(&current_word);
            current_word.clear();
        }
    }

    result.push_str(&current_word);
    result
}

fn render_plain(text: &str, strip_urls: bool) -> String {
    if strip_urls {
        strip_long_urls(text)
    } else {
        text.to_string()
    }
}

fn clean_markdown(md: &str, strip_urls: bool) -> String {
    let mut output = md.to_string();

    // Remove YAML frontmatter (including partial ones)
    let frontmatter_re = Regex::new(r"(?m)^---\n[\s\S]*?\n---\n?").unwrap();
    output = frontmatter_re.replace(&output, "").to_string();
    // Also remove standalone --- lines
    let dashes_re = Regex::new(r"(?m)^---$\n?").unwrap();
    output = dashes_re.replace_all(&output, "").to_string();

    if strip_urls {
        // [text](url) → text
        let link_re = Regex::new(r"\[([^\]]+)\]\(https?://[^)]+\)").unwrap();
        output = link_re.replace_all(&output, "$1").to_string();

        // <url> → remove
        let bare_url_re = Regex::new(r"<https?://[^>]+>").unwrap();
        output = bare_url_re.replace_all(&output, "").to_string();

        // Long bare URLs → remove
        let long_url_re = Regex::new(r"https?://[^\s\)\]]{40,}").unwrap();
        output = long_url_re.replace_all(&output, "").to_string();

        // Convert mailto links to just the email: [text](mailto:email) → text
        let mailto_re = Regex::new(r"\[([^\]]+)\]\(mailto:[^)]+\)").unwrap();
        output = mailto_re.replace_all(&output, "$1").to_string();
    }

    // Clean list items with single-cell tables: "- | text |" → "- **text**"
    let list_single_cell_re = Regex::new(r"^(-\s*)\|\s*([^|]+?)\s*\|$").unwrap();
    output = output
        .lines()
        .map(|line| {
            if let Some(caps) = list_single_cell_re.captures(line) {
                let prefix = caps.get(1).map_or("", |m| m.as_str());
                let text = caps.get(2).map_or("", |m| m.as_str()).trim();
                if text.is_empty() || text.chars().all(|c| c == '-' || c == ' ') {
                    String::new()
                } else {
                    format!("{}**{}**", prefix, text)
                }
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Clean standalone single-cell table rows: "| text |" → "**text**"
    let single_cell_re = Regex::new(r"^\|\s*([^|]+?)\s*\|$").unwrap();
    output = output
        .lines()
        .map(|line| {
            if let Some(caps) = single_cell_re.captures(line) {
                let text = caps.get(1).map_or("", |m| m.as_str()).trim();
                if text.is_empty() || text.chars().all(|c| c == '-' || c == ' ') {
                    String::new()
                } else {
                    format!("**{}**", text)
                }
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Remove empty table cells "| |" or "| | |" etc
    let empty_cells_re = Regex::new(r"\|\s*\|").unwrap();
    output = empty_cells_re.replace_all(&output, "|").to_string();

    // Clean lines that are just "| |" or "- | |"
    let empty_table_line_re = Regex::new(r"^-?\s*\|\s*\|?\s*$").unwrap();
    output = output
        .lines()
        .filter(|line| !empty_table_line_re.is_match(line))
        .collect::<Vec<_>>()
        .join("\n");

    // Remove zero-width spaces and other invisible chars
    output = output.replace('\u{034F}', ""); // combining grapheme joiner
    output = output.replace('\u{200B}', ""); // zero-width space
    output = output.replace('\u{200C}', ""); // zero-width non-joiner
    output = output.replace('\u{200D}', ""); // zero-width joiner
    output = output.replace('\u{FEFF}', ""); // BOM

    // Remove redundant table separators in consecutive tables
    let table_sep_re = Regex::new(r"^\|\s*[-:]+\s*\|").unwrap();
    let mut in_table = false;
    let mut had_separator = false;
    output = output
        .lines()
        .filter(|line| {
            let is_table_row = line.starts_with('|') && line.ends_with('|');
            let is_separator =
                table_sep_re.is_match(line) && line.chars().filter(|c| *c == '-').count() > 2;

            if is_table_row {
                if is_separator {
                    if in_table && had_separator {
                        return false;
                    }
                    had_separator = true;
                }
                in_table = true;
            } else {
                in_table = false;
                had_separator = false;
            }
            true
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Clean excessive newlines
    let newline_re = Regex::new(r"\n{3,}").unwrap();
    output = newline_re.replace_all(&output, "\n\n").to_string();

    output.trim().to_string()
}

fn strip_long_urls(text: &str) -> String {
    let url_re = Regex::new(r"https?://[^\s]{40,}").unwrap();
    url_re.replace_all(text, "").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_plain() {
        let input = "Hello world";
        let output = render(input, true).unwrap();
        assert_eq!(output.trim(), "Hello world");
    }

    #[test]
    fn test_render_html() {
        let input = "<html><body><p>Hello</p></body></html>";
        let output = render(input, true).unwrap();
        assert!(output.contains("Hello"));
    }

    #[test]
    fn test_strip_urls() {
        let input = "Check https://very-long-url.example.com/path/to/something here";
        let output = strip_long_urls(input);
        assert!(!output.contains("https://"));
    }
}
