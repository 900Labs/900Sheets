# Roadmap

## MVP (Sprints 1–8)

### Sprint 1: Project Scaffold & Core Data Model ✅
- Tauri v2 + Svelte 5 project initialized
- Rust workspace with all crate stubs
- sheets-core: Workbook, Sheet, CellValue, CellAddress
- Basic grid UI with sheet tabs
- Tauri IPC: new_workbook, get_cell, set_cell
- Project docs and CI

### Sprint 2: Grid UI & Cell Editing ✅
### Sprint 3: Formula Engine — Parser & Basic Functions ✅
### Sprint 4: XLSX Import/Export ✅
### Sprint 5: CSV & JSON Import/Export ✅
### Sprint 6: Cell Formatting & Number Formats ✅
### Sprint 7: Data Tools ✅
### Sprint 8: MVP Polish, Testing & Documentation ✅

## Post-MVP (Sprints 9–16) — All Complete ✅

### Sprint 9: Pivot Tables ✅
- Pivot table engine with row/column grouping
- Aggregation: sum, count, average, min, max, product
- Filtering and grand totals
- Write pivot results back to sheet

### Sprint 10: Charts & Data Visualization ✅
- Chart types: bar, line, pie, scatter, area, column, doughnut
- Series extraction from sheet data
- SVG generation for frontend rendering

### Sprint 11: Advanced Formula Functions ✅
- ~100 additional functions: statistical, financial, engineering, date/time, lookup, array
- Total: 160+ built-in functions

### Sprint 12: Data Validation & Advanced Conditional Formatting ✅
- Data validation rules: whole number, decimal, list, text length, date, time, custom
- Conditional formatting: cell rules, top/bottom, data bars, color scales, icon sets, formula-based
- Input messages and error alerts

### Sprint 13: Internationalization & Accessibility ✅
- 22 supported locales with RTL support
- 88 translation keys
- Locale-aware number/currency/percentage/date/time formatting
- ARIA labels, screen reader support, keyboard navigation metadata

### Sprint 14: Print & PDF Export ✅
- Page layout: A4, A3, Letter, Legal, Tabloid
- Portrait/landscape, margins, scaling, headers/footers
- HTML print rendering and minimal PDF 1.4 generation
- Print preview data for frontend

### Sprint 15: Advanced Features ✅
- Sheet protection with password hashing and granular permissions
- Cell locking manager
- Goal seek (Newton's method + bisection fallback)
- Scenario manager for what-if analysis
- Cell comments/notes with author tracking

### Sprint 16: Post-MVP Polish & Release ✅
- Updated README, ARCHITECTURE.md, and ROADMAP.md with full feature documentation
- Updated CI workflow and verify-local.sh with frontend build check
- Sprint documentation for sprints 9–16
- Historical sprint-close quality gate passed; the current public release gate and test counts are tracked in `docs/QUALITY_GATE.md` and the README test suite table.
