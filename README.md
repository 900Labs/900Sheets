# 900Sheets

900Sheets is a free, local-first desktop spreadsheet editor. It opens common spreadsheet data, supports formulas and formatting, and works without an account, subscription, telemetry, or constant internet access.

## Why it exists

900 Labs starts from a simple problem: around 900 million people are priced out of modern software. The company builds enterprise-grade open-source tools for developing economies and for anyone who cannot justify another expensive software subscription.

900Sheets is one part of that mission. A spreadsheet is basic working infrastructure for schools, small businesses, community organizations, researchers, and public services. The tool should remain useful on an ordinary computer and should not stop working when a subscription ends.

## Current release

Version 0.3.0 is an early public release for macOS and source builds. It includes:

- A fast Rust workbook engine with a Tauri and Svelte desktop interface
- Multiple sheets, cell editing, keyboard navigation, copy and paste, and find and replace
- 174 formula functions with dependency tracking and circular-reference checks
- Cell fonts, colors, alignment, borders, wrapping, and number formats
- Native `.900sheets` workbooks for continued editing
- XLSX, CSV, and JSON import and export
- PDF export and print configuration
- Sort, filters, pivot tables, chart previews, validation, conditional formatting, named ranges, frozen panes, comments, and sheet protection
- No account requirement and no telemetry

Read [Compatibility and known limitations](docs/COMPATIBILITY.md) before using 900Sheets for important work. Cross-sheet formulas are not supported in v0.3.0, and the packaged macOS build is not yet signed or notarized.

## Install and run

### macOS release artifact

Tagged releases build and archive `900Sheets.app` in GitHub Actions. Download the `900Sheets-v0.3.0-macos` artifact from the release workflow, extract `900Sheets-v0.3.0-macos.zip`, and move the app to Applications. The archive workflow verifies that the app executable remains executable after extraction.

The v0.3.0 artifact is ad hoc signed and not notarized. macOS may require you to right-click the app and choose Open the first time. Only use artifacts published by the `900Labs/900Sheets` repository.

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

Create a release build with:

```bash
npm run tauri:build --prefix apps/desktop
```

## How to use it

1. Select a cell and type a value or a formula such as `=SUM(A1:A10)`.
2. Use the sheet tabs to add, rename, select, or remove sheets.
3. Use the toolbar and menus for formatting, data tools, charts, pivots, validation, and print output.
4. Choose **File > Save Workbook** to create an editable `.900sheets` file.
5. Use **File > Import XLSX** to bring in an Excel workbook.
6. Use the export commands when you need XLSX, CSV, JSON, or PDF output.

The `.900sheets` format is the safest choice for continued editing in 900Sheets. XLSX support is intended for exchange with other spreadsheet applications, but it does not preserve every Excel feature.

The full walkthrough is in the [User guide](docs/USER_GUIDE.md).

## Development

The repository is a Rust workspace with a Svelte 5 desktop application:

```text
apps/desktop/             Tauri v2 and Svelte desktop app
crates/sheets-core/       Workbook, sheet, cell, formatting, and data tools
crates/sheets-formula/    Formula parser, evaluator, functions, and dependencies
crates/sheets-xlsx/       XLSX import and export
crates/sheets-csv/        CSV import and export
crates/sheets-json/       JSON and native workbook formats
crates/sheets-chart/      Chart data and SVG previews
crates/sheets-pivot/      Pivot engine
crates/sheets-validation/ Validation and conditional formatting
crates/sheets-i18n/       Locale and accessibility helpers
crates/sheets-print/      Print layout and PDF output
crates/sheets-advanced/   Protection, goal seek, scenarios, and comments
```

See [Architecture](docs/ARCHITECTURE.md) for the data flow and crate boundaries.

## Quality gate

Run the complete local gate before opening a pull request:

```bash
./scripts/verify-local.sh
```

The v0.3.0 gate covers 418 Rust tests, 4 mutation-queue unit tests, and 8 Playwright interaction tests, plus Rust formatting, clippy with warnings denied, Svelte and TypeScript diagnostics, and a production frontend build. The Rust suite includes an external LibreOffice XLSX round trip. CI installs LibreOffice and requires that test to run; local systems without `soffice` report a skip.

If Playwright cannot find Chromium, install it with:

```bash
npx --prefix apps/desktop playwright install chromium
```

## Contributing and support

- [Contributing guide](CONTRIBUTING.md)
- [Support guide](SUPPORT.md)
- [Security policy](SECURITY.md)
- [Documentation index](docs/README.md)
- [Release process](docs/RELEASING.md)

Bug reports, compatibility fixtures, translations, documentation improvements, and focused performance work are welcome.

## License

900Sheets is licensed under the Apache License 2.0. See [LICENSE](LICENSE).
