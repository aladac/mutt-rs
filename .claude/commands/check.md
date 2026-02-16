# /check - Validate before finishing

Run this before declaring any work complete.

## Steps

1. **Format code:**
   ```bash
   cargo fmt
   ```

2. **Run clippy (must be zero warnings):**
   ```bash
   cargo clippy 2>&1
   ```
   If any warnings, fix them. Do not proceed with warnings.

3. **Run tests (single-threaded for env var safety):**
   ```bash
   cargo test -- --test-threads=1 2>&1
   ```

4. **Check coverage (must maintain 100%):**
   ```bash
   cargo tarpaulin --skip-clean 2>&1 | tail -5
   ```
   If coverage dropped, add missing tests before proceeding.

5. **Check for forbidden patterns in changed files:**
   - `.unwrap()` in src/ (lib code)
   - `todo!()` or `dbg!()`
   - Missing tests for new functions

6. **Report results** - only say "done" if all checks pass.
