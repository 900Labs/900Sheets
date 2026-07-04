# 900Sheets

Offline-first spreadsheet for low-resource environments.

Free. Local. Open.

900Sheets is a local-first desktop spreadsheet application designed for communities where expensive subscriptions, constant connectivity, and high-end hardware are not realistic assumptions.

Built by [900 Labs](https://www.900labs.com) — building enterprise-grade open source tools for the 900 million+ people in developing economies who are priced out of the software that modern businesses depend on.

## Features

### Core (MVP)

- Tauri v2 desktop shell with a Rust backend and Svelte 5 frontend.
- Custom Rust workbook/sheet/cell data model with A1 address notation.
- Grid UI (52 cols × 1000 rows) with cell selection, keyboard navigation, and inline editing.
- Formula bar for entering values and formulas.
- Sheet tab management (add, rename, delete).
- Formula engine with 174 built-in functions (math, logical, text, info, statistical, financial, date/time, lookup).
- Formula dependency graph with circular reference detection and topological sort.
- XLSX (OOXML) import/export with shared strings and multi-sheet support.
- CSV and JSON import/export with delimiter detection and multi-line field support.
- Cell formatting and number formats (general, number, currency, percentage, date, time, scientific).
- Sort, find/replace, copy/paste, undo/redo.
- No telemetry by default.
- Apache-2.0 licensing.

### Post-MVP

- **Pivot tables** with aggregation (sum, count, average, min, max, product), row/column grouping, filtering, and grand totals.
- **Charts & data visualization** — bar, line, pie, scatter, area, column, and doughnut charts with SVG generation and series extraction from sheet data.
- **Statistical, financial, engineering, date/time, lookup, and array functions** — expanding the formula engine to 174 total functions.
- **Advanced conditional formatting** — cell rules, top/bottom, data bars, color scales, icon sets, and formula-based rules. **Data validation** with input constraints, dropdowns, and error alerts.
- **Internationalization** — 22 supported locales, 88 translation keys, locale-aware number/currency/percentage/date/time formatting, RTL support (Arabic, Hebrew). **Accessibility** — ARIA labels, screen reader support, keyboard navigation metadata.
- **Print & PDF export** — page layout (A4, A3, Letter, Legal, Tabloid), portrait/landscape, margins, scaling (fit-to-page, fit-to-width, custom %), headers/footers with `{page}`/`{pages}` templates, gridlines, row/column headings, print areas, repeating rows/columns, HTML print rendering, and minimal PDF 1.4 generation.
- **Advanced features** — sheet protection with password hashing and granular permissions, cell locking, goal seek (Newton's method with bisection fallback), scenario manager for what-if analysis, and cell comments/notes with author tracking.
- **Cell comments/notes** with author tracking, visibility toggling, and bulk management.

## Architecture

900Sheets uses a Rust-owned data model with a Svelte 5 projection UI:

1. `sheets-core` owns the workbook/sheet/cell data model.
2. `sheets-formula` provides the formula parser, evaluator, and dependency graph.
3. `sheets-xlsx` handles XLSX (OOXML) import/export at the Rust boundary.
4. `sheets-csv` and `sheets-json` handle CSV and JSON import/export.
5. `sheets-chart` provides chart rendering and SVG generation.
6. `sheets-pivot` provides the pivot table engine.
7. `sheets-validation` handles data validation and conditional formatting.
8. `sheets-i18n` provides locale management, translations, and accessibility labels.
9. `sheets-print` handles print layout and PDF generation.
10. `sheets-advanced` provides sheet protection, goal seek, scenarios, and comments.
11. The Svelte 5 frontend is an editing projection over the Rust model.

Repository layout:

```
900Sheets/
├── apps/desktop/              # Tauri v2 + Svelte 5 desktop app
│   └── src-tauri/             # Rust backend (Tauri commands, IPC)
├── crates/
│   ├── sheets-core/           # Workbook/sheet/cell model, types, commands, undo/redo
│   ├── sheets-formula/        # Formula parser, evaluator, dependency graph, functions
│   ├── sheets-xlsx/           # XLSX read/write (OOXML) — import/export boundary
│   ├── sheets-csv/            # CSV import/export with delimiter detection
│   ├── sheets-json/           # JSON import/export
│   ├── sheets-chart/          # Chart rendering and data visualization
│   ├── sheets-pivot/          # Pivot table engine
│   ├── sheets-validation/     # Data validation and conditional formatting
│   ├── sheets-i18n/           # Internationalization and accessibility
│   ├── sheets-print/          # Print layout and PDF export
│   └── sheets-advanced/       # Sheet protection, goal seek, scenarios, comments
├── docs/                      # Public documentation, ADRs, sprint records
├── scripts/                   # Validation and release-preflight scripts
└── .github/                   # CI/CD workflows and templates
```

## Build From Source

Prerequisites:
- Rust 1.88+ — install from [rustup.rs](https://rustup.rs)
- Node.js 20.19+, 22.12+, or 24+ — install from [nodejs.org](https://nodejs.org)
- Tauri CLI v2: `cargo install tauri-cli --version "^2"`
- Tauri v2 system dependencies — see [v2.tauri.app/start/prerequisites](https://v2.tauri.app/start/prerequisites)

```bash
git clone https://github.com/900Labs/900Sheets.git
cd 900Sheets
npm install --prefix apps/desktop
npm run tauri:dev --prefix apps/desktop
```

## Validation

Run the local quality gate before opening a pull request:

```bash
./scripts/verify-local.sh
```

The local quality gate includes Rust formatting, clippy, Rust tests, Svelte type checks, the frontend build, and Playwright spreadsheet interaction smokes. On a fresh machine, if Playwright reports that Chromium is missing, run:

```bash
npx --prefix apps/desktop playwright install chromium
```

## Test Suite

The project includes 387 unit tests across Rust crates and the desktop backend, plus 3 Playwright spreadsheet interaction smoke tests:

| Crate | Tests | Description |
|-------|-------|-------------|
| sheets-core | 65 | Workbook, sheet, cell, address, format |
| sheets-formula | 111 | Parser, evaluator, 174 functions |
| sheets-xlsx | 12 | Import/export round-trip and import resource limits |
| sheets-csv | 20 | Delimiter detection, import/export |
| sheets-json | 18 | Import/export and import resource limits |
| sheets-chart | 9 | Chart types, SVG generation |
| sheets-pivot | 11 | Grouping, aggregation, filtering |
| sheets-validation | 35 | Data validation, conditional formatting |
| sheets-i18n | 44 | Locales, translations, formatting, accessibility |
| sheets-print | 25 | Page layout, HTML, PDF generation |
| sheets-advanced | 32 | Protection, goal seek, scenarios, comments |
| sheets-desktop backend | 5 | Tauri protection/path helper behavior |
| Playwright E2E | 3 | Cell typing, Delete/Backspace clearing, compact toolbar menu viewport checks |

## Documentation

- [Documentation index](docs/README.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Roadmap](docs/ROADMAP.md)
- [Quality gate](docs/QUALITY_GATE.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Contributions must include matching documentation updates when behavior, workflows, public APIs, or contributor expectations change.

## License

900Sheets is licensed under Apache License 2.0. See [LICENSE](LICENSE) for details.
