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

Dirty workbooks are written to a separate recovery store after a 750 millisecond debounce and again during a close request. Startup recovery preserves unselected snapshots. Recovery writes and cleanup are serialized and use platform-specific atomic replacement.

## Bundle

On macOS, `npm run tauri:build --prefix apps/desktop` creates the `.app` bundle. The release workflow then ad hoc signs and strictly verifies the complete bundle. DMG packaging, Developer ID signing, and notarization are not configured.

On Windows or Linux, use `cargo build --release -p sheets-desktop` for a non-bundle source compile after building the frontend. This release does not publish or claim Windows or Linux packages.
