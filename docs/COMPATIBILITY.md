# Compatibility and known limitations

This page describes v0.5.0. It records current behavior and limits. It does not promise full fidelity with every feature in another spreadsheet application.

## File workflows

| Format | Workflow | Read | Write | Notes |
| --- | --- | --- | --- | --- |
| `.900sheets` | Open and Save Workbook | Yes | Yes | Preferred format for continued editing. Stores cells, formulas, formats, active sheet, stable sheet identities, and supported feature metadata. |
| `.xlsx` | Open XLSX and Export XLSX | Yes | Yes | Open replaces the current workbook. Save as `.900sheets` before continued editing. Supported values, formulas, sheets, direct cell styles, validation, and conditional formatting round-trip. |
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

The native file format remains format version 1. v0.5.0 reads `.900sheets` files written by v0.3.0 and v0.4.0. Files without current stable sheet identities are assigned noncolliding identities on import. v0.5 preserves stable identities, cross-sheet formula text, sparse cells, formats, active sheet, and supported feature metadata on save and reopen.

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

## Recovery and rotating backups

Recovery snapshots use the native workbook representation and are stored in the operating system's per-user app data directory, separate from source and saved workbook files.

- Recovery autosave defaults to 750 milliseconds after the latest successful edit. **Tools > Recovery and Backups** offers 0.75, 2, 5, and 15 second intervals without allowing an old timer to write stale state. Queued mutations still flush before every write.
- Writes use a unique temporary file, file synchronization, and atomic replacement. Unix builds also synchronize the recovery directory. Windows uses `MoveFileExW` with replace and write-through flags.
- A close request flushes pending work and writes a final snapshot if the workbook is dirty. If that fails, the app asks whether to close without the latest recovery.
- Startup lists recoveries newest first. Restoring one leaves unselected snapshots untouched. The **Recovery and Backups** panel remains available after startup and can inspect sheet summaries, restore, or delete one retained snapshot.
- Corrupt snapshots are quarantined and removed from discovery.
- Cleanup first retires a snapshot from discovery. If deletion fails, Save Workbook presents a retryable error under the same recovery identity.
- A successful native save removes the active recovery.

Each successful native save also attempts to create a private rotating backup. The backup store:

- Groups backups by an opaque key derived from the absolute save path but does not persist that path.
- Retains the newest five backups for each saved document.
- Lets the user inspect workbook and sheet summary information, restore a selected version, or delete it.
- Reports backup-write or rotation failures separately after the primary workbook save has succeeded.
- Validates a restore candidate before replacing the live workbook.

Recovery protects dirty session state after a crash or interrupted close. Rotating backups preserve a small set of previously saved native versions. Neither feature is cloud sync, and neither replaces normal external backups.

The filesystem guarantees are covered on Unix and Windows code paths, but power-loss behavior still depends on the operating system, filesystem, and storage device honoring synchronization requests.

## XLSX compatibility

The deterministic XLSX tests cover cached formula values, shared formulas, relationship-based worksheet ordering, multiple sheets, cross-sheet formula text, built-in and custom number formats, fonts, fills, alignment, wrapping, borders, formatted blank cells, shared strings, package relationships, supported validation records, supported conditional-formatting rules, and differential formatting. A generated workbook is also opened and re-saved by LibreOffice in CI.

The supported XLSX validation subset includes whole number, decimal, list, date, time, text length, and custom rules; the standard comparison operators; formula fields; allow-blank and dropdown behavior; error styles and messages; prompt text; and multiple worksheet ranges. Literal list values are enforced in 900Sheets. Range-backed and defined-name list sources are preserved for XLSX round trips, but 900Sheets does not resolve those sources for local validation yet and therefore does not report false literal-list errors for them. Custom formula validation rules are also preserved, but their formulas are not evaluated locally yet.

The supported conditional-formatting subset includes cell-value, formula, text contains, text does not contain, text begins with, text ends with, blanks, nonblanks, and duplicate-value rules. Supported direct differential font, fill, number-format, and border fields round-trip. Unsupported rule types are rejected on export rather than silently omitted from a 900Sheets-authored document.

