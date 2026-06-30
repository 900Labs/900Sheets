# Sprint 5: CSV & JSON Import/Export

## Goal
Implement CSV and JSON file format import and export.

## Scope Delivered

### CSV
- [x] Import: parse CSV text → Sheet (with configurable delimiter)
- [x] Export: Sheet → CSV text (with configurable delimiter)
- [x] Quoted field support (embedded delimiters, newlines, escaped quotes)
- [x] Delimiter auto-detection (comma, tab, semolon, pipe)
- [x] Multiple delimiter support (comma, tab, semicolon, pipe)
- [x] Empty field handling (skipped, not stored as empty cells)
- [x] CSV roundtrip verified
- [x] 18 unit tests

### JSON
- [x] Import: parse JSON → Workbook
  - Array of arrays (2D grid)
  - Array of objects (key-value pairs per row)
  - Object with key-value pairs
  - Multi-sheet structure ({sheets: [{name, data}]})
- [x] Export: Workbook → pretty-printed JSON
  - Multi-sheet support with sheet names
  - Cell type preservation (numbers, strings, booleans, null)
  - 2D array format for sheet data
- [x] JSON roundtrip verified
- [x] preserve_order feature enabled for serde_json (maintains key insertion order)
- [x] 14 unit tests

## Validation Results

- cargo test --workspace: 136 tests pass (20 core + 74 formula + 10 xlsx + 18 csv + 14 json)
- cargo clippy --workspace -- -D warnings: clean
- cargo fmt --all -- --check: clean
- npm run check: 0 errors, 0 warnings

## Decisions

- CSV: manual parser (no external CSV crate) for minimal dependencies
- JSON: serde_json with preserve_order feature for key ordering
- JSON object import: keys at even columns, values at odd columns
- JSON export: {sheets: [{name, data: [[...]]}]} structure
- CSV delimiter detection samples first 5 lines for consistency

## Issues Found

None.

## Carry-Over

- CSV encoding detection (UTF-8 BOM handling) deferred
- CSV with custom quote characters deferred
- JSON nested objects/arrays as cell values stored as JSON strings
