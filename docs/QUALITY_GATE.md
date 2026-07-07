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
7. **Spreadsheet interaction smoke tests**: `npm run test:e2e --prefix apps/desktop` — core editing, formula, copy/paste, and compact-menu flows pass in Playwright

## Failure Policy

If any check fails, the PR cannot be merged. Fix all issues before requesting review.

## Public Release Preflight

Before tagging a public release, also run:

```bash
./scripts/verify-public-release.sh
npm audit --prefix apps/desktop
cargo audit
npm run tauri:build --prefix apps/desktop
```

The public release preflight verifies that source files do not contain local
machine paths or obvious committed secrets, that npm dependencies have no known
vulnerabilities, that Rust dependencies have no failing RustSec advisories, and
that the desktop app bundle can be produced from a clean checkout.
