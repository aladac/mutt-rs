# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

`mutt-rs` is a Rust CLI tool (`mu`) for mutt/neomutt email integration. It provides commands for rendering HTML emails, fuzzy searching with fzf/notmuch, and syncing mail with mbsync.

## Development Commands

```bash
# Build and install
cargo build
cargo install --path .

# Run checks (required before completing work)
cargo fmt
cargo clippy                      # Must be zero warnings
cargo test -- --test-threads=1    # Single-threaded for env var safety
cargo tarpaulin --skip-clean      # Must maintain 100% coverage

# Or use /check command
```

## Architecture

```
src/
├── main.rs    # CLI entry point (clap), stdin/stdout handling
├── render.rs  # HTML→text rendering (w3m primary, html-to-markdown-rs fallback)
├── fzf.rs     # Fuzzy search via fzf + notmuch, preview command
└── sync.rs    # Mail sync via mbsync + notmuch with progress + notifications
```

The binary is named `mu` (defined in Cargo.toml `[[bin]]`).

## Related Projects

- `/Users/chi/Projects/mutt` - NeoMutt config files (neomuttrc, account configs, install scripts)

## External Dependencies

- **w3m**: HTML rendering (optional, has fallback)
- **notmuch**: Mail indexing and search
- **mbsync**: IMAP sync
- **fzf**: Fuzzy finder for mail selection
- **terminal-notifier**: macOS notifications (sync command)

## Code Rules

See `.claude/RULES.md` for full rules. Key points:

- **No `.unwrap()` in lib code** - use `?` or `anyhow::bail!`
- **No `panic!()` for recoverable errors**
- **No `dbg!()` or `todo!()` in commits**
- **Function body limit**: 50 lines
- **File limit**: ~300 lines
- **Zero clippy warnings**
- **100% test coverage** (cargo-tarpaulin)

Import order: std → external crates → crate → super/self

## Testing Pattern

Use `run_with()` pattern for testable CLI commands:

```rust
pub fn run(args: Args) -> Result<()> {
    run_with(args, &Config::load()?)
}

fn run_with(args: Args, cfg: &Config) -> Result<()> {
    // testable logic here
}
```

Tests are inline (`#[cfg(test)] mod tests`) in each module.
