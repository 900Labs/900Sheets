# v0.4.0 compatibility matrix

This matrix ties public compatibility claims to deterministic tests. Results below are from the v0.4.0 release-prep run on 2026-07-11. Test IDs are Rust test names or Playwright titles and can be selected directly with the commands shown.

## File and application compatibility

| Area | Result | Test or fixture ID | What it proves |
| --- | --- | --- | --- |
| XLSX basic import and export | Pass | `export::tests::test_export_import_roundtrip`, `import::tests::test_import_minimal_xlsx` | Supported cells and workbook packages can be exported and reopened. |
| XLSX formulas with cached values | Pass | `import::tests::test_formula_with_cached_value_preserves_formula` | Formula text is retained when Excel-style cached values are present. |
| XLSX shared formulas | Pass | `import::tests::test_shared_formula_followers_are_expanded` | Shared-formula followers are expanded from their master record. |
| XLSX sheet ordering | Pass | `import::tests::test_sheet_relationship_ids_control_worksheet_mapping` | Workbook relationship IDs, not archive order, select worksheet parts. |
| XLSX styles and formatted blanks | Pass | `export::tests::test_export_import_preserves_complete_styles_and_formatted_blank_cells` | Supported fonts, fills, alignment, wrapping, borders, number formats, and styled empty cells survive an internal round trip. |
| XLSX cross-sheet formula text | Pass | `export::tests::test_cross_sheet_formula_roundtrip_preserves_quoted_absolute_reference` | Quoted sheet names and absolute references survive export and import. |
| LibreOffice external round trip | Pass in CI; conditional locally | `libreoffice_can_open_and_resave_exported_workbook` | A deterministic generated workbook with data, a formula, and formatting is opened and re-saved by headless LibreOffice, then imported again. CI fails if LibreOffice is unavailable. |
| Microsoft Excel desktop round trip | Not automated | No v0.4.0 external Excel fixture runner | OOXML structures are tested deterministically, but Excel itself is not run in CI. |
| Native v0.3 compatibility | Pass | `native::tests::native_import_remaps_legacy_zero_ids_without_colliding_with_explicit_ids` | Legacy native sheets without current stable IDs receive safe, noncolliding identities. |
| Native cross-sheet persistence | Pass | `native::tests::native_roundtrip_preserves_cross_sheet_formula_text_and_stable_ids` | Cross-sheet formula text and stable sheet identity survive native save and reopen. |
| Native feature persistence | Pass | `native::tests::native_roundtrip_preserves_feature_metadata`, `tests::native_metadata_scopes_backend_features_by_stable_sheet_id` | Supported metadata remains scoped to stable sheet identities. |

Run the file-format evidence with:

```bash
cargo test -p sheets-xlsx
cargo test -p sheets-json
```

## Formula and dependency compatibility

| Area | Result | Test ID | What it proves |
| --- | --- | --- | --- |
| Quoted and bounded references | Pass | `ast::tests::test_parse_quoted_and_bounded_refs` | Escaped quotes, `$` markers, last valid row and column, and invalid cross-sheet range endpoints are handled. |
| Cross-sheet tokenization | Pass | `tokenizer::tests::test_tokenize_cross_sheet_references` | Simple, quoted, escaped, absolute, and invalid references tokenize deterministically. |
| Cross-sheet evaluation | Pass | `evaluator::tests::test_eval_qualified_cell_and_range_refs` | Qualified ranges provide values to functions. |
| Workbook dependent refresh | Pass | `tests::workbook_provider_evaluates_cross_sheet_chain_and_reflects_source_edits` | Workbook-wide graph validation resolves a dependent chain across worksheets, and a source edit is reflected when those formulas are refreshed. |
| Cross-sheet cycle safety | Pass | `dependency::tests::cross_sheet_cycle_is_rejected_and_rolled_back`, `tests::dependency_rebuild_rejects_cross_sheet_cycle` | Cycles are rejected without leaving an invalid graph or workbook. |
| Formula reference budget | Pass | `ast::tests::huge_reference_expansion_returns_explicit_budget_error`, `dependency::tests::huge_range_dependency_is_rejected_without_materialization`, `evaluator::tests::test_huge_range_is_rejected_before_materialization` | Ranges above 100,000 expanded references fail before materialization in reference collection, graph construction, and evaluation. |

Run the formula evidence with:

```bash
cargo test -p sheets-formula
cargo test -p sheets-desktop --lib
```

## Transactions, undo, and recovery

| Area | Result | Test ID | What it proves |
| --- | --- | --- | --- |
| Atomic workbook transaction | Pass | `tests::mutation_without_transaction_is_rejected_and_live_state_is_unchanged`, `tests::failed_multi_delta_restore_does_not_partially_mutate_or_move_history` | Mutations require a transaction and failed restore does not partially change state or history. |
| Cross-feature undo and redo | Pass | `tests::one_transaction_undoes_and_redoes_cell_format_and_metadata_together`, `tests::feature_only_transaction_restores_comments_on_undo_and_redo` | Data, formatting, metadata, and comments restore together. |
| History budgets | Pass | `tests::budget_overflow_keeps_live_state_and_history_unchanged`, `tests::aggregate_history_eviction_obeys_count_and_byte_budgets`, `tests::serialized_history_size_counts_large_cell_format_name_and_sheet_payloads` | Per-transaction rejection is atomic and aggregate eviction honors exact serialized cost. |
| View and print metadata | Pass | `tests::view_and_print_metadata_roundtrip_through_undo_and_redo`, `view and print settings are dirty, undoable, and included in save and recovery metadata` | Gridlines, page size, and orientation mark the workbook dirty, move through undo and redo, and enter native save and recovery metadata. |
| Recovery write and replacement | Pass | `tests::recovery_store_discovers_restores_discards_and_never_touches_source`, `tests::failed_recovery_write_retains_last_good_snapshot`, `tests::overlapping_recovery_writes_are_complete_and_leave_no_temp_files` | Recovery files stay separate, failed writes preserve the prior snapshot, and overlapping writes serialize cleanly. |
| Recovery hardening | Pass | `tests::recovery_store_rejects_symlink_root_and_target`, `tests::corrupt_recovery_is_quarantined_from_discovery`, `tests::failed_cleanup_retires_snapshot_from_discovery_and_is_retryable` | Symlink targets are rejected, corrupt snapshots are quarantined, and failed cleanup cannot reappear as a current recovery. |
| Multiple startup recoveries | Pass | `restoring one recovery preserves every unselected snapshot`, `explicit recovery discard removes only the selected snapshot` | Restore preserves unselected snapshots and cancel discards only the selected one. |
| Cleanup retry identity | Pass | `replacement cleanup failure retains identity and Save retries cleanup`, `save cleanup failure is visible and retryable under the same recovery identity` | Cleanup failures remain visible and retry under the same recovery identity. |
| Windows and Linux backend gate | Configured | `.github/workflows/ci.yml`, job `recovery-platform-checks` | The full 40-test desktop library suite compiles and runs on `ubuntu-latest` and `windows-latest`. |

Run the local recovery and UI evidence with:

```bash
cargo test -p sheets-desktop --lib
npm run test:unit --prefix apps/desktop
npm run test:e2e --prefix apps/desktop
```

## Release-prep totals

| Suite | Result |
| --- | --- |
| Rust workspace | 462 passed |
| Frontend unit | 9 passed |
| Playwright Chromium | 16 passed |
| Svelte and TypeScript diagnostics | 0 errors, 0 warnings |

Counts are a release-prep snapshot, not a substitute for running the current gate. A later patch may add tests without changing compatibility.
