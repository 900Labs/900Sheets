# Roadmap

The roadmap is ordered by user risk and community value. It does not assign dates or promise delivery.

## v0.4.0 baseline

The v0.4.0 baseline includes:

- Local workbook editing and native `.900sheets` persistence
- Cross-sheet formulas, quoted sheet references, workbook-wide dependencies, and cycle rejection
- Candidate-state transactions with bounded undo and redo across cells, formats, sheets, structure, CSV import, and supported feature state
- Autosaved recovery, final close recovery, multi-snapshot startup prompts, quarantine, and retryable cleanup
- Stable sheet identities across dependency, metadata, native save, undo, and redo boundaries
- Supported XLSX, CSV, and JSON exchange plus PDF output
- Deterministic compatibility tests and a generated LibreOffice open/resave check
- Linux and Windows desktop library gates for transaction and recovery behavior

The exact release boundary is documented in [COMPATIBILITY.md](COMPATIBILITY.md), with test evidence in [COMPATIBILITY_MATRIX.md](COMPATIBILITY_MATRIX.md).

## Next priorities

### Data safety

- Add user-facing recovery management after startup, including a way to inspect and remove retained snapshots later.
- Add a configurable autosave interval without weakening flush ordering.
- Add native backup rotation and document restore semantics separately from crash recovery.
- Expand power-interruption and filesystem fault testing where platform runners allow it.

### Spreadsheet compatibility

- Add a publishable workbook saved by Microsoft Excel with invented data and an automated or repeatable Excel verification procedure.
- Expand XLSX support for validation, conditional formatting, tables, native charts, and pivot metadata.
- Extend structural formula rewriting to more complex reference forms.
- Add more compatibility fixtures from multiple LibreOffice and Excel versions.

### Large workbooks

- Virtualize columns as well as rows.
- Expose more of the engine's row and column range through navigation controls.
- Add repeatable sparse and dense workload benchmarks.
- Make safe export limits visible before a long-running operation begins.

### Desktop distribution

- Sign and notarize macOS builds.
- Produce and manually verify Windows packages before publishing them.
- Define supported Linux distributions and produce packages for that set.
- Add GUI smoke coverage on packaged applications where runners are reliable.

### Community use

- Add task-based examples and screenshots.
- Expand keyboard and screen-reader testing.
- Complete the locale settings interface and verify translated workflows.
- Build public templates for schools, small businesses, and community organizations using invented data.

## Historical records

The sprint files in `docs/sprints/` describe earlier implementation phases. They are historical notes, not the current release contract.
