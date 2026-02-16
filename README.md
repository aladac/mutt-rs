# mutt-rs

Swiss army knife for mutt/neomutt. Provides a single `mu` binary for common tasks.

## Installation

```bash
cargo install --path .
```

## Usage

```bash
# Render HTML email to terminal
mu render -i email.html

# Pipe from stdin
cat email.html | mu render

# With options
mu render --width 80 --strip-urls=false
```

## Commands

| Command | Description |
|---------|-------------|
| `render` | Render HTML email to clean terminal output |

### Future Commands (planned)

- `sync` - mbsync wrapper with progress
- `search` - notmuch search interface
- `config` - generate mutt/mbsync/notmuch configs
- `setup` - interactive setup wizard

## Integration with neomutt

In `~/.mailcap`:

```mailcap
text/html; mu render -i %s; copiousoutput
```

## License

MIT
