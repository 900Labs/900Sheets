# Compatibility and known limitations

This page describes v0.4.0. It records current behavior and limits. It does not promise full fidelity with every feature in another spreadsheet application.

## File workflows

| Format | Workflow | Read | Write | Notes |
| --- | --- | --- | --- | --- |
| `.900sheets` | Open and Save Workbook | Yes | Yes | Preferred format for continued editing. Stores cells, formulas, formats, active sheet, stable sheet identities, and supported feature metadata. |
| `.xlsx` | Open XLSX and Export XLSX | Yes | Yes | Open replaces the current workbook. Save as `.900sheets` before continued editing. Supported values, formulas, sheets, and direct cell styles round-trip. |
| `.csv`, `.tsv`, `.txt` | Import CSV and Export CSV | Yes | Yes | Import writes into the active sheet as one undoable transaction. Export writes one sheet of delimited values. |
| `.json` | Open JSON and Export JSON | Yes | Yes | Open replaces the current workbook. JSON is an exchange structure, not the native format. |
| `.pdf` | Export PDF | No | Yes | Fixed output from the active sheet and current print settings. |

**Open XLSX** and **Open JSON** are replacement operations. They require confirmation when the current workbook has unsaved changes, clear the prior undo history, and leave the imported workbook dirty so it can be saved as `.900sheets`. **Import CSV** changes only the active sheet and participates in undo and redo.

## Formula compatibility

Cross-sheet cell and range references are supported. Use:

```text
=Data!A1
=SUM(Data!A1:A10)
=SUM('Annual Budget'!$A$1:$A$12)
='Sam''s Data'!B2
```

Sheet names containing spaces or punctuation must be enclosed in single quotes. A single quote inside a sheet name is written twice. Absolute row and column markers are preserved. A range cannot begin on one sheet and end on another.

Formula parsing, dependency tracking, and evaluation allow at most 100,000 expanded cell references per formula. A larger range, such as a whole-sheet-scale range, returns an explicit reference-budget error before the engine materializes it. This limit protects memory and applies across local and cross-sheet references.

Cross-sheet dependencies use stable sheet identities. After a source edit, the desktop refreshes formula displays whose dependency chain includes that cell. Workbook imports and transaction commits rebuild and validate the complete workbook graph, including cross-sheet cycle detection. A missing sheet produces a reference error.

Named ranges remain interface bookmarks and are not formula identifiers. Imported functions outside the supported set may return a formula error. Structural row and column edits rewrite supported direct A1 references, including absolute markers; check complex imported formulas after a structural change.

## Native workbook compatibility

The native file format remains format version 1. v0.4.0 reads `.900sheets` files written by v0.3.0. Files without the stable sheet identities introduced during the v0.4 work are assigned noncolliding identities on import. v0.4 preserves stable identities, cross-sheet formula text, sparse cells, formats, active sheet, and supported feature metadata on save and reopen.

Native files are limited to 100 MiB and 10,000,000 combined cells and formats. Unsupported native format versions are rejected rather than guessed.

## Undo and transaction limits

User mutations run against a candidate workbook and commit only after dependency and backend-state validation succeeds. Undo and redo cover:

- Cell edits, clear, paste, and range formatting
- Sheet addition, deletion, and rename
- Row and column structural edits
- Sort and find-and-replace mutations
- CSV import and pivot output
- Comments, protection, cell locks, and sheet-scoped feature metadata

Opening a native workbook, XLSX, or JSON starts a new workbook session and clears history. Export operations do not change history.

History is bounded to 100 transactions and 64 MiB in aggregate. A single transaction is limited to 32 MiB of serialized history and 200,000 changed coordinates. Older transactions are evicted when aggregate limits are reached. A transaction that exceeds a per-operation limit, creates a dependency cycle, or fails backend-state validation is rejected without partially changing the live workbook or moving undo history.

## Recovery behavior

Recovery snapshots use the native workbook representation and are stored in the operating system's per-user app data directory, separate from source and saved workbook files.

- Autosave waits 750 milliseconds after the latest successful edit, then flushes queued mutations before writing.
- Writes use a unique temporary file, file synchronization, and atomic replacement. Unix builds also synchronize the recovery directory. Windows uses `MoveFileExW` with replace and write-through flags.
- A close request flushes pending work and writes a final snapshot if the workbook is dirty. If that fails, the app asks whether to close without the latest recovery.
- Startup lists recoveries newest first. Restoring one leaves unselected snapshots untouched. Canceling a prompt discards only the selected snapshot.
- Corrupt snapshots are quarantined and removed from discovery.
- Cleanup first retires a snapshot from discovery. If deletion fails, Save Workbook presents a retryable error under the same recovery identity.
- A successful native save removes the active recovery. Recovery is not a versioned backup service and does not replace normal saves or external backups.

The filesystem guarantees are covered on Unix and Windows code paths, but power-loss behavior still depends on the operating system, filesystem, and storage device honoring synchronization requests.

## XLSX compatibility

The deterministic XLSX tests cover cached formula values, shared formulas, relationship-based worksheet ordering, multiple sheets, cross-sheet formula text, built-in and custom number formats, fonts, fills, alignment, wrapping, borders, formatted blank cells, shared strings, and package relationships. A generated workbook is also opened and re-saved by LibreOffice in CI.

The following Excel features are not preserved:

- Macros and VBA
- External workbook links
- Excel tables and slicers
- Native Excel charts and pivot caches
- Images, shapes, and embedded objects
- Advanced conditional-formatting and validation records
- Workbook themes beyond directly supported cell colors and fonts

There is no automated Microsoft Excel desktop runner in v0.4.0. Deterministic OOXML tests validate Excel-format structures, and LibreOffice supplies the external application round trip. Keep a backup of an original XLSX when evaluating a new workflow.

## Editor and export limits

The workbook engine accepts up to 1,000,000 rows and 16,384 columns. The desktop grid exposes 1,000 rows and 52 columns. Imported data outside the visible area remains in the workbook but cannot be edited through the grid.

CSV, JSON, and print output reject dense areas above 5,000,000 cells. This prevents a sparse high-coordinate cell from causing an unexpectedly large export.

Validation, conditional formatting, named ranges, frozen panes, filters, and chart previews use the 900Sheets feature model. They are saved in native metadata but are not a complete Excel-compatible object model. Sheet protection is an editing deterrent, not encryption.

## Platform support

The release workflow builds a macOS `.app`. It is ad hoc signed and not notarized. Windows and Linux packages are not produced in v0.4.0.

The complete desktop library suite compiles and runs on `windows-latest` and `ubuntu-latest` in CI. This includes transaction and recovery tests. Linux CI installs the Tauri GTK, WebKitGTK, and SVG development dependencies. This gate validates backend platform behavior, not full GUI behavior across every Windows version, Linux distribution, desktop environment, or filesystem.

The exact automated evidence is recorded in [COMPATIBILITY_MATRIX.md](COMPATIBILITY_MATRIX.md).
