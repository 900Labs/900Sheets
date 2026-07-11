# User guide

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
=IF(C2>0,"Yes","No")
```

References must point to cells on the same sheet in v0.3.0. A formula such as `=Sheet2!A1` is rejected with a clear error.

## Formatting

Select one cell or drag across a range. Use the formatting toolbar to change font emphasis, size, colors, alignment, wrapping, borders, and number formats. Range formatting is sent to the workbook engine as one atomic batch.

## Sheets and structure

Use the tabs at the bottom of the window to add, rename, select, or delete sheets. The Insert menu can add or remove rows and columns. Structural changes move stored formatting and update affected A1 references. To prevent coordinate-bound rules from silently targeting the wrong cells, a structural edit clears comments, locks, validation, conditional formatting, named ranges, filters, frozen panes, and chart preview state on that sheet. Structural edits are not fully covered by undo history in v0.3.0, so save before a large structural edit.

## Save and exchange files

Use **File > Save Workbook** for an editable `.900sheets` file. This format stores cells, formulas, formats, sheet information, and the supported 900Sheets feature metadata.

Use import and export for interoperability:

- XLSX imports and exports cells, formulas, multiple sheets, and supported styles.
- CSV imports or exports one sheet and cannot store formulas as executable workbook logic, formatting, or multiple sheets.
- JSON is a data exchange format. It is not the native workbook format.
- PDF creates a fixed output document for sharing or printing.

Save a native workbook before exporting if you want to continue editing later.

## Data tools

- Sort moves values and formats inside the selected range.
- Find and Replace searches the active sheet.
- Filters hide nonmatching rows in the current session and are stored in native workbook metadata.
- Remove Duplicates operates on the selected range.
- Named ranges are saved bookmarks for ranges. Formula name evaluation is not implemented.

## Charts, pivots, and validation

Charts are SVG previews generated from the selected data range. Their current configuration and preview are stored in native workbook metadata, but they are not exported as native Excel chart objects.

Pivot tables create calculated output from a selected range. Data validation and conditional formatting rules are available in the desktop interface and are stored in native workbook metadata. XLSX round trips do not preserve these advanced rules in v0.3.0.

## Recovering from a problem

900Sheets v0.3.0 does not include autosave or crash recovery. Save regularly and keep backups of important workbooks. If an import fails, do not overwrite the source file. Open an issue with a small workbook containing invented data that reproduces the failure.
