# Changelog

## v0.2.0

- Hardened formula dependency graph updates so circular formulas are rejected before storage and dependency edges are cleared on cell clears or non-formula edits.
- Batched sheet data snapshots to include populated-cell formats, avoiding one format IPC call per populated cell during refresh.
- Cleared comments and other derived state on full workbook resets/imports.
- Added explicit CSV import row, column, and cell budgets while keeping the existing maximum CSV byte size.
- Updated the desktop bundle identifier for the 900Labs/900Sheets release line.
