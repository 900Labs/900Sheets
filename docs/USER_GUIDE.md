# User guide

This guide describes 900Sheets v0.5.0.

## Start a workbook

Open 900Sheets and select a cell. Type text, a number, `TRUE`, `FALSE`, or a formula beginning with `=`. Press Enter to commit the value and move down one row.

The workbook header shows the document name and save state. Use the visible **Open** and **Save** controls for native workbooks; other formats remain in the File menu. Unsaved changes and saving progress are shown separately from the filename. The bottom zoom controls change the view without changing your data.

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
| Go to a cell or range | Ctrl+G, Command+G, or F5 |
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
=B2*10%
```

Use a simple sheet name directly before `!`. Put a name containing spaces or punctuation in single quotes. Write an embedded single quote twice, as in `'Sam''s Data'!A1`.

If a referenced sheet does not exist, the formula returns a reference error. Cross-sheet circular references are rejected. A formula can expand at most 100,000 references, so very large ranges return a budget error instead of consuming unbounded memory.

Percentage is a postfix operator: `=10%` is `0.1` and `=200*10%` is `20`. Use `MOD(number, divisor)` for a remainder. Formulas are also bounded by input size, tokens, expression depth and calculation work; see [Compatibility](COMPATIBILITY.md) for the exact limits.

## Start from a template

The first-run guide offers a blank workbook or one of four starter templates:

- School Budget
- Small Business Accounts
- Community Project
- Household Planning

Every sample uses invented data. A template is inserted at the selected cell, and its relative formulas are rebased to the insertion point. Replace the sample values before relying on its totals. The first-run completion preference stays on the local computer and does not create an account or send data elsewhere. You can open the template chooser again after completing the guide.

## Formatting and sheets

Select one cell or drag across a range. Use the toolbar to change font emphasis, size, colors, alignment, wrapping, borders, and number formats. A range format is committed as one transaction.

Use the tabs at the bottom to add, rename, select, or delete sheets. The Insert menu adds or removes rows and columns. Structural changes move stored cells and formats and rewrite supported A1 references. If the sheet has coordinate-bound features that cannot be moved safely, the app rejects the structural edit and keeps those features. Review the reported features before explicitly removing them or editing a simpler sheet.

Sheet and structural changes participate in undo and redo. If an operation is too large for the bounded history, the app rejects it without leaving a partial change.

## Navigate the full grid

The editor covers rows 1 through 1,000,000 and columns A through XFD. It renders only the rows and columns near the viewport, so reaching a distant coordinate does not create a cell element for every position in between.

Use the address field or press Ctrl+G, Command+G, or F5. Enter one cell, such as `B25000`, or one range, such as `A1:F20`, and press Enter. Addresses outside `A1:XFD1000000` are rejected. Arrow keys, Home, Page Up, and Page Down continue from the selected location. Tab and Shift+Tab leave the grid so keyboard users can reach the surrounding controls. Frozen panes are limited to the first 20 rows and first 20 columns.

Operations that must visit every coordinate, such as formatting, copy, cut, paste, charts, pivots, filters, duplicate removal, validation, conditional formatting, and range locking, accept at most 200,000 selected cells. Sparse deletion accepts at most 200,000 populated cells. Break larger work into smaller ranges.

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

Before the file picker opens, 900Sheets runs an export preflight:

- CSV, JSON, and PDF report how many dense cells the export would process.
- Work at or above 1,000,000 dense cells produces a confirmation warning.
- A dense area above the 5,000,000-cell safety limit is blocked before export begins.
- XLSX reports stored cell and format coordinates because it writes sparse workbook data rather than expanding a dense grid.

If a sparse workbook contains one cell at a very high coordinate, CSV or JSON may still be large because those formats represent the rectangle up to that coordinate. Clear or move the distant cell if preflight blocks the export.

## Undo and redo

Undo and redo cover cell edits, clear and paste, formatting, sheet changes, structural edits, CSV import, sort, replace, pivot output, comments, protection, locks, and sheet-scoped feature metadata.

History keeps at most 100 transactions and 64 MiB. One transaction may use at most 32 MiB and touch at most 200,000 coordinates. When aggregate limits are reached, the oldest entries are removed. Opening or creating a workbook starts fresh history.

## Recovery and rotating backups

Recovery protects unsaved work without silently overwriting a workbook you opened.

After a successful edit, the app waits for the configured interval, flushes pending edits, and writes a recovery snapshot in the operating system's app data directory. The default interval is 750 milliseconds. Under **Tools > Recovery and Backups**, choose 0.75 seconds, 2 seconds, 5 seconds, or 15 seconds. The setting is local to the computer. A dirty workbook also gets a final recovery write when you close the desktop app. If that final write fails, the app asks whether to close without preserving the latest edits.

On startup, the app lists available recoveries newest first. For each prompt:

1. Choose **OK** to restore that recovery.
2. Choose **Cancel** to keep that recovery for later and see the next one. Deferred snapshots remain available in **Tools > Recovery and Backups**.
3. After restoring, choose **Save Workbook** to keep it as a normal `.900sheets` file.

The **Recovery and Backups** panel remains available after startup. It shows each snapshot's time and size. Select its record to inspect sheet names without replacing the workbook. **Restore** asks before discarding unsaved changes, loads the snapshot as an unsaved replacement, and leaves retained recoveries in place until an explicit save or delete. **Delete** asks for confirmation and affects only the selected snapshot.

Restoring one recovery leaves every unrelated recovery untouched. A corrupt recovery is quarantined and will not keep reappearing. If cleanup fails after Save or a workbook replacement, the app shows an error and asks you to use Save Workbook to retry under the same recovery identity.

Rotating backups protect previously saved native versions. After a `.900sheets` save succeeds, the app attempts to create a private backup and keeps the newest five versions for that saved document. The **Saved-workbook backups** section shows the workbook name, time, and size. Select its record to inspect sheet names, choose **Restore copy** to open it as an unsaved replacement, or delete it explicitly. Restoring does not delete the backup. If backup creation or rotation fails after the primary save, the app reports a warning without claiming that the already-written workbook failed to save.

Recovery follows dirty session state. Rotating backups are attempted after successful native saves, with any failure reported separately from the completed save. They are separate local safeguards, and neither replaces external backups for important files.

## Data tools and advanced features

- Sort moves values and formats inside the selected range.
- Find and Replace searches the active sheet; replacement is undoable.
- Filters hide nonmatching rows and are stored in native metadata.
- Remove Duplicates operates on the selected range.
- Named ranges are saved bookmarks. Formula name evaluation is not implemented.
- Charts built in the panel are SVG previews and are not exported as native Excel chart objects. Imported native Excel charts (bar, column, line, area, pie, doughnut) are preserved through XLSX round trips.
- Pivot output can be created in a generated sheet and is undoable.
- Validation and conditional formatting use the 900Sheets model, are saved in native metadata, and round-trip through XLSX for the documented subset.
- Comments, protection, and cell locks are scoped to stable sheet identities.
- Sheet protection is an editing deterrent, not encryption.

## Language and accessibility

Choose **Tools > Locale Settings** to select English, Swedish, or Spanish. The preference stays on the local computer. It changes verified grid-navigation and core accessibility labels, not workbook content or the complete application interface.

The virtualized grid uses one keyboard focus target and exposes its row count, column count, active cell, selection state, and cell labels to assistive technology. Dialogs keep keyboard focus inside while open, close with Escape, and return focus when dismissed. Screen readers and platform keyboard behavior still vary, so include the operating system and assistive technology when reporting a problem.

## If a file does not behave as expected

Keep the original file unchanged and check [Compatibility and known limitations](COMPATIBILITY.md). When reporting a problem, attach only a small workbook containing invented data. Never post customer, financial, credential, or other private material.
