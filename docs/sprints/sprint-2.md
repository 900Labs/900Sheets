# Sprint 2: Grid UI & Cell Editing

## Goal
Enhanced grid with range selection, copy/paste, undo/redo, and sheet tab management.

## Scope Delivered

- [x] Virtualized row rendering (only visible rows rendered for performance)
- [x] Range selection (click-drag, shift-click, shift+arrow keys)
- [x] Range label in formula bar (e.g. A1:B5)
- [x] Copy/cut/paste with system clipboard (Ctrl+C/X/V)
- [x] Paste from external clipboard (TSV format)
- [x] Delete selection (Delete/Backspace key)
- [x] Select all (Ctrl+A)
- [x] Undo/redo stack with toolbar buttons (Ctrl+Z / Ctrl+Y)
- [x] Sheet tab management: add (+), delete (×), rename (double-click)
- [x] Sheet data loading via get_sheet_data IPC
- [x] New Tauri IPC commands: get_sheets, add_sheet, delete_sheet, rename_sheet, clear_cell, get_sheet_data
- [x] Extracted utility modules: lib/types.ts, lib/utils/grid.ts, lib/utils/undoRedo.ts
- [x] Active cell outline distinct from range selection background
- [x] All quality gates pass (cargo test, clippy, fmt, npm run check)

## Validation Results

- cargo test --workspace: 20 tests pass
- cargo clippy --workspace -- -D warnings: clean
- cargo fmt --all -- --check: clean
- npm run check: 0 errors, 0 warnings

## Decisions

- Virtualized rows only (all 26 columns rendered, rows windowed to ~45 visible)
- Undo/redo stores HistoryEntry[] batches (supports multi-cell paste/delete operations)
- System clipboard integration via navigator.clipboard API
- Sheet data loaded on sheet switch via get_sheet_data IPC command
- Active cell gets outline border, range gets background highlight

## Issues Found

None.

## Carry-Over

- Full column virtualization can be added if performance requires (26 cols is fine for now)
- Row/column insert/delete operations deferred to Sprint 7
