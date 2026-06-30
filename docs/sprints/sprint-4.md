# Sprint 4: XLSX Import/Export

## Goal
Implement XLSX (OOXML) file format import and export using zip and roxmltree.

## Scope Delivered

- [x] Import: parse XLSX ZIP archive → Workbook
  - Read shared strings table (xl/sharedStrings.xml)
  - Read workbook.xml for sheet names
  - Read workbook.xml.rels for sheet file paths
  - Parse worksheet XML for cell data (numbers, strings, booleans, formulas, errors)
  - Support shared strings, inline strings, and direct values
  - File size limit (100MB) and cell count limit (10M) for safety
- [x] Export: Workbook → XLSX ZIP archive
  - Generate [Content_Types].xml, _rels/.rels, xl/workbook.xml
  - Generate workbook.xml.rels with sheet relationships
  - Shared strings deduplication for text cells
  - Generate worksheet XML with proper cell types (n, s, b, e, f)
  - XML escaping for special characters
- [x] Export→Import roundtrip verified with tests
- [x] Multi-sheet support verified
- [x] 10 unit tests (6 new in sheets-xlsx)

## Validation Results

- cargo test --workspace: 104 tests pass (20 core + 74 formula + 10 xlsx)
- cargo clippy --workspace -- -D warnings: clean
- cargo fmt --all -- --check: clean
- npm run check: 0 errors, 0 warnings

## Decisions

- Shared strings deduplication on export (HashMap index)
- Cell references parsed from A1 notation (e.g. "B3" → row=2, col=1)
- Formula cells stored with leading "=" in raw field
- File size and cell count limits for safety against malicious files
- XML escaping for &, <, >, ", '

## Issues Found

None.

## Carry-Over

- Style/format preservation deferred to Sprint 6
- Number formats not preserved on import
- Column widths and row heights not preserved
- Charts and images not supported
