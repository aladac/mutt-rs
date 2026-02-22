//! Mail sync with notifications

use anyhow::{Context, Result};
use std::process::Command;

/// Sync mail and notify of new messages
pub fn sync(quiet: bool, quick: bool) -> Result<()> {
    use std::io::{self, Write};

    // Get list of channels from mbsync
    let channels = get_mbsync_channels(quick)?;
    let total_steps = channels.len() + 1; // +1 for indexing
    let mut sync_stats: Vec<(String, SyncStats)> = Vec::new();

    // Sync each channel with progress bar
    for (i, channel) in channels.iter().enumerate() {
        if !quiet {
            print_progress(i, total_steps, &format!("Syncing {}", channel));
        }

        let mbsync = Command::new("mbsync")
            .args(["-V", channel]) // -V for verbose output with counts
            .output()
            .context("Failed to run mbsync")?;

        if !mbsync.status.success() {
            let stderr = String::from_utf8_lossy(&mbsync.stderr);
            if !quiet {
                eprintln!("\r\x1b[K\x1b[31m✗\x1b[0m mbsync {} failed", channel);
            }
            anyhow::bail!("mbsync {} failed: {}", channel, stderr);
        }

        // Parse mbsync output for stats
        let output = String::from_utf8_lossy(&mbsync.stdout);
        let stderr = String::from_utf8_lossy(&mbsync.stderr);
        let stats = parse_mbsync_output(&output, &stderr);
        if stats.has_activity() {
            sync_stats.push((channel.clone(), stats));
        }
    }

    // Index with notmuch
    if !quiet {
        print_progress(channels.len(), total_steps, "Indexing");
    }

    let notmuch = Command::new("notmuch")
        .args(["new"])
        .output()
        .context("Failed to run notmuch new")?;

    if !notmuch.status.success() {
        let stderr = String::from_utf8_lossy(&notmuch.stderr);
        if !quiet {
            eprintln!("\r\x1b[K\x1b[31m✗\x1b[0m notmuch failed");
        }
        anyhow::bail!("notmuch new failed: {}", stderr);
    }

    // Clear progress line
    if !quiet {
        eprint!("\r\x1b[K");
        io::stderr().flush()?;
    }

    // Parse notmuch output for new messages
    let output = String::from_utf8_lossy(&notmuch.stdout);
    let new_messages = parse_new_messages(&output);

    // Show sync results
    if !quiet {
        if sync_stats.is_empty() && new_messages.is_empty() {
            eprintln!("\x1b[32m✓\x1b[0m No changes");
        } else {
            // Show per-account stats
            for (channel, stats) in &sync_stats {
                let parts: Vec<String> = [
                    (stats.new_msgs, "new"),
                    (stats.deleted, "del"),
                    (stats.flags, "flags"),
                ]
                .iter()
                .filter(|(n, _)| *n > 0)
                .map(|(n, label)| format!("{} {}", n, label))
                .collect();

                if !parts.is_empty() {
                    eprintln!(
                        "\x1b[32m✓\x1b[0m \x1b[33m{}\x1b[0m: {}",
                        channel,
                        parts.join(", ")
                    );
                }
            }

            // Summary
            let total_new: usize = sync_stats.iter().map(|(_, s)| s.new_msgs).sum();
            if total_new > 0 {
                eprintln!(
                    "\x1b[32m✉\x1b[0m {} new message{}",
                    total_new,
                    if total_new == 1 { "" } else { "s" }
                );
            }
        }
    }

    // Send notification if there are new messages
    if !new_messages.is_empty() {
        notify(&new_messages)?;
    }

    Ok(())
}

#[derive(Default)]
struct SyncStats {
    new_msgs: usize,
    deleted: usize,
    flags: usize,
}

impl SyncStats {
    fn has_activity(&self) -> bool {
        self.new_msgs > 0 || self.deleted > 0 || self.flags > 0
    }
}

/// Parse mbsync verbose output for sync statistics
fn parse_mbsync_output(stdout: &str, stderr: &str) -> SyncStats {
    let mut stats = SyncStats::default();
    let combined = format!("{}\n{}", stdout, stderr);

    for line in combined.lines() {
        // Summary line format:
        // "Channels: 1    Boxes: 1    Far: +0 *0 #0 -0    Near: +0 *0 #0 -0"
        // +N = new, *N = flags, -N = deleted

        if line.contains("Far:") && line.contains("Near:") {
            // Parse Far and Near stats
            for part in line.split_whitespace() {
                if let Some(n) = part.strip_prefix('+') {
                    if let Ok(num) = n.parse::<usize>() {
                        stats.new_msgs += num;
                    }
                } else if let Some(n) = part.strip_prefix('*') {
                    if let Ok(num) = n.parse::<usize>() {
                        stats.flags += num;
                    }
                } else if let Some(n) = part.strip_prefix('-')
                    && let Ok(num) = n.parse::<usize>()
                {
                    stats.deleted += num;
                }
            }
        }

        // Also capture message counts: "near side: 20 messages, 11 recent"
        if line.contains("messages,") && line.contains("recent") {
            // Could parse total message count here if needed
        }
    }

    stats
}