The supported Excel table subset includes table name and display name, the worksheet range, header and totals row visibility, column names and identifiers, per-column totals-row functions (sum, average, count, count numbers, max, min, standard deviation, variance, and custom) and totals-row labels, the auto-filter range, and table style information (style name and the first-column, last-column, row-stripe, and column-stripe flags). Tables are preserved through XLSX round trips by reading each worksheet's table relationship parts. Tables authored outside workbook bounds or with inconsistent column counts are skipped on import and rejected on export. 900Sheets preserves table metadata for interoperability; it does not yet evaluate structured references (`TableName[Column]`) in formulas, render table styles in the grid, or expose table authoring in the desktop editor.

The following Excel features are not preserved:

- Macros and VBA
- External workbook links
- Table slicers
- Native Excel charts and pivot caches
- Images, shapes, and embedded objects
- Conditional-formatting features outside the documented subset, including color scales, data bars, icon sets, top or bottom rules, time periods, and above-average rules
- Validation extensions and records outside the documented subset
- Workbook themes beyond directly supported cell colors and fonts

There is no automated Microsoft Excel desktop runner in v0.5.0. Deterministic OOXML tests validate Excel-format structures, and LibreOffice supplies the external application round trip. The repeatable manual procedure in [XLSX_COMPATIBILITY_CHECK.md](XLSX_COMPATIBILITY_CHECK.md) uses an invented workbook and keeps observed Excel or LibreOffice behavior separate from automated evidence. Keep a backup of an original XLSX when evaluating a new workflow.

## Editor and export limits

The workbook engine and virtualized desktop grid accept 1,000,000 rows and 16,384 columns, from `A1` through `XFD1000000`. Only a bounded viewport window is rendered at once. **Go To** accepts one cell or one normalized range inside those bounds. Frozen panes are limited to the first 20 rows and first 20 columns.

Frontend operations that must enumerate every coordinate in a selection are limited to 200,000 cells. This applies to range formatting, copy or cut, paste, chart and pivot input, filters, duplicate removal, validation, conditional formatting, and range locks. Sparse deletion is limited to 200,000 populated cells. The limit is independent of the sparse grid bounds and prevents a full-column or full-sheet selection from allocating an unbounded coordinate list.

CSV, JSON, and print output reject dense areas above 5,000,000 cells. Before export, preflight reports the estimated dense work and warns when it reaches 1,000,000 cells. This prevents a sparse high-coordinate cell from causing an unexpectedly large export. XLSX stays sparse, so its preflight reports stored cell and format coordinates instead of applying the dense-grid limit.

Validation, conditional formatting, named ranges, frozen panes, filters, and chart previews use the 900Sheets feature model. They are saved in native metadata but are not a complete Excel-compatible object model. Sheet protection is an editing deterrent, not encryption.

## Interface language and accessibility

The locale setting persists English, Swedish, or Spanish labels for grid navigation and core accessibility text. It does not translate workbook content or the complete application interface. The grid exposes row and column counts, indexed headers and cells, active-cell ownership, selection state, and localized cell labels. Dialogs trap keyboard focus and restore focus when closed. These behaviors improve access but do not constitute certification against every assistive-technology combination.

## Platform and release workflow

The v0.5.0 release workflow is configured to produce a macOS app archive and Windows NSIS installer, calculate and verify SHA-256 files, and record signing provenance.

- With Apple release credentials, the macOS job can Developer ID sign, submit for notarization, staple the result, and record that provenance. Without them, it records ad hoc signing and no notarization, uploads the artifact for inspection, and refuses GitHub Release publication.
- Automated publication requires the macOS provenance to report Developer ID signing and stapled notarization.
- With configured Windows signing credentials, the Windows job can Authenticode sign and verify the installer. Without them, it records an unsigned installer, which may be published only with that explicit provenance. The job is also configured to perform a silent temporary install, check the installed application version, and uninstall it before upload.
- Linux source and backend gates remain available, but no Linux package is configured for v0.5.0.

Workflow capability is not a verification result for a particular release. Release notes must state which credential path ran, which artifacts were published, and whether any clean-machine installation was performed. Source-tree documentation does not claim Microsoft Excel desktop verification, a successful Windows installation, Developer ID notarization, or hosted CI completion before that evidence exists.

The exact automated evidence is recorded in [COMPATIBILITY_MATRIX.md](COMPATIBILITY_MATRIX.md).
