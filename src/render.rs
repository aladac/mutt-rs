//! HTML to terminal rendering

use anyhow::Result;
use html_to_markdown_rs::{convert, ConversionOptions};
use regex::Regex;

/// Render HTML content to clean terminal output
pub fn render(html: &str, strip_urls: bool, _width: usize) -> Result<String> {
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
    let md = convert(html, Some(ConversionOptions::default()))?;
    let cleaned = clean_markdown(&md, strip_urls);
    Ok(cleaned)
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

    // Remove YAML frontmatter
    let frontmatter_re = Regex::new(r"^---\n[\s\S]*?\n---\n").unwrap();
    output = frontmatter_re.replace(&output, "").to_string();

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
    }

    // Clean excessive newlines
    let newline_re = Regex::new(r"\n{3,}").unwrap();
    output = newline_re.replace_all(&output, "\n\n").to_string();

    // Clean empty table rows
    let empty_table_re = Regex::new(r"^\|[\s\-\|]+\|$").unwrap();
    output = empty_table_re.replace_all(&output, "").to_string();

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
        let output = render(input, true, 100).unwrap();
        assert_eq!(output, "Hello world");
    }

    #[test]
    fn test_render_html() {
        let input = "<html><body><p>Hello</p></body></html>";
        let output = render(input, true, 100).unwrap();
        assert!(output.contains("Hello"));
    }

    #[test]
    fn test_strip_urls() {
        let input = "Check https://very-long-url.example.com/path/to/something here";
        let output = strip_long_urls(input);
        assert!(!output.contains("https://"));
    }
}
