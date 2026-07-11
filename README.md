# 900Sheets

900Sheets is a free, local-first desktop spreadsheet editor. It supports formulas, formatting, common exchange formats, and continued editing without an account, subscription, telemetry, or constant internet access.

## Why it exists

900 Labs builds open-source tools for people and communities priced out of modern software. Spreadsheets are basic working infrastructure for schools, small businesses, researchers, public services, and community organizations. 900Sheets is designed to remain useful on an ordinary computer and when an internet connection is unavailable.

## Current release

Version 0.4.0 focuses on workbook correctness and recovery:

- Cross-sheet cell and range formulas, including quoted sheet names and absolute references
- Workbook-wide dependency validation and refresh of formulas that depend on edited cells, with cross-sheet cycle rejection
- Transactional workbook mutations with undo and redo for cells, formats, sheets, structure, CSV import, feature metadata, comments, locks, sort, replace, and pivot output
- Bounded history with atomic rejection when an operation exceeds its safety budget
- Autosaved recovery snapshots after edits and a final recovery write when the app closes with unsaved changes
- Startup recovery prompts that preserve unselected snapshots and quarantine corrupt snapshots
- Native `.900sheets` workbooks compatible with files written by v0.3.0
- Clear **Open XLSX**, **Open JSON**, and **Import CSV** workflows
- Recovery regression tests on both Linux and Windows, plus the macOS release build

The editor also includes 174 formula functions, formatting, find and replace, filters, pivots, chart previews, validation, conditional formatting, named ranges, frozen panes, comments, protection, print settings, and PDF output.

Read [Compatibility and known limitations](docs/COMPATIBILITY.md) before using 900Sheets for important work. The packaged macOS build is ad hoc signed and is not notarized. Windows and Linux packages are not published in v0.4.0.

## Install and run

### macOS release artifact

Tagged releases build and archive `900Sheets.app` in GitHub Actions. Download the `900Sheets-v0.4.0-macos` artifact, extract `900Sheets-v0.4.0-macos.zip`, and move the app to Applications. The release workflow verifies that the executable permission survives the archive round trip.

The artifact is ad hoc signed and not notarized. macOS may require you to right-click the app and choose **Open** the first time. Only use artifacts published by the `900Labs/900Sheets` repository.

### Build from source

Prerequisites:

- Rust 1.92.0, pinned by `rust-toolchain.toml`
- Node.js 20.19 or newer, 22.12 or newer, or 24 or newer
- The [Tauri v2 system prerequisites](https://v2.tauri.app/start/prerequisites/)

```bash
git clone https://github.com/900Labs/900Sheets.git
cd 900Sheets
npm ci --prefix apps/desktop
npm run tauri:dev --prefix apps/desktop
```

### Build a macOS app bundle

On macOS, create the `.app` bundle with:

```bash
npm run tauri:build --prefix apps/desktop
```

That command is the documented v0.4.0 bundle path for macOS. It does not claim or produce the supported Windows and Linux release packages, which are not available in this release.

### Validate Windows or Linux source

After installing the Tauri prerequisites for the host platform, run the source checks and compile the desktop target without packaging:

```bash
npm ci --prefix apps/desktop
npm run check --prefix apps/desktop
npm run build --prefix apps/desktop
cargo test -p sheets-desktop --lib
cargo build --release -p sheets-desktop
```

The final command builds the Rust desktop target for the current host. It is not an installer or distribution package.

## How to use it

1. Select a cell and type a value or a formula such as `=SUM(A1:A10)`.
2. Refer to another sheet with `=Data!A1` or `=SUM('Annual Budget'!$A$1:$A$12)`.
3. Use the sheet tabs and menus to organize sheets, format data, and apply data tools.
4. Choose **File > Save Workbook** to create or update an editable `.900sheets` file.
5. Choose **Open XLSX** or **Open JSON** to replace the current workbook with imported content. Save afterward as `.900sheets` if you want to keep editing.
6. Choose **Import CSV** to add CSV or TSV data to the active sheet as one undoable transaction.
7. Use the export commands for XLSX, CSV, JSON, or PDF exchange files.

### Recovery and undo

After a successful edit, 900Sheets waits 750 milliseconds for activity to settle, flushes pending mutations, and writes a recovery snapshot to the app data directory. Recovery files are separate from the workbook you opened. A normal native save retires the corresponding recovery.

If recovery snapshots are found at startup, the app lists them newest first. Restore the selected snapshot or cancel to discard only that snapshot and continue to the next. Restoring one does not delete the others. Save a restored workbook to keep it as a normal `.900sheets` file.

Undo history is bounded to 100 transactions, 64 MiB in aggregate, 32 MiB per transaction, and 200,000 changed coordinates per transaction. An operation that exceeds a per-transaction limit is rejected without partially changing the live workbook.

The full walkthrough is in the [User guide](docs/USER_GUIDE.md).

## Platform support

| Platform | v0.4.0 status |
| --- | --- |
| macOS | Release workflow builds an ad hoc signed `.app`; notarization is not configured |
| Windows | Desktop recovery and transaction library suite compiles and runs in CI; no installer is published |
| Linux | Desktop recovery and transaction library suite compiles and runs in CI with Tauri dependencies; no package is published |

Source builds may work on Tauri-supported systems once their platform prerequisites are installed. The CI matrix is a backend behavior gate, not a claim that every desktop environment has been manually tested.

## Development

The repository is a Rust workspace with a Svelte 5 and Tauri v2 desktop application:

```text
apps/desktop/             Desktop UI and Tauri command boundary
crates/sheets-core/       Workbook, sheet, cell, formatting, and data tools
crates/sheets-formula/    Formula parser, evaluator, functions, and dependencies
crates/sheets-xlsx/       XLSX import and export
crates/sheets-csv/        CSV import and export
crates/sheets-json/       JSON exchange and native workbook format
crates/sheets-chart/      Chart data and SVG previews
crates/sheets-pivot/      Pivot engine
crates/sheets-validation/ Validation and conditional formatting
crates/sheets-i18n/       Locale and accessibility helpers
crates/sheets-print/      Print layout and PDF output
crates/sheets-advanced/   Protection, goal seek, scenarios, and comments
```

See [Architecture](docs/ARCHITECTURE.md) for transaction, recovery, and dependency-graph design.

## Quality gate

Run the complete local gate before opening a pull request:

```bash
./scripts/verify-local.sh
```

The v0.4.0 release-prep baseline is 462 Rust tests, 9 frontend unit tests, and 16 Playwright tests. The Rust suite includes a generated workbook that LibreOffice opens and re-saves. CI requires LibreOffice for that test; local systems without `soffice` report a skip. The platform matrix also runs all 40 desktop library tests on Ubuntu and Windows.

See the [versioned compatibility matrix](docs/COMPATIBILITY_MATRIX.md) for exact fixture and regression IDs.

## Contributing and support

- [Contributing guide](CONTRIBUTING.md)
- [Support guide](SUPPORT.md)
- [Security policy](SECURITY.md)
- [Documentation index](docs/README.md)
- [Release process](docs/RELEASING.md)

Bug reports, compatibility fixtures made with invented data, translations, documentation improvements, and focused performance work are welcome.

## License

900Sheets is licensed under the Apache License 2.0. See [LICENSE](LICENSE).
