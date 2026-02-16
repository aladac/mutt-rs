# mutt-rs

Swiss army knife for mutt/neomutt. Provides a single `mu` binary for common tasks.

## Installation

```bash
cargo install --path .
```

## Commands

| Command | Description |
|---------|-------------|
| `render` | Render HTML email to clean terminal output (w3m + ANSI colors) |
| `sync` | Sync mail with mbsync + notmuch, show progress, send notifications |
| `fzf` | Fuzzy search mail with fzf + notmuch |
| `preview` | Preview mail thread (for fzf preview window) |

## Usage

```bash
# Render HTML email to terminal
mu render -i email.html
cat email.html | mu render

# Sync mail (with progress bar and macOS notifications)
mu sync           # Full sync
mu sync --quick   # Inbox only

# Fuzzy search mail
mu fzf
mu fzf -q "from:github"
```

## Integration with neomutt

### Mailcap (HTML rendering)

In `~/.mailcap`:

```mailcap
text/html; mu render -i %s; copiousoutput
```

### Keybindings

In `~/.config/neomutt/neomuttrc`:

```muttrc
# Sync mail with progress
macro index S "<shell-escape>mu sync<enter>" "Sync all mail"
macro index s "<shell-escape>mu sync --quick<enter>" "Quick sync"

# Fuzzy search with fzf
macro index <C-f> "<shell-escape>mu fzf<enter><enter-command>source /tmp/neomutt-fzf-cmd<enter>" "fzf search"
```

## Related

- [mutt](https://github.com/aladac/mutt) - NeoMutt config files and install scripts

## License

MIT
