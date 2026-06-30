# Sprint 1: Project Scaffold & Core Data Model

## Goal
Bootable Tauri app with Rust workspace and basic workbook model.

## Scope Delivered

- [x] Tauri v2 + Svelte 5 project initialized
- [x] Rust workspace with 8 crates (sheets-core, sheets-formula, sheets-xlsx, sheets-csv, sheets-json, sheets-chart, sheets-pivot, sheets-fixtures)
- [x] sheets-core: Workbook, Sheet, CellValue, CellAddress with A1 notation
- [x] Cell type inference (Number, Text, Boolean, Formula)
- [x] Sparse cell storage via HashMap
- [x] Basic grid UI with 26 columns × 100 rows
- [x] Cell selection with keyboard navigation (arrows, Tab, Enter)
- [x] Cell editing mode (double-click, F2, or type to start)
- [x] Formula bar showing cell reference and content
- [x] Sheet tab UI
- [x] Tauri IPC commands: new_workbook, get_cell, set_cell
- [x] Project documentation: README, ARCHITECTURE, ROADMAP, QUALITY_GATE, SPRINT_PROCESS
- [x] CONTRIBUTING.md, SECURITY.md, CODE_OF_CONDUCT.md
- [x] Apache-2.0 license
- [x] CI workflow (GitHub Actions)
- [x] Verification script (verify-local.sh)
- [x] Unit tests for address parsing, cell types, sheet operations, workbook operations

## Validation Results

- cargo test --workspace: all sheets-core tests pass
- cargo clippy: clean
- cargo fmt: clean

## Decisions

- Custom Rust formula engine (no IronCalc dependency)
- Apache-2.0 license (permissive, matches 900Invoice)
- XLSX as native file format
- Sparse HashMap storage for cells (memory-efficient for low-resource hardware)
- Default grid: 26 columns × 100 rows for low-memory display

## Issues Found

None.

## Carry-Over

None. Sprint 1 complete.
