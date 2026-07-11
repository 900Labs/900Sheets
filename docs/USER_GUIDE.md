# User guide

This guide describes 900Sheets v0.4.0.

## Start a workbook

Open 900Sheets and select a cell. Type text, a number, `TRUE`, `FALSE`, or a formula beginning with `=`. Press Enter to commit the value and move down one row.

Useful shortcuts:

| Action | Shortcut |
| --- | --- |
| New workbook | Ctrl+N or Command+N |
| Open `.900sheets` workbook | Ctrl+O or Command+O |
| Save workbook | Ctrl+S or Command+S |
| Undo | Ctrl+Z or Command+Z |
| Redo | Ctrl+Y or Command+Y |
| Copy, cut, paste | Ctrl+C, Ctrl+X, Ctrl+V |
| Find and replace | Ctrl+F or Command+F |
| Edit active cell | F2 |
| Clear selected cells | Delete or Backspace |

## Formulas

Enter formulas in a cell or in the formula bar. Examples:

```text
=A1+B1
=SUM(A1:A10)
=Data!A1*2
=SUM('Annual Budget'!$A$1:$A$12)
=IF(C2>0,"Yes","No")
```

Use a simple sheet name directly before `!`. Put a name containing spaces or punctuation in single quotes. Write an embedded single quote twice, as in `'Sam''s Data'!A1`.

If a referenced sheet does not exist, the formula returns a reference error. Cross-sheet circular references are rejected. A formula can expand at most 100,000 references, so very large ranges return a budget error instead of consuming unbounded memory.

## Formatting and sheets

Select one cell or drag across a range. Use the toolbar to change font emphasis, size, colors, alignment, wrapping, borders, and number formats. A range format is committed as one transaction.

Use the tabs at the bottom to add, rename, select, or delete sheets. The Insert menu adds or removes rows and columns. Structural changes move stored cells and formats and rewrite supported A1 references. They also remove coordinate-bound state that cannot be moved safely.

Sheet and structural changes participate in undo and redo in v0.4.0. If an operation is too large for the bounded history, the app rejects it without leaving a partial change.

## Save, open, import, and export

### Save Workbook

Choose **File > Save Workbook** for an editable `.900sheets` file. It stores cells, formulas, formats, active sheet, sheet identities, and supported feature metadata. Use this format for continued editing.

### Open Workbook

Choose **Open Workbook** for a `.900sheets` file. Opening starts a new workbook session and clears the prior undo history. If there are unsaved changes, the app asks before replacing them.

### Open XLSX or JSON

**Open XLSX** and **Open JSON** replace the current workbook. They are not additive imports. The app asks before discarding unsaved changes, clears prior history, opens the selected content, and marks the result as unsaved. Choose **Save Workbook** afterward to create an editable `.900sheets` copy.

XLSX supports multiple sheets, formulas, and direct cell styles, but not every Excel feature. JSON is a data exchange structure and does not carry the complete native feature model.

### Import CSV

**Import CSV** writes CSV, TSV, or text data into the active sheet. It participates in undo and redo as one transaction. CSV cannot represent multiple sheets, formatting, or executable workbook formulas as a native workbook does.

### Export

XLSX, CSV, JSON, and PDF exports create exchange files. Export does not change the current native workbook path or clear undo history. Save a native workbook before exporting if you intend to continue editing.

## Undo and redo

Undo and redo cover cell edits, clear and paste, formatting, sheet changes, structural edits, CSV import, sort, replace, pivot output, comments, protection, locks, and sheet-scoped feature metadata.

History keeps at most 100 transactions and 64 MiB. One transaction may use at most 32 MiB and touch at most 200,000 coordinates. When aggregate limits are reached, the oldest entries are removed. Opening or creating a workbook starts fresh history.

## Autosave and recovery

Recovery protects unsaved work without silently overwriting a workbook you opened.

After a successful edit, the app waits 750 milliseconds for more activity, flushes pending edits, and writes a recovery snapshot in the operating system's app data directory. A dirty workbook also gets a final recovery write when you close the desktop app. If that final write fails, the app asks whether to close without preserving the latest edits.

On startup, the app lists available recoveries newest first. For each prompt:

1. Choose **OK** to restore that recovery.
2. Choose **Cancel** to discard only that recovery and see the next one.
3. After restoring, choose **Save Workbook** to keep it as a normal `.900sheets` file.

Restoring one recovery leaves every unselected recovery untouched. A corrupt recovery is quarantined and will not keep reappearing. If cleanup fails after Save or a workbook replacement, the app shows an error and asks you to use Save Workbook to retry under the same recovery identity.

Recovery snapshots are not a versioned backup system. Save important workbooks normally and keep external backups.

## Data tools and advanced features

- Sort moves values and formats inside the selected range.
- Find and Replace searches the active sheet; replacement is undoable.
- Filters hide nonmatching rows and are stored in native metadata.
- Remove Duplicates operates on the selected range.
- Named ranges are saved bookmarks. Formula name evaluation is not implemented.
- Charts are SVG previews. They are not exported as native Excel charts.
- Pivot output can be created in a generated sheet and is undoable.
- Validation and conditional formatting use the 900Sheets model and are saved in native metadata.
- Comments, protection, and cell locks are scoped to stable sheet identities.
- Sheet protection is an editing deterrent, not encryption.

## If a file does not behave as expected

Keep the original file unchanged and check [Compatibility and known limitations](COMPATIBILITY.md). When reporting a problem, attach only a small workbook containing invented data. Never post customer, financial, credential, or other private material.
