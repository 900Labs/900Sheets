# Sprint 8: MVP Polish, Testing & Documentation

## Goal
Wire all backend crates into the Tauri IPC layer, verify full system integration, and finalize documentation.

## Scope Delivered

### Tauri IPC Commands (apps/desktop/src-tauri/src/lib.rs)
- [x] Formula evaluation: `evaluate_formula` — parse and evaluate formula in sheet context
- [x] XLSX import/export: `import_xlsx`, `export_xlsx`
- [x] CSV import/export: `import_csv_data`, `export_csv` (with configurable delimiter)
- [x] JSON import/export: `import_json_data`, `export_json`
- [x] Cell formatting: `set_cell_format`, `get_cell_format`
- [x] Data tools: `sort_data`, `find_in_sheet_cmd`, `replace_in_sheet_cmd`
- [x] TauriProvider implementing CellProvider trait for formula evaluation against sheet data
- [x] Updated `get_sheet_data` to include display string and cell_type
- [x] 12 new IPC commands added (21 total)

### Integration
- [x] sheets-formula wired into Tauri backend (EvaluCellProvider trait)
- [x] sheets-xlsx wired into Tauri backend (import/export)
- [x] sheets-csv wired into Tauri backend (import/export with delimiter)
- [x] sheets-json wired into Tauri backend (import/export)
- [x] sheets-core data_tools wired into Tauri backend (sort, find, replace)
- [x] sheets-core format wired into Tauri backend (set/get cell format)

## Validation Results

- cargo test --workspace: 178 tests pass (62 core + 74 formula + 10 xlsx + 18 csv + 14 json)
- cargo clippy --workspace -- -D warnings: clean
- cargo fmt --all -- --check: clean
- npm run check: 0 errors, 0 warnings
- cargo build -p sheets-desktop: clean build

## Full IPC Command List

| Command | Description |
|---------|-------------|
| `new_workbook` | Create a fresh workbook |
| `get_sheets` | List all sheets |
| `add_sheet` | Add a new sheet |
| `delete_sheet` | Delete a sheet |
| `rename_sheet` | Rename a sheet |
| `get_cell` | Get cell raw value |
| `set_cell` | Set cell value |
| `clear_cell` | Clear a cell |
| `get_sheet_data` | Get all cell data for a sheet |
| `evaluate_formula` | Evaluate a formula in sheet context |
| `import_xlsx` | Import XLSX file |
| `export_xlsx` | Export workbook as XLSX |
| `import_csv_data` | Import CSV with delimiter |
| `export_csv` | Export sheet as CSV |
| `import_json_data` | Import JSON |
| `export_json` | Export workbook as JSON |
| `set_cell_format` | Set cell formatting |
| `get_cell_format` | Get cell formatting |
| `sort_data` | Sort sheet by column |
| `find_in_sheet_cmd` | Find text in sheet |
| `replace_in_sheet_cmd` | Replace text in sheet |

## Issues Found

None.

## Carry-Over (Post-MVP)

- Frontend UI for import/export dialogs
- Frontend UI for formatting controls (bold, italic, colors, alignment)
- Frontend UI for sort/filter/find/replace dialogs
- Formula bar with live evaluation
- Column width and row height management
- Conditional formatting
- Charts and pivot tables
- Multi-column sort
- Advanced filter (OR logic, custom predicates)
- Undo/redo
- Clipboard support (copy/paste)
- Keyboard shortcuts
- Auto-save
