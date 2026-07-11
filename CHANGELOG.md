# Changelog

## v0.3.0

### Added

- Added the sparse `.900sheets` workbook format for continued editing.
- Added dirty-state indication and unsaved-change confirmation for destructive workbook actions and app close events.
- Added atomic batch commands for cell changes, formatting, and complete sheet snapshots.
- Added persisted native metadata for comments, protection, cell locks, validation rules, conditional rules, named ranges, frozen panes, filters, and chart previews.
- Added release compatibility, user, support, and maintainer documentation.

### Fixed

- Preserved XLSX formulas when cells include cached `<v>` values.
- Resolved workbook sheets through their relationship IDs instead of relationship file order.
- Corrected XLSX font, fill, alignment, wrapping, border, number-format, and formatted-blank-cell round trips.
- Refreshed formula displays after edits, clears, paste, undo, redo, sort, replace, import, and structural changes.
- Made row and column edits preserve formatting and rewrite affected A1 formula references.
- Made sort operate on the selected range and move cell formats with their values.
- Scoped comments by sheet.
- Guarded dense CSV, JSON, and print exports against sparse high-coordinate expansion.
- Batched range formatting to avoid one IPC request per cell.

### Changed

- Cross-sheet references now fail with a clear unsupported-feature message.
- `Ctrl+S` saves the editable `.900sheets` format. XLSX is an explicit export.
- CI and the local gate use `npm ci` for lockfile-reproducible installs.
- Package and desktop bundle versions are now 0.3.0.

### Known limitations

- Cross-sheet formulas are not supported.
- The visible editor grid is limited to 52 columns and 1,000 rows.
- Structural edits and format-only operations are not fully represented in undo history.
- Autosave and crash recovery are not included.
- The macOS artifact is ad hoc signed and not notarized. Windows and Linux packages are not produced by the release workflow.

## v0.2.0

- Hardened formula dependency updates and circular-reference rejection.
- Included populated-cell formats in sheet snapshots.
- Cleared derived state on complete workbook resets and imports.
- Added CSV import row, column, cell, and byte budgets.
- Updated the desktop bundle identifier for the 900Labs release line.