/// Print progress bar
fn print_progress(current: usize, total: usize, label: &str) {
    use std::io::{self, Write};

    let bar_width = 20;
    let filled = (current * bar_width) / total;
    let empty = bar_width - filled;

    let bar: String = format!(
        "\x1b[36m{}\x1b[0m\x1b[2m{}\x1b[0m",
        "█".repeat(filled),
        "░".repeat(empty)
    );

    eprint!("\r\x1b[K{} {}", bar, label);
    let _ = io::stderr().flush();
}

/// Get list of mbsync channels from config
fn get_mbsync_channels(quick: bool) -> Result<Vec<String>> {
    let home = std::env::var("HOME").unwrap_or_default();
    let config_path = format!("{}/.mbsyncrc", home);
    let content = std::fs::read_to_string(&config_path).context("Failed to read ~/.mbsyncrc")?;

    let mut channels = Vec::new();
    for line in content.lines() {
        if line.starts_with("Channel ")
            && let Some(name) = line.strip_prefix("Channel ")
        {
            let name = name.trim().to_string();
            if quick {
                // Quick mode: only -inbox channels
                if name.ends_with("-inbox") {
                    channels.push(name);
                }
            } else {
                // Full mode: skip -inbox channels (they're subsets)
                if !name.ends_with("-inbox") {
                    channels.push(name);
                }
            }
        }
    }

    if channels.is_empty() {
        // Fallback to -a
        channels.push("-a".to_string());
    }

    Ok(channels)
}

/// Parse notmuch new output for new message info
fn parse_new_messages(output: &str) -> Vec<NewMessage> {
    let mut messages = Vec::new();

    for line in output.lines() {
        // notmuch new output: "Added 1 new message to the database."
        // or individual: "Note: Ignoring non-mail file: ..."
        // We want lines like: "Added X new message(s)"
        if line.starts_with("Added") && line.contains("new message") {
            // This is a summary line, not individual messages
            continue;
        }

        // Look for actual new message additions
        // Format varies, but we can also query notmuch for recent messages
    }

    // Better approach: query notmuch for messages added in last minute
    if let Ok(recent) = get_recent_messages() {
        messages = recent;
    }

    messages
}

/// Get messages added in the last sync (within last 2 minutes)
fn get_recent_messages() -> Result<Vec<NewMessage>> {
    let output = Command::new("notmuch")
        .args([
            "search",
            "--format=text",
            "--output=summary",
            "date:2min..",
            "tag:inbox",
        ])
        .output()
        .context("Failed to query recent messages")?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut messages = Vec::new();

    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        // Parse: "thread:xxx date [count] sender; subject (tags)"
        if let Some(msg) = parse_notmuch_line(line) {
            messages.push(msg);
        }
    }

    Ok(messages)
}

/// Parse a notmuch search output line
fn parse_notmuch_line(line: &str) -> Option<NewMessage> {
    // Format: "thread:000... 2026-02-16 [1/1] Sender Name; Subject (tags)"
    let parts: Vec<&str> = line.splitn(4, ' ').collect();
    if parts.len() < 4 {
        return None;
    }

    // Find sender and subject (after date and count)
    let rest = parts[3..].join(" ");

    // Split on semicolon to get sender and subject
    let (sender, subject) = if let Some(pos) = rest.find(';') {
        let sender_part = &rest[..pos];
        // Remove [x/y] count prefix
        let sender = sender_part
            .split(']')
            .next_back()
            .unwrap_or(sender_part)
            .trim();
        let subject = rest[pos + 1..].trim();
        // Remove trailing (tags)
        let subject = if let Some(tag_pos) = subject.rfind('(') {
            subject[..tag_pos].trim()
        } else {
            subject
        };
        (sender.to_string(), subject.to_string())
    } else {
        return None;
    };

    Some(NewMessage { sender, subject })
}

#[derive(Debug)]
struct NewMessage {
    sender: String,
    subject: String,
}

/// Send notification (platform-specific)
fn notify(messages: &[NewMessage]) -> Result<()> {
    let (title, body) = if messages.len() == 1 {
        let msg = &messages[0];
        (format!("New mail from {}", msg.sender), msg.subject.clone())
    } else {
        (
            format!("{} new messages", messages.len()),
            messages
                .iter()
                .take(5)
                .map(|m| {
                    format!(
                        "• {}: {}",
                        truncate(&m.sender, 20),
                        truncate(&m.subject, 30)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
    };

    #[cfg(target_os = "macos")]
    {
        Command::new("terminal-notifier")
            .args([
                "-title",
                "Mail",
                "-subtitle",
                &title,
                "-message",
                &body,
                "-sound",
                "default",
                "-group",
                "mu-mail",
                "-activate",
                "com.apple.Terminal",
            ])
            .output()
            .context("Failed to send notification")?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("notify-send")
            .args(["--app-name=Mail", &title, &body])
            .output()
            .context("Failed to send notification")?;
    }

    Ok(())
}

/// Truncate string to max length
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_notmuch_line() {
        let line = "thread:000000000000000a  2026-02-16 [1/1] Google; Security alert (inbox)";
        let msg = parse_notmuch_line(line).unwrap();
        assert_eq!(msg.sender, "Google");
        assert_eq!(msg.subject, "Security alert");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello w…");
    }
}
