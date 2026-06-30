# Quality Gate

The quality gate must pass before merging to `main`. Run:

```bash
./scripts/verify-local.sh
```

## Checks

1. **Rust formatting**: `cargo fmt --all -- --check` — properly formatted
2. **Rust lints**: `cargo clippy --workspace --all-targets -- -D warnings` — zero warnings
3. **Rust tests**: `cargo test --workspace` — all tests must pass
4. **Frontend install**: `npm install --prefix apps/desktop` — dependencies installed
5. **Frontend check**: `npm run check --prefix apps/desktop` — type check passes
6. **Frontend build**: `npm run build --prefix apps/desktop` — build succeeds

## Failure Policy

If any check fails, the PR cannot be merged. Fix all issues before requesting review.
