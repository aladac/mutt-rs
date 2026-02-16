//! Fuzzy mail search with fzf + notmuch

use crate::render;
use anyhow::{Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

const CMD_FILE: &str = "/tmp/neomutt-fzf-cmd";

/// Run fuzzy mail search and output neomutt command
pub fn search(query: Option<&str>) -> Result<()> {
    let query = query.unwrap_or("*");

    // Get mail list from notmuch
    let mails = get_mail_list(query)?;
    if mails.is_empty() {
        eprintln!("No messages found");
        write_empty_cmd()?;
        return Ok(());
    }

    // Run fzf with preview - use mu preview directly, {1} = first field (thread ID)
    let selected = run_fzf(&mails)?;

    if let Some(line) = selected {
        // Extract thread ID (first word, like "thread:0000000000000123")
        let thread_id = line.split_whitespace().next().unwrap_or("");
        if !thread_id.is_empty() {
            write_neomutt_cmd(thread_id)?;
        } else {
            write_empty_cmd()?;
        }
    } else {
        write_empty_cmd()?;
    }

    Ok(())
}

/// Get formatted mail list from notmuch
fn get_mail_list(query: &str) -> Result<Vec<String>> {
    let output = Command::new("notmuch")
        .args(["search", "--format=text", "--output=summary", query])
        .output()
        .context("Failed to run notmuch search")?;

    if !output.status.success() {
        anyhow::bail!(
            "notmuch search failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.lines().map(String::from).collect())
}

/// Run fzf with mail preview
fn run_fzf(items: &[String]) -> Result<Option<String>> {
    let mut child = Command::new("fzf")
        .args([
            "--ansi",
            "--preview",
            "mu preview {1}",  // {1} = first field = thread ID
            "--preview-window=right:50%:wrap",
            "--header",
            "Enter: open | Esc: cancel",
            "--prompt",
            "mail> ",
            "--no-mouse",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit()) // Show fzf UI on terminal
        .spawn()
        .context("Failed to spawn fzf")?;

    // Write items to fzf stdin
    if let Some(mut stdin) = child.stdin.take() {
        for item in items {
            writeln!(stdin, "{}", item)?;
        }
    }

    let output = child.wait_with_output()?;

    if output.status.success() {
        let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if selected.is_empty() {
            Ok(None)
        } else {
            Ok(Some(selected))
        }
    } else {
        // User cancelled (Esc)
        Ok(None)
    }
}

/// Write neomutt command to navigate to thread
fn write_neomutt_cmd(thread_id: &str) -> Result<()> {
    let cmd = format!("push '<vfolder-from-query>{}<enter>'\n", thread_id);
    std::fs::write(CMD_FILE, cmd).context("Failed to write neomutt command file")?;
    Ok(())
}

/// Write empty command (cancelled)
fn write_empty_cmd() -> Result<()> {
    std::fs::write(CMD_FILE, "").context("Failed to write empty command file")?;
    Ok(())
}

/// Preview a mail thread (for fzf preview)
pub fn preview(thread_id: &str) -> Result<()> {
    // Get the email in text format (notmuch handles MIME decoding)
    let output = Command::new("notmuch")
        .args(["show", "--format=text", "--entire-thread=false", thread_id])
        .output()
        .context("Failed to run notmuch show")?;

    if !output.status.success() {
        anyhow::bail!("notmuch show failed");
    }

    let text = String::from_utf8_lossy(&output.stdout);

    // Parse notmuch text output
    let mut in_headers = false;
    let mut in_body = false;
    let mut body_type = String::new();
    let mut body_content = String::new();
    let mut headers_printed = false;
    let mut body_printed = false;
    let mut has_html_only = false;

    for line in text.lines() {
        // Notmuch text format markers
        if line.starts_with("\u{c}header{") {
            in_headers = true;
            if !headers_printed {
                println!("\x1b[1;36m=== Headers ===\x1b[0m");
            }
            continue;
        }
        if line.starts_with("\u{c}header}") {
            in_headers = false;
            headers_printed = true;
            continue;
        }
        if line.starts_with("\u{c}body{") {
            in_body = true;
            continue;
        }
        if line.starts_with("\u{c}body}") {
            in_body = false;
            continue;
        }
        if line.starts_with("\u{c}part{") {
            // Extract content type
            if let Some(ct) = line.split("content-type:").nth(1) {
                body_type = ct.split_whitespace().next().unwrap_or("").to_string();
            }
            continue;
        }
        if line.starts_with("\u{c}part}") {
            // End of part - render if we have useful content and haven't printed body yet
            if !body_printed && !body_content.is_empty() {
                if body_content.trim() == "Non-text part: text/html" {
                    // HTML-only email - need to fetch raw and render
                    has_html_only = true;
                } else {
                    print_body(&body_content, &body_type);
                    body_printed = true;
                }
            }
            body_content.clear();
            body_type.clear();
            continue;
        }
        if line.starts_with("\u{c}") {
            // Other control lines - skip
            continue;
        }

        if in_headers {
            // Print key headers with colors
            if line.starts_with("Subject:") {
                println!("\x1b[1;33m{}\x1b[0m", line);
            } else if line.starts_with("From:") || line.starts_with("To:") || line.starts_with("Date:") {
                println!("{}", line);
            }
        } else if in_body && !body_printed {
            body_content.push_str(line);
            body_content.push('\n');
        }
    }

    // Print any remaining body content
    if !body_printed && !body_content.is_empty() && body_content.trim() != "Non-text part: text/html" {
        print_body(&body_content, &body_type);
        body_printed = true;
    }

    // HTML-only email - fetch raw and render
    if !body_printed && has_html_only {
        preview_html_only(thread_id)?;
    }

    Ok(())
}

/// Preview HTML-only emails by fetching raw and rendering with w3m
fn preview_html_only(thread_id: &str) -> Result<()> {
    // Use notmuch to get the raw email, then extract and render HTML
    // We'll use Python's email module which handles all MIME decoding properly
    let output = Command::new("python3")
        .args([
            "-c",
            &format!(
                r#"
import subprocess
import email
from email import policy

# Get raw message from notmuch
result = subprocess.run(['notmuch', 'show', '--format=raw', '{}'], capture_output=True)
msg = email.message_from_bytes(result.stdout, policy=policy.default)

# Find HTML part
for part in msg.walk():
    if part.get_content_type() == 'text/html':
        html = part.get_content()
        print(html)
        break
"#,
                thread_id
            ),
        ])
        .output()
        .context("Failed to extract HTML")?;

    if output.status.success() && !output.stdout.is_empty() {
        let html = String::from_utf8_lossy(&output.stdout);
        print_body(&html, "text/html");
    }

    Ok(())
}

/// Print body content, rendering HTML if needed
fn print_body(content: &str, content_type: &str) {
    println!("\n\x1b[1;36m=== Preview ===\x1b[0m");

    let rendered = if content_type.contains("text/html") {
        // Render HTML to clean text
        match render::render(content, true) {
            Ok(text) => text,
            Err(_) => content.to_string(),
        }
    } else {
        content.to_string()
    };

    // Print first 30 lines
    for (i, line) in rendered.lines().enumerate() {
        if i >= 30 {
            println!("\x1b[2m... (truncated)\x1b[0m");
            break;
        }
        println!("{}", line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_neomutt_cmd() {
        let thread_id = "thread:0000000000000001";
        write_neomutt_cmd(thread_id).unwrap();
        let content = std::fs::read_to_string(CMD_FILE).unwrap();
        assert!(content.contains("vfolder-from-query"));
        assert!(content.contains(thread_id));
    }
}
