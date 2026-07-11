# Release process

This checklist is for project maintainers. The current release line is v0.4.0.

## Prepare

1. Confirm `main` is clean and up to date.
2. Choose the release version using semantic versioning.
3. Align the Cargo workspace, npm package and lock, Tauri config, Cargo lock, changelog, README, compatibility page, and release notes.
4. Confirm no generated artifacts, local paths, credentials, personal data, or private fixtures are tracked.
5. Confirm the compatibility matrix points to tests that still exist and distinguishes automated evidence from manual claims.

## Verify locally

Run the shared checks on a supported development host:

```bash
./scripts/verify-local.sh
./scripts/verify-public-release.sh
npm audit --prefix apps/desktop --audit-level=high
cargo audit
```

On macOS, build the release bundle with:

```bash
npm run tauri:build --prefix apps/desktop
```

On Windows or Linux, use `npm run build --prefix apps/desktop`, `cargo test -p sheets-desktop --lib`, and `cargo build --release -p sheets-desktop` as source checks. These commands do not create a supported installer or distribution package.

Check the macOS bundle version:

```bash
/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' \
  target/release/bundle/macos/900Sheets.app/Contents/Info.plist
```

Exercise a new workbook, native save and reopen, XLSX open, CSV import and undo, XLSX export, cross-sheet formula, recovery restore, and PDF export. Use invented data.

Ad hoc sign and strictly verify the completed app bundle before archiving:

```bash
codesign --force --deep --sign - \
  target/release/bundle/macos/900Sheets.app
codesign --verify --deep --strict --verbose=2 \
  target/release/bundle/macos/900Sheets.app
```

Create the same archive shape as CI, extract it, then verify both its executable permission and the extracted app signature:

```bash
ditto -c -k --sequesterRsrc --keepParent \
  target/release/bundle/macos/900Sheets.app \
  target/release/bundle/macos/900Sheets-v0.4.0-macos.zip
mkdir -p target/release/archive-check
ditto -x -k target/release/bundle/macos/900Sheets-v0.4.0-macos.zip \
  target/release/archive-check
test -x target/release/archive-check/900Sheets.app/Contents/MacOS/sheets-desktop
codesign --verify --deep --strict --verbose=2 \
  target/release/archive-check/900Sheets.app
```

Record the SHA-256 hash of the zip used for review.

## Platform recovery gate

The `recovery-platform-checks` CI job runs on `ubuntu-latest` and `windows-latest`. It executes:

```bash
cargo test -p sheets-desktop --lib
```

This command must compile and run the complete desktop library suite, including `failed_cleanup_retires_snapshot_from_discovery_and_is_retryable`. Linux installs the GTK, WebKitGTK, and SVG development packages needed to compile Tauri. Windows uses its native recovery replacement implementation through `MoveFileExW`.

Do not narrow this job to a name filter that omits a recovery regression. The job validates backend recovery and transaction behavior; it does not create or verify Windows or Linux packages.

## GitHub artifact flow

An annotated `v0.4.0` tag starts `.github/workflows/release.yml`:

1. GitHub checks out the tagged commit on `macos-latest`.
2. Rust 1.92.0 and Node.js 22 are installed.
3. `npm ci` installs the locked frontend dependencies.
4. Tauri builds `target/release/bundle/macos/900Sheets.app`.
5. `codesign --force --deep --sign -` ad hoc signs the completed app, then strict deep verification checks it.
6. `ditto` creates `900Sheets-v0.4.0-macos.zip` with the app directory and permissions intact.
7. The workflow extracts the zip, requires the executable permission, and strictly verifies the extracted app signature.
8. The artifact named `900Sheets-v0.4.0-macos` is uploaded for inspection.

The workflow does not publish a GitHub Release automatically. Inspect the downloaded artifact and its version before publishing release notes.

## Review

Require an independent review of:

- Workbook persistence and data-loss risks
- Transaction atomicity, undo, and recovery behavior
- Formula and file-format compatibility changes
- Documentation claims and known limitations
- Version alignment and release workflow output
- Platform matrix results
- Privacy, secret, and generated-artifact scan results

Do not tag while a release-blocking finding remains open.

## Tag and publish

After approval, create and push an annotated tag:

```bash
git tag -a v0.4.0 -m "900Sheets v0.4.0"
git push origin v0.4.0
```

Confirm the release workflow succeeds, download and inspect the artifact, compare its SHA-256 hash with the reviewed output where applicable, and then publish the release notes.

The v0.4.0 app is ad hoc signed and is not notarized. Windows and Linux packages are not release assets. State both limits clearly.
