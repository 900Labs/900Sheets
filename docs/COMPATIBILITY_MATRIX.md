# v0.5.0 compatibility matrix

This matrix ties public compatibility claims to deterministic tests. Existing v0.4.0 rows retain their release evidence. Rows marked **Pass locally** were observed on 2026-07-11 in the complete integrated v0.5.0 local gate. Hosted workflow results remain release-time evidence and must be recorded for the tag before publication. Test IDs are Rust test names, Node test titles, or Playwright titles and can be selected directly with the commands shown.

## File and application compatibility

| Area | Result | Test or fixture ID | What it proves |
| --- | --- | --- | --- |
| XLSX basic import and export | Pass | `export::tests::test_export_import_roundtrip`, `import::tests::test_import_minimal_xlsx` | Supported cells and workbook packages can be exported and reopened. |
| XLSX formulas with cached values | Pass | `import::tests::test_formula_with_cached_value_preserves_formula` | Formula text is retained when Excel-style cached values are present. |
| XLSX shared formulas | Pass | `import::tests::test_shared_formula_followers_are_expanded` | Shared-formula followers are expanded from their master record. |
| XLSX sheet ordering | Pass | `import::tests::test_sheet_relationship_ids_control_worksheet_mapping` | Workbook relationship IDs, not archive order, select worksheet parts. |
| XLSX styles and formatted blanks | Pass | `export::tests::test_export_import_preserves_complete_styles_and_formatted_blank_cells` | Supported fonts, fills, alignment, wrapping, borders, number formats, and styled empty cells survive an internal round trip. |
| XLSX cross-sheet formula text | Pass | `export::tests::test_cross_sheet_formula_roundtrip_preserves_quoted_absolute_reference` | Quoted sheet names and absolute references survive export and import. |
| XLSX validation subset | Pass locally | `export::tests::validation_fields_and_xml_escaping_roundtrip_exactly`, `import::tests::multiple_sqref_ranges_become_separate_sheet_scoped_rules` | Supported validation fields, XML escaping, and multiple sheet-scoped ranges are represented without silent field loss. |
| XLSX conditional-formatting subset | Pass locally | `export::tests::user_visible_conditional_format_types_roundtrip_without_silent_loss`, `import::tests::differential_format_boolean_false_values_remain_false` | Supported rule types and differential formatting survive an internal document round trip. |
| XLSX feature rejection | Pass locally | `export::tests::export_rejects_feature_mapping_mismatches_invalid_ranges_and_unsupported_rules`, `import::tests::feature_range_lists_are_bounded_before_unbounded_collection` | Invalid mappings and unsupported rules fail explicitly, and feature ranges are bounded before collection. |
| Complex structural reference persistence | Pass locally | `export::tests::structural_edits_preserve_complex_formula_rewrites_through_xlsx` | Supported rewritten absolute, mixed, quoted cross-sheet, and string-literal formula forms survive XLSX export and import. |
| Desktop XLSX feature identity | Pass locally | `tests::xlsx_features_follow_imported_sheet_stable_ids` | Imported worksheet features remain attached to the relationship-selected sheet's stable identity through desktop metadata mapping. |
| LibreOffice external round trip | Pass locally; required in CI | `libreoffice_can_open_and_resave_exported_workbook` | A deterministic generated workbook with data, formulas, formatting, validation, and conditional formatting is opened and re-saved by headless LibreOffice, then imported again. CI fails if LibreOffice is unavailable. |
| Microsoft Excel desktop round trip | Not automated | No v0.5.0 external Excel fixture runner | OOXML structures are tested deterministically, but Excel itself is not run in CI. |
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
| Configurable recovery timing | Pass locally | `changing the delay reschedules a pending write without duplicating it`, `changing the delay preserves serialization with an in-flight write`, `rejects invalid configurable delays` | A timing change invalidates pending work safely, preserves serialized writes, and rejects invalid settings. |
| Edit during native save | Pass locally | `an edit committed during a slow save remains dirty and recoverable` | A primary save cannot clear the dirty state for a newer edit committed while that save is in flight. |
| Replacement during native save | Pass locally | `a slow save cannot take ownership of a replacement workbook session` | A completed save from an older workbook cannot claim the replacement session path, dirty state, or recovery cleanup. |
| Rotating native backups | Pass locally | `tests::native_backup_store_rotates_per_document_and_keeps_private_files`, `tests::failed_backup_write_and_rotation_never_remove_last_good_backup`, `tests::native_backup_store_rejects_symlink_root_and_target` | Backups are private and document-scoped, retain five versions, preserve the last good version on failure, and reject unsafe filesystem targets. |
| Atomic native save | Pass locally | `tests::atomic_native_write_replaces_existing_file_and_cleans_unique_temporary_files`, `tests::interrupted_native_write_retains_last_good_file`, `tests::atomic_native_write_rejects_symlink_target` | Primary native saves replace atomically, preserve the prior file after an interruption, and reject unsafe targets. |
| Candidate restore validation | Pass locally | `tests::invalid_native_restore_leaves_live_workbook_unchanged` | A malformed backup or native restore candidate cannot partially replace live workbook state. |
| Windows and Linux backend gate | Configured | `.github/workflows/ci.yml`, job `recovery-platform-checks` | The complete desktop library suite is configured to compile and run on `ubuntu-latest` and `windows-latest`. |

