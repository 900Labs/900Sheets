# Quality gate

Run the complete gate before merging to `main`:

```bash
./scripts/verify-local.sh
```

It runs:

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
3. `cargo test --workspace`
4. `npm ci --prefix apps/desktop`
5. `npm run check --prefix apps/desktop`
6. `npm run build --prefix apps/desktop`
7. `npm run test:unit --prefix apps/desktop`
8. `npm run test:e2e --prefix apps/desktop`

The v0.4.0 release-prep baseline is 462 Rust tests, 9 frontend unit tests, and 16 Playwright tests. A higher count is expected when new tests are added. The gate must remain free of Rust warnings and Svelte or TypeScript diagnostics.

The XLSX compatibility test opens and re-saves a generated workbook with LibreOffice. CI installs LibreOffice and fails if `soffice` is unavailable. Local runs report a skip when LibreOffice is not installed.

The `recovery-platform-checks` CI matrix runs all desktop library tests on Ubuntu and Windows. This keeps the Unix and Windows recovery implementations compiled and executes the cleanup-retirement regression on both platforms.

## Public-release preflight

Before tagging, also run:

```bash
./scripts/verify-public-release.sh
npm audit --prefix apps/desktop --audit-level=high
cargo audit
```

On macOS, also build and verify the release bundle with `npm run tauri:build --prefix apps/desktop`, then follow the signing and archive checks in [RELEASING.md](RELEASING.md). On Windows or Linux, `cargo build --release -p sheets-desktop` is a valid non-bundle compile check. It does not produce a supported package.

Confirm that:

- Versions match across Cargo, npm, Tauri, the changelog, and release notes.
- The privacy gate finds no local paths or obvious committed secrets.
- The native workbook can be saved and reopened.
- Representative XLSX import and export fixtures behave as documented.
- The built app reports the intended release version.
- Known distribution limits, including signing and platform coverage, are stated in the release notes.

Any failed check or unresolved release-blocking review finding stops the release.
