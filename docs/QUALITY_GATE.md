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
   - `node scripts/check-resource-budgets.mjs` verifies asset budgets and offline WebView2 installer configuration.
7. `npm run test:unit --prefix apps/desktop`
8. `npm run test:e2e --prefix apps/desktop`

The current verification record is in the [product audit](audits/2026-09-07-product-readiness.md). Test counts change as regressions are added. The gate must remain free of Rust compiler/clippy warnings and Svelte or TypeScript diagnostics. Dependency advisory warnings are tracked separately from compiler warnings; the audit records any inherited advisory debt.

The XLSX compatibility test opens and re-saves a generated workbook with LibreOffice. CI installs LibreOffice and fails if `soffice` is unavailable. Local runs report a skip when LibreOffice is not installed.

The `recovery-platform-checks` CI matrix runs all desktop library tests on Ubuntu and Windows. This keeps the Unix and Windows recovery implementations compiled and executes the cleanup-retirement regression on both platforms.

## Public-release preflight

Before tagging, also run:

```bash
./scripts/verify-public-release.sh
npm audit --prefix apps/desktop --audit-level=high
cargo audit
```

On macOS, also build and verify the release bundle with `npm run tauri:build --prefix apps/desktop -- --bundles app`, then follow the signing and archive checks in [RELEASING.md](RELEASING.md). On Windows, build the NSIS package with `npm run tauri:build --prefix apps/desktop -- --bundles nsis` and perform the documented install/uninstall smoke. On Linux, `cargo build --release -p sheets-desktop` is a valid non-bundle compile check. It does not produce a supported package.

Confirm that:

- Versions match across Cargo, npm, Tauri, the changelog, and release notes.
- The privacy gate finds no local paths or obvious committed secrets.
- The native workbook can be saved and reopened.
- Representative XLSX import and export fixtures behave as documented.
- The built app reports the intended release version.
- Known distribution limits, including signing and platform coverage, are stated in the release notes.
- The release workflow validation passes with `./scripts/verify-release-workflow.sh`.
- The tagged workflow verifies the macOS archive and Windows installer before the publication job starts.
- The published checksums match both release packages.
- The macOS and Windows provenance files describe the signing state actually verified by the workflow.
- A partial signing-secret configuration, tag/version mismatch, lightweight tag, non-main release commit, package smoke failure, macOS notarization failure, ad hoc macOS artifact, or existing GitHub Release stops publication.

Any failed check or unresolved release-blocking review finding stops the release.
