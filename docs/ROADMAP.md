# Roadmap

The roadmap is ordered by user risk and community value. It does not assign dates or promise delivery.

## v0.3.0 baseline

The first public-release baseline includes local workbook editing, the native `.900sheets` format, supported XLSX import and export, CSV and JSON exchange, PDF output, formulas, formatting, data tools, chart previews, pivots, validation, conditional formatting, comments, and protection.

The exact compatibility boundary is documented in [COMPATIBILITY.md](COMPATIBILITY.md).

## Next priorities

### Data safety

- Add autosave and crash recovery with clear recovery prompts.
- Extend undo and redo to structural edits and format-only changes.
- Make all native feature state explicitly sheet-scoped in the frontend model.
- Add atomic-save recovery tests for interrupted writes.

### Spreadsheet compatibility

- Implement cross-sheet cell and range references.
- Add Excel and LibreOffice fixture workbooks to the test corpus.
- Expand XLSX support for validations, conditional formatting, tables, charts, and pivot metadata.
- Publish a versioned compatibility matrix backed by fixture results.

### Large workbooks

- Virtualize columns as well as rows.
- Expose the engine's full row and column range through navigation controls.
- Add workload benchmarks for sparse and dense sheets.
- Keep export budgets visible and configurable where that can be done safely.

### Desktop distribution

- Sign and notarize macOS builds.
- Add tested Windows packages.
- Add tested Linux packages for a documented set of distributions.
- Publish release assets through GitHub Releases after workflow verification.

### Community use

- Add more task-based examples and screenshots.
- Expand keyboard and screen-reader testing.
- Complete the locale settings interface and verify translated workflows.
- Build a public collection of small business, school, and community organization templates using invented data.

## Historical records

The sprint files in `docs/sprints/` describe how the initial codebase was assembled. They are historical notes, not the current release contract.
