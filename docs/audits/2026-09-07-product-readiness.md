# Product readiness audit, 2026-09-07

## Decision and scope

900Sheets has useful spreadsheet capabilities, but the audited starting point had defects in recovery, clipboard safety, structural edits, formatting projection and formula execution. This change repairs those paths and makes the existing editor more understandable. It does not establish full Excel/Numbers parity or verified support for every older computer.

The control-tower workflow used separate backend and frontend builders, a focused frontend reliability builder, a research agent, and an independent reviewer. Review findings were routed back to the builders before acceptance. The initial checkout matched `origin/main` at `91fee5d3df7e809bb9d7f786a6fb754adff13bbc`; live GitHub API identity was `900Labs` (ID 265666979). Main CI run `30168098104` was successful. There were no open pull requests. A pre-existing local roadmap addition was retained in scope and reconciled with the observed merge of PR #8 on 2026-07-25.

## Findings and remediation

| Priority | Finding | Remediation and evidence |
| --- | --- | --- |
| P1 | Shared formula dependency paths were recursively revisited; long graphs could exhaust the stack | Iterative visited-set cycle detection; `cycle_check_handles_deep_chains_without_recursive_stack_growth`, `cycle_check_visits_shared_dependency_subgraphs_once` |
| P1 | Desktop formula evaluation repeatedly recalculated shared dependencies without a work/depth budget | Read-scoped memoization, deterministic projection order, and explicit calculation/payload-copy budgets; `repeated_formula_dependencies_are_evaluated_once_per_read`, `deep_formula_chains_and_work_budget_exhaustion_return_errors`, `formula_snapshot_limits_are_deterministic_across_insertion_orders`, `formula_value_copy_budget_bounds_cached_arrays_and_text` |
| P1 | Deep/large formulas could exhaust parser or evaluation resources | Input, token and depth limits; `formula_size_and_token_limits_are_enforced_before_parsing`, `deeply_nested_grammar_and_expression_trees_are_rejected` |
| P1 | Tiny REPT/factorial/date/precision formulas could bypass expression guards and allocate or loop excessively; integer extremes could panic | Function-specific input/work/output guards, checked arithmetic and a shared concatenation limit; seven function regressions plus `concatenation_rejects_oversized_results_before_output_allocation` |
| P2 | PERMUTA calculated a rising factorial rather than permutations with repetition | Correct `n^k` semantics and register `PERMUTATIONA`, retaining the old alias; `combinatorial_fast_paths_and_repetition_counts_are_correct` |
| P1 | Structural edits erased coordinate metadata from the whole active sheet | Reject edits when metadata cannot be preserved; `structural edits reject named range metadata before dispatch and preserve it` plus every-feature unit guard coverage |
| P1 | Uncommitted formula-bar drafts were written after coordinates or sheet identity changed | Commit/flush before structural commands and sheet deletion; `structural commands flush the formula bar draft before changing coordinates` |
| P1 | Cut cleared its source before successful paste; formula copies did not rebase | Retain cut source until atomic paste, reject unsupported dependency moves, translate supported copied A1 references; clipboard unit and browser regression suite. Review caught a stale-preflight race: guarded backend `move_cells` now rechecks source/dependencies/protection inside the candidate lock. |
| P2 | Cancelling startup recovery deleted the deferred snapshot | Retain it for the manager; `deferring a startup recovery preserves it while restoring another` |
| P2 | Native smoke testing exposed stale enabled Undo/Redo controls after reopening a workbook | Clear frontend availability only after successful replacement; `successful workbook replacements clear undo and redo controls`. Cancelled/failed opens preserve existing history. |
| P2 | Formatted blank cells vanished from the frontend on refresh | Include sparse format-only coordinates in projection; `sheet_snapshot_preserves_blank_formats_without_duplicate_cells` |
| P2 | Number formats did not immediately refresh displayed values; formulas were excluded from numerical statistics/conditional rules | Refresh projection and expose finite evaluated numeric values separately from display text; `snapshot_numeric_values_use_calculated_numbers_before_display_formatting` and browser coverage |
| P2 | Spreadsheet percentage syntax was parsed as a binary remainder | Correct postfix `%`; `percentages_work_as_postfix_values_in_arithmetic`; `MOD` remains remainder |
| P2 | Malformed CSV quoting was silently accepted, and the complete parsed matrix duplicated memory | Reject malformed quotes, handle UTF-8 BOM, move one parsed row at a time into the isolated candidate; `malformed_quotes_are_rejected_instead_of_silently_changing_data`, `malformed_csv_leaves_pending_workbook_unchanged` |
| P2 | A transitive development dependency had a current npm advisory | Update `nanoid` from 3.3.16 to a compatible fixed version in the lockfile; npm audit reports zero vulnerabilities |
| P2 | Unclear workbook state and crowded controls obscured existing functionality | Workbook title/save state, primary Open/Save, selection-aware formatting, direct size input, named alignment controls, scrollable tabs and zoom; 800×500, 1024×768 and 1280×720 browser layout checks |
| P2 | Small assets and offline-capable Windows installation were intentions without enforced configuration | Build budget guard in local gate/CI and explicit offline WebView2 installer; native/disconnected installer validation remains separate |

