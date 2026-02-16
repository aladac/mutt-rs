//! mu - Swiss army knife for mutt/neomutt
//!
//! Handles stdin/stdout/files for mutt integration.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::io::{self, Read, Write};
use std::path::PathBuf;

mod fzf;
mod render;
mod sync;

#[derive(Parser)]
#[command(name = "mu", version, about = "Swiss army knife for mutt/neomutt")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Render HTML email to markdown (pipe to glow for colors)
    Render {
        /// Input file (reads stdin if not provided)
        #[arg(short, long)]
        input: Option<PathBuf>,

        /// Output file (writes stdout if not provided)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Strip URLs from output
        #[arg(long, default_value_t = true)]
        strip_urls: bool,
    },

    /// Fuzzy search mail with fzf + notmuch
    Fzf {
        /// Search query (default: all mail)
        #[arg(short, long)]
        query: Option<String>,
    },

    /// Preview a mail thread (for fzf preview window)
    Preview {
        /// Thread ID (e.g., thread:0000000000000123)
        thread_id: String,
    },

    /// Sync mail (mbsync + notmuch) with notifications
    Sync {
        /// Quiet mode (no output, just notify)
        #[arg(short, long)]
        quiet: bool,

        /// Quick mode (inbox only)
        #[arg(long)]
        quick: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Render {
            input,
            output,
            strip_urls,
        } => {
            let content = read_input(input.as_deref())?;
            let rendered = render::render(&content, strip_urls)?;
            write_output(output.as_deref(), &rendered)?;
        }
        Commands::Fzf { query } => {
            fzf::search(query.as_deref())?;
        }
        Commands::Preview { thread_id } => {
            fzf::preview(&thread_id)?;
        }
        Commands::Sync { quiet, quick } => {
            sync::sync(quiet, quick)?;
        }
    }

    Ok(())
}

/// Read from file or stdin
fn read_input(path: Option<&std::path::Path>) -> Result<String> {
    match path {
        Some(p) => Ok(std::fs::read_to_string(p)?),
        None => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        }
    }
}

/// Write to file or stdout
fn write_output(path: Option<&std::path::Path>, content: &str) -> Result<()> {
    match path {
        Some(p) => Ok(std::fs::write(p, content)?),
        None => {
            io::stdout().write_all(content.as_bytes())?;
            Ok(())
        }
    }
}
