# Sprint 6: Cell Formatting & Number Formats

## Goal
Add cell formatting (bold, italic, font, colors, alignment, borders) and number formats (currency, percentage, dates, custom patterns) to the core data model.

## Scope Delivered

### Cell Formatting (sheets-core/src/format.rs)
- [x] CellFormat struct with: bold, italic, underline, strikethrough, font_size, font_name, font_color, bg_color, h_align, v_align, wrap_text, number_format, borders (top/bottom/left/right)
- [x] HorizontalAlignment enum (Left, Center, Right, General)
- [x] VerticalAlignment enum (Top, Middle, Bottom)
- [x] Border struct with BorderStyle enum (None, Thin, Medium, Thick, Dotted, Dashed, Double)
- [x] Builder pattern for CellFormat (fluent API)
- [x] Format merge (override takes precedence)
- [x] Serde serialization/deserialization with skip_serializing_if for Option fields
- [x] Format storage in Sheet (HashMap keyed by (row, col))
- [x] set_format, get_format, clear_format, iter_format methods on Sheet
- [x] clear_cell also clears format
- [x] 11 format tests

### Number Formats (sheets-core/src/number_format.rs)
- [x] NumberFormat enum: General, Number, Currency, Percentage, Scientific, Fraction, Date, Time, DateTime, Duration, Text, Custom
- [x] Pattern parsing (from_pattern): "$#,##0.00" → Currency, "0.00%" → Percentage, etc.
- [x] Formatting functions: general, number with thousands sep, currency, percentage, scientific, fraction, date, time, datetime, duration
- [x] Thousands separator support
- [x] Excel serial date conversion (chrono-based)
- [x] 12 number format tests

## Validation Results

- cargo test --workspace: 159 tests pass (43 core + 74 formula + 10 xlsx + 18 csv + 14 json)
- cargo clippy --workspace -- -D warnings: clean
- cargo fmt --all -- --check: clean
- npm run check: 0 errors, 0 warnings

## Decisions

- CellFormat uses Option fields for sparse storage (only set fields are stored)
- Format merge: other overrides self (for style inheritance)
- Empty formats not stored in Sheet HashMap
- NumberFormat enum with pattern parsing for Excel-compatible format strings
- chrono used for date conversion (Excel serial date → year/month/day)

## Issues Found

None.

## Carry-Over

- Wire format IPC commands into Tauri backend (Sprint 8)
- Frontend format UI controls deferred to Sprint 8
- XLSX format preservation on import/export deferred
- Conditional formatting deferred to post-MVP
