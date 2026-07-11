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
- Import XLSX: `import_xlsx_file`
- Import CSV or TSV into the active sheet: `import_csv_file`
- Import JSON workbook: `import_json_file`
- Export XLSX: `export_xlsx_file`
- Export active sheet as CSV: `export_csv_file`
- Export workbook as JSON: `export_json_file`

The backend validates absolute dialog paths and applies importer resource limits before replacing workbook state.

## Bundle

The default bundle target is the macOS `.app` bundle. DMG packaging is intentionally not the default until signing, notarization, and packaging prerequisites are configured.