Run the local recovery and UI evidence with:

```bash
cargo test -p sheets-desktop --lib
npm run test:unit --prefix apps/desktop
npm run test:e2e --prefix apps/desktop
```

## Grid, export, onboarding, and accessibility

| Area | Result | Test ID | What it proves |
| --- | --- | --- | --- |
| Full-bound Go To | Pass locally | `Go To reaches the maximum grid address with a bounded DOM`, `Go To accepts ranges and rejects addresses outside the workbook bounds` | Navigation reaches `XFD1000000`, accepts bounded ranges, rejects invalid addresses, and keeps the rendered DOM bounded. |
| Virtualized keyboard focus | Pass locally | `grid uses one keyboard focus target across virtualized navigation and zoom` | Keyboard navigation and zoom retain one grid focus target across window changes. |
| Localized grid labels | Pass locally | `locale setting persists verified grid accessibility labels` | English, Swedish, and Spanish settings persist and update the verified grid labels. |
| Dense operation budget | Pass locally | `dense operations fail quickly for selections above the shared safety budget` | Dense frontend operations reject a full-grid selection before enumerating its coordinates. |
| Export preflight | Pass locally | `export_preflight::tests::sparse_high_coordinate_csv_is_blocked_before_materialization`, `export_preflight::tests::xlsx_reports_sparse_extent_without_applying_a_dense_limit`, `export_preflight::tests::json_compares_each_sheet_to_its_per_sheet_backend_limit`, `export_preflight::tests::explicit_pdf_print_area_ignores_unrelated_far_cell` | Dense exporters apply the correct bounds before materialization, while XLSX reports sparse coordinates and PDF respects an explicit print area. |
| Starter-template catalog | Pass locally | `catalog includes the four community starter templates`, `catalog uses only invented, publishable examples`, `template cells are rebased when inserted away from A1` | The four public templates contain invented data and formulas rebase at the insertion point. |
| Local first-run state | Pass locally | `first-run completion is local, explicit, and repeatable`, `first-run helpers stay usable when storage is unavailable`, `first run offers local templates and records completion without an account` | First-run completion stays local, the guide remains usable when preference storage is unavailable, and the UI inserts a template without an account. |

## Distribution workflow

| Area | Result | Test or workflow ID | What it proves |
| --- | --- | --- | --- |
| Release workflow structure | Static validation passed | `scripts/verify-release-workflow.sh`, `.github/workflows/release.yml` | The configured jobs enforce version-matched annotated tags, source gates, artifact checksums, provenance, and publication prerequisites. |
| macOS signing and notarization | Hosted release gate pending | `macos-app`, `publish-release` | Publication is configured to require Developer ID signing, accepted notarization, stapling, and verification. The source tree alone does not prove those hosted steps ran. |
| Windows package smoke | Hosted release gate pending | `windows-installer`, step `Smoke install and uninstall package` | The hosted job is configured to install the NSIS package silently, check its version, and uninstall it before upload. The source tree alone does not prove the hosted run succeeded. |
| Microsoft Excel desktop | Not automated | [XLSX compatibility check](XLSX_COMPATIBILITY_CHECK.md) | Manual observations use an invented fixture and remain scoped to the recorded Excel version and platform. |

## Release-prep totals

| Suite | Result |
| --- | --- |
| Rust workspace | 501 passed locally |
| Frontend unit | 21 passed locally |
| Playwright Chromium | 32 tests defined; final release run pending |
| Svelte and TypeScript diagnostics | 0 errors, 0 warnings locally |

Counts are a local release-prep snapshot, not a substitute for running the complete gate or hosted platform jobs. A later patch may add tests without changing compatibility.
