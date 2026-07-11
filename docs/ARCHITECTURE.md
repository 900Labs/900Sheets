# Architecture Overview

## System Design

900Sheets follows the 900 Labs pattern of Rust-owned data models with a Svelte 5 projection UI.

### Data Flow

```
User Input (Svelte UI)
    ↓ Tauri IPC
Rust Command Handler (src-tauri/src/lib.rs)
    ↓
sheets-core (Workbook → Sheet → Cell)
    ↓
sheets-formula (Evaluation Engine) [Sprint 3]
    ↓
sheets-xlsx/csv/json (File I/O) [Sprints 4-5]
```

### Core Crates

#### sheets-core
The core data model. Owns:
- `Workbook` - collection of sheets with active sheet tracking
- `Sheet` - grid of cells with sparse storage via `HashMap<(u32, u32), CellValue>`
- `CellValue` - tagged union: Number, Text, Boolean, Error, Empty, Formula
- `CellAddress` - A1 notation parsing and serialization with absolute/relative support
- `CoreError` - typed errors for out-of-bounds and parse failures
- `CellFormat` - bold, italic, underline, font size/name/color, background color, alignment, borders, number format
- `NumberFormat` - general, number, currency, percentage, date, time, scientific

Sheet dimensions default to 1,000,000 rows × 16,384 columns and cells are stored sparsely - only non-empty cells consume memory.

#### sheets-formula
Custom formula engine:
- Tokenizer → AST → compiled expression tree
- Evaluator with type coercion and error propagation
- Dependency graph for incremental recalculation
- Cycle detection for circular references
- Extensible function registry (160+ functions across math, logical, text, date/time, lookup, financial, engineering, info)
- `CellProvider` trait for pluggable cell access
- `SimpleProvider` for testing and standalone evaluation

#### sheets-xlsx
OOXML import/export boundary:
- Reads XLSX zip packages (workbook.xml, worksheets, shared strings, styles)
- Writes XLSX with style tables and number formats
- Sanitizes all input before reaching frontend
- Bounded file size, decompressed XML entry size, supported sheet dimensions, and cell count for safety

#### sheets-csv
CSV import/export with auto-detection of delimiters and encoding.

#### sheets-json
JSON import/export supporting arrays of objects and arrays of arrays, with byte-size, depth, supported sheet dimension, and cell-count limits.

#### sheets-chart
Chart rendering and data visualization:
- Chart types: bar, line, pie, scatter, area, column, doughnut
- Series extraction from sheet data ranges
- SVG generation for frontend rendering
- Configurable titles, axes, legends, colors

#### sheets-pivot
Pivot table engine:
- Row and column grouping by field
- Aggregation functions: sum, count, average, min, max, product
- Filtering by field values
- Grand totals for rows and columns
- Writing pivot results back to sheet

#### sheets-validation
Data validation and conditional formatting:
- Data validation rules: whole number, decimal, list, text length, date, time, custom formula
- Input messages and error alerts (stop, warning, information)
- Conditional formatting: cell value rules, top/bottom, data bars, color scales, icon sets, formula-based rules
- Rule evaluation and matching

#### sheets-i18n
Internationalization and accessibility:
- 22 supported locales (en, es, fr, de, it, pt, ru, zh, ja, ko, ar, hi, tr, nl, sv, pl, fa, he, th, vi, id, uk)
- 88 translation keys covering UI and accessibility
- Locale-aware number, currency, percentage, date, and time formatting
- RTL support for Arabic, Hebrew, Persian, Urdu
- Accessibility labels: ARIA labels, screen reader support, keyboard navigation metadata
- Locale formatting and accessibility helpers are backend-ready; the desktop locale settings UI is not fully wired.

#### sheets-print
Print layout and PDF export:
- Page sizes: A4, A3, Letter, Legal, Tabloid
- Portrait and landscape orientation
- Configurable margins, headers, and footers with `{page}`/`{pages}` templates
- Scaling: actual size, fit-to-page-width, fit-to-single-page, custom percentage
- Print area, repeating rows/columns, gridlines, headings
- HTML print rendering for browser/webview printing
- Minimal valid PDF 1.4 generation with text, gridlines, and formatting
- Print preview data for frontend rendering

#### sheets-advanced
Advanced spreadsheet features:
- Sheet protection with password hashing and granular permissions (select, format, insert, delete, sort, filter, pivot)
- Cell locking manager (per-cell and range-based)
- Goal seek: Newton's method with numerical derivatives, bisection fallback with range expansion
- Scenario manager: named what-if scenarios with cell values, apply/restore, summary reports
- Cell comments/notes with author tracking, visibility toggling, and bulk management
- Protection, goal seek, and comments are wired into the desktop UI. Scenario support is available through the backend/IPC, with the full manager UI still pending.

### Frontend

The Svelte 5 frontend in `apps/desktop/src/` provides:
- Grid component with CSS grid layout
- Cell selection, keyboard navigation, inline editing
- Formula bar
- Sheet tab management
- Native-dialog open/save/import/export flows backed by Tauri IPC

### Tauri IPC

Commands exposed to the frontend:
- **Workbook**: `new_workbook`, `get_sheets`, `add_sheet`, `delete_sheet`, `rename_sheet`
- **Cells**: `get_cell`, `set_cell`, `clear_cell`, `get_sheet_data`, `evaluate_formula`
- **File I/O**: `import_xlsx`, `export_xlsx`, `import_xlsx_file`, `export_xlsx_file`, `import_csv_data`, `export_csv`, `import_csv_file`, `export_csv_file`, `import_json_data`, `export_json`, `import_json_file`, `export_json_file`
- **Data tools**: `sort_data`, `find_in_sheet_cmd`, `replace_in_sheet_cmd`
- **Charts**: `create_chart`
- **Pivot tables**: `create_pivot`, `create_pivot_sheet`, `get_pivot_columns`
- **Validation**: `validate_cell_value`, `validate_range_cmd`, `check_input_value`
- **Conditional formatting**: `evaluate_conditional_formats`, `find_conditional_format_matches`
- **i18n**: `get_available_locales`, `get_translations`, `translate_key`, `format_number_i18n`, `format_currency_i18n`, `format_percentage_i18n`, `format_date_i18n`, `format_time_i18n`
- **Accessibility**: `get_cell_accessibility_label`, `get_selected_cell_label`, `get_editing_cell_label`, `get_navigation_direction_name`
- **Print & PDF**: `get_print_preview`, `render_print_html`, `export_pdf`, `get_page_count`, `save_pdf_to_file`
- **Advanced**: `protect_sheet`, `unprotect_sheet`, `set_cell_locked`, `lock_cell_range`, `is_cell_locked`, `goal_seek_cmd`, `apply_scenario`, `get_cell_comment`, `add_cell_comment`, `remove_cell_comment`, `list_comments`

Protected sheets are enforced at the Tauri command boundary for mutating cell edits, clears, formatting, sorting, replace, CSV overwrite, scenario application, protected-sheet deletion, and pivot use. Cell-level locking uses `CellLockManager` - when a sheet is protected, only cells explicitly unlocked via `set_cell_locked` or `lock_cell_range` are editable; all others are locked by default.

### Design Principles

1. **Rust owns the truth** - all data models live in Rust, the frontend is a projection
2. **Sanitize at the boundary** - all imported content is validated in Rust
3. **Sparse storage** - only non-empty cells consume memory
4. **Bounded operations** - resource limits prevent pathological files
5. **No telemetry** - no data leaves the machine unless explicitly exported
6. **Offline-first** - all features work without network connectivity
7. **Low-resource optimized** - designed for modest hardware in developing economies
