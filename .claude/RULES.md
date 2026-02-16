# Project Rules

Read this BEFORE writing code. Not after.

## Naming

**NO redundant prefixes:**
- `config::Config` → `Config` (re-export at crate root)
- `render::Renderer` → `Renderer` (re-export at crate root)
- `module::ModuleThing` → `module::Thing`

## Imports

Order: std → external crates → crate → super/self

## Code Limits

| Metric | Limit |
|--------|-------|
| Line width | 120 |
| Function body | 50 lines |
| Arguments | 5 max |
| File length | ~300 lines |

## Forbidden

- `.unwrap()` in lib code
- `panic!()` for recoverable errors
- `use module::*`
- `dbg!()` or `todo!()` in commits
- Clippy warnings
- Unformatted code

## Testing

**Write tests alongside code, not after.**

Every new function needs:
- Unit test for happy path
- Unit test for error paths
- `run_with()` pattern for CLI commands

```rust
// CLI command pattern
pub fn run(args: Args) -> Result<()> {
    run_with(args, &Config::load()?)
}

fn run_with(args: Args, cfg: &Config) -> Result<()> {
    // testable logic
}
```

See `doc/stack/testing.md` for patterns.

## Before Finishing

```bash
cargo fmt
cargo clippy  # must be zero warnings
cargo test -- --test-threads=1
cargo tarpaulin --skip-clean  # must maintain 100%
```

Or run `/check` command.
