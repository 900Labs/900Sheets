# Sprint 14: Print & PDF Export

## Goal
Implement print layout configuration, HTML print rendering, and PDF generation.

## What Was Built
- **Crate**: `sheets-print`
- **Print configuration**: `PrintConfig` with page size (A4, A3, Letter, Legal, Tabloid), orientation (portrait/landscape), margins, scaling (actual size, fit-to-page-width, fit-to-single-page, custom %)
- **Headers/footers**: left/center/right with `{page}`/`{pages}` template support
- **Page layout**: `calculate_pages()` computes page grid based on data, page size, margins, and scaling
- **Print area**: auto-detect or explicit specification
- **Repeat rows/columns**: repeating headers on each page
- **Gridlines and headings**: configurable printing of gridlines and row/column labels
- **HTML rendering**: `render_html()` generates complete HTML document with `@page` CSS, per-page tables, cell formatting
- **PDF generation**: `render_pdf()` produces minimal valid PDF 1.4 with `PdfBuilder` — catalog, pages, content streams, Helvetica font, gridlines, cell text
- **Print preview**: `generate_preview()` returns structured `PrintPreview` data for frontend
- Tauri IPC: `get_print_preview`, `render_print_html`, `export_pdf`, `get_page_count`, `save_pdf_to_file`

## Tests
25 unit tests covering page layout, HTML rendering, PDF generation, and preview.

## Status: Complete ✅