## Verification

The final complete `./scripts/verify-local.sh` pass succeeded on 2026-09-07:

| Check | Result |
| --- | --- |
| Rust format/clippy, warnings denied | Passed |
| Rust workspace tests, including LibreOffice external round trip | 560 passed |
| Svelte/TypeScript | 0 errors, 0 warnings |
| Frontend production build | Passed |
| Node unit tests | 31 passed |
| Playwright Chromium | 51 passed |
| Complete-output resource guard | Passed; JavaScript 57,919 gzip bytes, CSS 4,137 gzip bytes, total raw output 219,864 bytes |
| Oversized JavaScript outside `assets/` negative guard check | Rejected as required |
| `./scripts/verify-public-release.sh` and release-workflow invariants | Passed |
| Documentation file links and `git diff --check` | Passed |

Independent review approved the final changes with no remaining reported critical/P1 blockers in scope. The reviewer independently ran the final 72 desktop tests, the prior 128 formula plus 29 CSV tests, and the final added integer-boundary regression. Builder and complete-gate evidence remains distinct from that independent review. The first integration browser run exposed two ambiguous zoom selectors after adding footer controls; menu-scoped selectors fixed those tests, and the final full suite passed.

The browser suite uses mocked Tauri IPC and file dialogs. It verifies frontend ordering, rendering and interaction, not native file I/O, operating-system clipboard integration or Windows installation. Rust tests cover backend transactions, formulas, imports and persistence independently. A browser networking-offline test establishes the UI can keep working after its local assets load, not a clean offline installation.

The final macOS arm64 application bundle built successfully with `npm run tauri:build --prefix apps/desktop -- --bundles app`. A native UI smoke run created the invented School Budget template, evaluated `=200*10%` as `20`, saved through the native dialog with a reported rotating backup, and reopened the saved file. After the history-control fix and rebuild, a format-only bold edit on blank H2 survived save/reopen; Undo/Redo changed from available to disabled on successful reopen; F2 retained the percentage formula and displayed numeric statistics of 20. This is observed macOS development-host behavior, not Windows/Linux packaging, signing/notarization, disconnected installation or native screen-reader certification.

### Resource evidence

Host: Apple M3 Max, 128 GiB RAM, Darwin 25.6.0 arm64. This is a modern development machine, not the proposed older-computer validation target.

Command: `./scripts/benchmark-workbooks.sh` (release profile, default workload).

| Workload | Stored cells | Write time | Scan time |
| --- | --- | --- | --- |
| Sparse dispersed coordinates | 50,000 | 6 ms | <1 ms |
| Dense 500×500 values | 250,000 | 49 ms | <1 ms |

The benchmark prints integer milliseconds; a reported zero means below its millisecond reporting resolution. It exercises core sparse storage, not desktop transaction cloning, formula-heavy recalculation, IPC, rendering, recovery writes or operating-system memory. No low-end speed claim follows from these numbers.

`node scripts/check-resource-budgets.mjs` scans the complete frontend `dist` tree, including files copied from `public`. Budgets are 100 KiB gzip JavaScript, 20 KiB gzip CSS and 1 MiB total raw output. These are regression guards, not installed application or memory measurements.

### Dependency audit

`npm audit --prefix apps/desktop --audit-level=high` reports zero vulnerabilities after the compatible lockfile update. `cargo audit` exits successfully with 17 allowed warnings: inherited Linux GTK3/unic/proc-macro-error maintenance notices and the glib iterator unsoundness advisory. These remain dependency debt, not a clean bill of dependency health. Replacing Tauri's transitive GTK stack by an unsupported major override would need separate platform validation; no warning suppression or risky override was added.

## Product criteria and remaining gates

The [six-product research comparison](../COMPETITOR_RESEARCH.md) uses primary sources and distinguishes published requirements from measured performance. [Product acceptance criteria](../PRODUCT_CRITERIA.md) define the reference machine, workloads and provisional latency/memory targets.

Before a broad product-ready or old-computer-supported release claim:

- Measure the packaged app and all webview/helper processes on physical two-core, 4 GiB target systems, including formula-heavy editing and recovery.
- Complete a disconnected Windows install with WebView2 absent, then first launch/save/reboot/reopen; verify signing and published artifact provenance separately.
- Choose an explicit Linux distribution/CPU baseline and ship/test a package. Linux source CI alone is insufficient.
- Complete Microsoft Excel desktop and native screen-reader checks on declared versions.
- Add reference-aware cut across formula workbooks and sheets, metadata-aware structural edits, incremental sheet projection, and useful long-operation progress/cancellation.
- Add fixture-backed unsupported-XLSX-feature warnings, fuller locale coverage and observed community task testing.

No release tag, signing credentials, release publication or physical hardware certification is part of this change.
