# Compatibility and known limitations

This page describes v0.3.0. It is a statement of current behavior, not a promise that every feature in another spreadsheet application will round-trip.

## File formats

| Format | Read | Write | What is preserved |
| --- | --- | --- | --- |
| `.900sheets` | Yes | Yes | Cells, formulas, formats, sheet names, active sheet, and supported 900Sheets feature metadata |
| `.xlsx` | Yes | Yes | Multiple sheets, supported cell values and formulas, fonts, fills, alignment, wrapping, borders, and common number formats |
| `.csv`, `.tsv`, `.txt` | Yes | Yes | One sheet of text values using the selected delimiter |
| `.json` | Yes | Yes | Data arrays, simple sheet structures, and formulas represented as JSON objects |
| `.pdf` | No | Yes | Fixed print output from the active sheet |

## XLSX notes

The v0.3.0 compatibility tests cover cached formula values, workbook relationship ordering, multiple sheets, custom and built-in number formats, supported fonts and fills, alignment, wrapping, borders, and formatted blank cells.

The following Excel features are not preserved:

- Macros and VBA
- External workbook links
- Excel tables and slicers
- Native Excel charts and pivot caches
- Images, shapes, and embedded objects
- Advanced conditional-formatting and validation records
- Workbook themes beyond directly supported cell colors and fonts
- Cross-sheet formulas

Keep a backup of the original XLSX file when testing a new workflow.

## Formula limitations

- Formula references are limited to the current sheet.
- Named ranges act as range bookmarks in the interface. They are not formula identifiers.
- Imported functions outside the supported set may produce a formula error.
- Structural row and column edits rewrite direct A1 references, including absolute markers. Complex formulas should still be checked after a structural edit.

## Editor limits

The workbook engine accepts up to 1,000,000 rows and 16,384 columns. The v0.3.0 desktop grid exposes 1,000 rows and 52 columns. Imported data outside that visible area remains in the workbook but cannot be edited through the grid.

CSV, JSON, and print output reject dense areas above 5,000,000 cells. This prevents a single sparse high-coordinate cell from producing an unexpectedly large export.

## Persistence limits

- Use `.900sheets` for continued editing.
- There is no autosave or crash recovery.
- Validation, conditional formatting, named ranges, frozen panes, filters, and chart previews are stored as desktop feature metadata. Their current interface model is not a complete Excel-compatible object model.
- Comments are scoped by sheet and stored in native workbooks.
- Sheet protection is a local editing deterrent, not encryption.
- Row or column structural edits clear coordinate-bound feature metadata on the affected sheet so stale rules cannot move to unintended cells.

## Platform support

The release workflow builds a macOS application artifact. It is ad hoc signed and not notarized. Windows and Linux packages are not produced in v0.3.0. Source builds may work on supported Tauri platforms when their system prerequisites are installed.

## Undo limits

Cell value edits, paste, clear, and related operations participate in undo and redo. Structural row and column changes and format-only changes are not fully represented in undo history. Save before applying either operation to a large range.
