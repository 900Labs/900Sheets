# 900Sheets Desktop App

Tauri v2 desktop shell for 900Sheets. The frontend is Svelte 5 and the backend commands live in `src-tauri/src/lib.rs`.

## Run Locally

From the repository root:

```bash
npm ci --prefix apps/desktop
npm run tauri:dev --prefix apps/desktop
```

Frontend-only checks:

```bash
npm run check --prefix apps/desktop
npm run build --prefix apps/desktop
```

Spreadsheet interaction smokes:

```bash
npm run test:e2e --prefix apps/desktop
```

If Playwright reports that Chromium is missing on a fresh machine, install the browser once:

```bash
npx --prefix apps/desktop playwright install chromium
```

Backend checks are run from the repository root:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## File Workflows

The desktop toolbar exposes native-dialog flows backed by Rust commands:

- Open native workbook: `import_native_file`
- Save native workbook: `export_native_file`
- Open XLSX as a replacement workbook: `import_xlsx_file`
- Import CSV or TSV into the active sheet: `import_csv_file`
- Open JSON as a replacement workbook: `import_json_file`
- Export XLSX: `export_xlsx_file`
- Export active sheet as CSV: `export_csv_file`
- Export workbook as JSON: `export_json_file`

The backend validates absolute dialog paths and applies importer resource limits before replacing workbook state. XLSX and JSON replace the workbook and clear history. CSV import is one undoable active-sheet transaction.

Dirty workbooks are written to a separate recovery store after a configurable debounce and again during a close request. Startup recovery preserves unselected snapshots. The post-startup manager keeps crash recoveries separate from rotating native backups. Recovery writes, backup writes, and cleanup use private storage and platform-specific atomic replacement.

## Bundle

On macOS, `npm run tauri:build --prefix apps/desktop -- --bundles app` creates the `.app` bundle. The release workflow supports Developer ID signing and notarization when every documented Apple secret is configured. An artifact-only manual run can use an explicitly marked ad hoc fallback, but the publication job rejects a build that is not Developer ID signed, accepted by Apple, stapled, and reverified.

On Windows, `npm run tauri:build --prefix apps/desktop -- --bundles nsis` creates the NSIS installer. The release workflow performs a silent install, version check, and uninstall before upload. Authenticode signing is used only when both Windows signing secrets are configured; otherwise provenance identifies the installer as unsigned. Linux remains a source and backend validation target without a v0.5.0 distribution package.
