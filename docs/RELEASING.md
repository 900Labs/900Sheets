# Release process

This checklist is for project maintainers. The current release line is v0.5.0.

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

On Windows, create the supported NSIS installer with:

```powershell
npm run tauri:build --prefix apps/desktop -- --bundles nsis
```

On Linux, use `npm run build --prefix apps/desktop`, `cargo test -p sheets-desktop --lib`, and `cargo build --release -p sheets-desktop` as source checks. These commands do not create a supported Linux installer or distribution package.

Check the macOS bundle version:

```bash
/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' \
  target/release/bundle/macos/900Sheets.app/Contents/Info.plist
```

Exercise a new workbook, native save and reopen, XLSX open, CSV import and undo, XLSX export, cross-sheet formula, recovery restore, and PDF export. Use invented data.

Without Apple release credentials, ad hoc sign and strictly verify the completed app bundle before archiving:

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
  target/release/bundle/macos/900Sheets-v0.5.0-macos.zip
mkdir -p target/release/archive-check
ditto -x -k target/release/bundle/macos/900Sheets-v0.5.0-macos.zip \
  target/release/archive-check
test -x target/release/archive-check/900Sheets.app/Contents/MacOS/sheets-desktop
codesign --verify --deep --strict --verbose=2 \
  target/release/archive-check/900Sheets.app
```

This local fallback is not Developer ID signed or notarized. Record that fact and the SHA-256 hash of the zip used for review.

## Release signing credentials

The release workflow supports two explicit macOS modes. It never describes an ad hoc build as notarized.

For a Developer ID signed and notarized build, configure all six GitHub Actions secrets:

- `APPLE_CERTIFICATE`: the Developer ID Application certificate and private key exported as a password-protected PKCS #12 file, then base64 encoded
- `APPLE_CERTIFICATE_PASSWORD`: the PKCS #12 export password
- `APPLE_SIGNING_IDENTITY`: the full Developer ID Application identity shown by `security find-identity`
- `APPLE_ID`: the Apple ID used for notarization
- `APPLE_PASSWORD`: an app-specific password for that Apple ID
- `APPLE_TEAM_ID`: the Apple Developer team identifier

When all six are present, the workflow imports the certificate into a temporary keychain, applies a hardened-runtime timestamped signature, submits the app to Apple's notary service, staples the accepted ticket, and verifies the final archive with `codesign`, `stapler`, and Gatekeeper. A failure in any step stops publication.

When all six are absent, artifact-only manual runs preserve the v0.4 behavior: the workflow applies an ad hoc signature, strictly verifies the archive, and records `signing=ad-hoc` and `notarization=not-notarized` in its provenance file. A partial Apple configuration fails closed. An ad hoc artifact can be inspected from the workflow run, but the publication job rejects it. Public releases require Developer ID signing and a successfully validated, stapled notarization ticket.

Windows Authenticode signing is optional until a project certificate is available. Configure both `WINDOWS_CERTIFICATE` (a base64-encoded PKCS #12 file) and `WINDOWS_CERTIFICATE_PASSWORD` to sign and timestamp the installer. If both are absent, the workflow verifies that the installer is unsigned and records `signing=unsigned`. A partial Windows configuration fails closed.

## Platform recovery gate

The `recovery-platform-checks` CI job runs on `ubuntu-latest` and `windows-latest`. It executes:

```bash
cargo test -p sheets-desktop --lib
```

This command must compile and run the complete desktop library suite, including `failed_cleanup_retires_snapshot_from_discovery_and_is_retryable`. Linux installs the GTK, WebKitGTK, and SVG development packages needed to compile Tauri. Windows uses its native recovery replacement implementation through `MoveFileExW`.

Do not narrow this job to a name filter that omits a recovery regression. The job validates backend recovery and transaction behavior; it does not create or verify Windows or Linux packages.

## GitHub artifact and publication flow

An annotated version tag starts `.github/workflows/release.yml`. The tag must exactly equal `v` followed by the version in `tauri.conf.json`, reference the workflow commit, and point to a reviewed merge commit reachable from `origin/main`. Directly tagging an unmerged feature commit or a single-parent commit fails closed.

1. GitHub checks out the tagged commit on `macos-latest`.
2. Rust 1.92.0 and Node.js 22 are installed.
3. `npm ci` installs the locked frontend dependencies.
4. The release workflow reruns the complete local gate and dependency audits against the tagged source. A failure stops both package builds.
5. Tauri builds the macOS `.app` and Windows NSIS installer in separate jobs.
6. The macOS job either completes Developer ID signing and notarization or uses the explicit ad hoc fallback described above.
7. The macOS archive is extracted. Its version, executable permission, signature, and notarization ticket when applicable are checked.
8. The Windows installer is Authenticode signed when both Windows secrets are configured. Otherwise its unsigned state is verified and recorded.
9. The Windows job performs a silent install, checks the installed executable and product version, then performs a silent uninstall.
10. Each platform job uploads the package, a SHA-256 checksum, and a signing provenance file.
11. The publication job starts only after the source gate and both platform jobs succeed. It downloads exactly six files, verifies both checksums and the provenance, requires Developer ID signing and notarization, and creates the GitHub Release from the matching existing tag.

Publication is immutable. If a release already exists for the tag, the job fails instead of replacing its assets. A manual workflow run builds artifacts without publishing by default. Selecting `publish_release` requires the selected commit to be the annotated tag's reviewed merge commit on `main`. Signing credentials are exposed only to the narrow mode-selection, import, signing, or notarization steps that need them. Dependency installation, application builds, and repository scripts do not receive signing secrets.

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
git tag -a v0.5.0 -m "900Sheets v0.5.0"
git push origin v0.5.0
```

Confirm that both package jobs and the publication job succeed. Download both published packages and compare their SHA-256 hashes with the accompanying checksum files.

Release notes must state the signing values from the two provenance files. Do not claim Developer ID signing, notarization, or Authenticode signing from configured intent alone. Linux packages are not release assets unless a later release workflow adds and verifies them.
