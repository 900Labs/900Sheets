# Changelog

## v0.5.0

### Added

- Added row and column virtualization across the full engine bounds of 1,000,000 rows and 16,384 columns.
- Added bounded **Go To** navigation for a cell or range from `A1` through `XFD1000000`.
- Added export preflight for XLSX, CSV, JSON, and PDF, including scale information, large dense-export warnings, and existing safety-limit enforcement.
- Added a **Recovery and Backups** panel that can list, inspect, restore, and delete retained autosave snapshots after startup.
- Added configurable recovery timing while preserving serialized flush and write ordering.
- Added private rotating native backups. After a successful `.900sheets` save, the app attempts to keep up to five backups for that document and reports maintenance failures separately. Inspection, restore, and deletion controls remain separate from recovery.
- Added school budget, small-business accounts, community project, and household planning starter templates using invented data.
- Added a local first-run guide and persisted English, Swedish, and Spanish labels for grid navigation and core accessibility text.
- Added XLSX import and export for the supported validation and conditional-formatting subset, including differential formatting.
- Added sparse and dense workbook benchmark generation and contributor-sized community planning.
- Added a Windows NSIS artifact job and credential-gated signing paths for macOS Developer ID and Windows Authenticode.
- Added workflow checks that verify artifact hashes and signing provenance, refuse to publish an ad hoc macOS build, and require accepted notarization plus stapling before GitHub Release publication.

### Changed

- Native saves use atomic replacement and report backup-maintenance warnings separately from primary save success.
- The desktop grid uses one keyboard focus target with screen-reader row, column, selection, and active-cell semantics.
- Structural reference rewriting covers the supported complex A1 forms exercised by the XLSX regression suite.
- Compatibility documentation separates workflow capability from verified facts about any particular tagged artifact.
- Package, workspace, desktop bundle, and documentation versions are aligned at 0.5.0.

### Known limitations

- Developer ID signing, notarization, Authenticode signing, hosted artifact publication, and clean-machine installation are release-time results, not properties guaranteed by the source tree.
- Microsoft Excel desktop is not an automated test dependency. Deterministic OOXML tests and the LibreOffice round trip cover the documented XLSX subset.
- Native Excel tables, slicers, charts, pivot caches, macros, external links, images, shapes, and embedded objects remain unsupported.
- The interface translations cover grid navigation and core accessibility labels, not the complete application.
- Frozen panes are limited to the first 20 rows and first 20 columns even though the virtualized grid reaches row 1,000,000 and column `XFD`.
- Recovery and rotating native backups are local safeguards, not cloud sync or a complete external backup strategy.
- Linux packaging is not configured for v0.5.0.

## v0.4.0

### Added

- Added cross-sheet cell and range formulas with quoted sheet names, absolute references, workbook-wide dependency tracking, and cross-sheet cycle rejection.
- Added candidate-state workbook transactions and bounded undo and redo for cells, formats, sheets, structure, CSV import, sort, replace, pivot output, comments, locks, protection, and supported feature metadata.
- Added debounced autosaved recovery, final close recovery, newest-first startup prompts, corrupt-snapshot quarantine, and retryable cleanup.
- Added stable sheet identities across native persistence, formula dependencies, feature metadata, undo, and redo.
- Added Linux and Windows CI coverage for the complete desktop recovery and transaction library suite.
- Added a versioned compatibility matrix tied to exact deterministic test IDs.

### Changed

- **Open XLSX** and **Open JSON** are labeled and documented as replacement workflows. **Import CSV** remains an undoable active-sheet import.
- Native `.900sheets` files retain format version 1 and remain readable from v0.3.0.
- Formula range expansion is explicitly limited to 100,000 references.
- Undo history is limited to 100 transactions, 64 MiB total, 32 MiB per transaction, and 200,000 coordinates per transaction.
- Package, workspace, desktop bundle, and documentation versions are aligned at 0.4.0.
- The release workflow ad hoc signs and strictly verifies the completed macOS app before and after archiving.

### Fixed

- Prevented failed transactions and failed undo or redo restores from partially mutating live workbook state or moving history.
- Preserved cross-sheet dependencies when a formula source changes and rebuilt the workbook graph after imports and structural changes.
- Prevented stale feature metadata from attaching to a replacement sheet after delete, undo, redo, save, and reopen.
- Serialized overlapping recovery writes, retained the last good snapshot after a failed write, and rejected unsafe symlink targets.
- Preserved unselected startup recoveries and made explicit discard affect only the selected snapshot.
- Retired a recovery from discovery before cleanup so a deletion failure cannot present stale data as the current snapshot.

### Known limitations

- The visible editor grid is limited to 52 columns and 1,000 rows.
- Microsoft Excel desktop is not run in CI. Deterministic OOXML tests and a generated LibreOffice round trip cover the documented XLSX subset.
- Recovery is not a versioned backup service and filesystem durability still depends on platform and storage behavior.
- The macOS artifact is ad hoc signed and not notarized. Windows and Linux packages are not produced.

## v0.3.0

### Added

- Added the sparse `.900sheets` workbook format for continued editing.
- Added dirty-state indication and unsaved-change confirmation for destructive workbook actions and app close events.
- Added atomic batch commands for cell changes, formatting, and complete sheet snapshots.
- Added persisted native metadata for comments, protection, cell locks, validation rules, conditional rules, named ranges, frozen panes, filters, and chart previews.
- Added release compatibility, user, support, and maintainer documentation.

### Fixed

- Preserved XLSX formulas when cells include cached `<v>` values.
- Resolved workbook sheets through their relationship IDs instead of relationship file order.
- Corrected XLSX font, fill, alignment, wrapping, border, number-format, and formatted-blank-cell round trips.
- Refreshed formula displays after edits, clears, paste, undo, redo, sort, replace, import, and structural changes.
- Made row and column edits preserve formatting and rewrite affected A1 formula references.
- Made sort operate on the selected range and move cell formats with their values.
- Scoped comments by sheet.
- Guarded dense CSV, JSON, and print exports against sparse high-coordinate expansion.
- Batched range formatting to avoid one IPC request per cell.

### Changed

- Cross-sheet references now fail with a clear unsupported-feature message.
- `Ctrl+S` saves the editable `.900sheets` format. XLSX is an explicit export.
- CI and the local gate use `npm ci` for lockfile-reproducible installs.
- Package and desktop bundle versions are now 0.3.0.

### Known limitations

- Cross-sheet formulas are not supported.
- The visible editor grid is limited to 52 columns and 1,000 rows.
- Structural edits and format-only operations are not fully represented in undo history.
- Autosave and crash recovery are not included.
- The macOS artifact is ad hoc signed and not notarized. Windows and Linux packages are not produced by the release workflow.

## v0.2.0

- Hardened formula dependency updates and circular-reference rejection.
- Included populated-cell formats in sheet snapshots.
- Cleared derived state on complete workbook resets and imports.
- Added CSV import row, column, cell, and byte budgets.
- Updated the desktop bundle identifier for the 900Labs release line.
