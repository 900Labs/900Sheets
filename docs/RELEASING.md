# Release process

This checklist is for project maintainers.

## Prepare

1. Confirm `main` is clean and up to date.
2. Choose the release version using semantic versioning.
3. Update the workspace, npm package, Tauri bundle, changelog, README, and compatibility documentation to the same version.
4. Confirm no generated artifacts, local paths, credentials, personal data, or private test files are tracked.

## Verify

Run:

```bash
./scripts/verify-local.sh
./scripts/verify-public-release.sh
npm audit --prefix apps/desktop --audit-level=high
cargo audit
npm run tauri:build --prefix apps/desktop
```

Check the built bundle version:

```bash
/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' \
  target/release/bundle/macos/900Sheets.app/Contents/Info.plist
```

Inspect the app manually with a new native workbook, an XLSX import, a native save and reopen, an XLSX export, and a PDF export.

The workflow packages the app with `ditto`, extracts the resulting zip in a clean temporary directory, and verifies executable permission before upload. The uploaded artifact is named `900Sheets-<tag>-macos` and contains `900Sheets-<tag>-macos.zip`.

## Review

Require an independent review of:

- Workbook persistence and data-loss risks
- File-format compatibility changes
- Formula and structural-edit behavior
- Documentation claims and known limitations
- Version alignment and release workflow output
- Privacy and secret scan results

Do not tag a release while a release-blocking review finding remains open.

## Tag and publish

Create an annotated tag on the reviewed commit:

```bash
git tag -a v0.3.0 -m "900Sheets v0.3.0"
git push origin v0.3.0
```

The tag starts the macOS workflow. Confirm the workflow succeeds and inspect the uploaded artifact before publishing release notes.

The v0.3.0 app is not signed with a Developer ID or notarized. State that clearly in the release notes. Do not describe Windows or Linux packages as available unless they were built and tested for that release.
