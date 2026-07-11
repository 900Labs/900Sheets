# Architecture

900Sheets uses a Rust-owned workbook model behind a Tauri v2 command boundary. The Svelte 5 frontend is a projection of that state and coordinates user interactions, transactions, and recovery scheduling.

## Runtime data flow

```text
Svelte UI
  | queued edits and transaction metadata
  v
Tauri command boundary
  | candidate workbook mutation
  v
Rust AppState
  |-- Workbook and stable sheet identities
  |-- Workbook-wide dependency graph
  |-- Protection and cell-lock state
  |-- Sheet-scoped comments and feature metadata
  `-- Bounded undo and redo history
```

File operations pass through bounded Rust importers and exporters. The frontend never treats its visible grid as the authoritative workbook.

## Workbook and formula graph

`sheets-core` stores sheets sparsely. Each sheet has a stable identity that is independent of its position and display name. Stable identities keep formula dependencies, comments, protection, locks, and frontend feature metadata attached to the correct sheet through rename, delete, undo, redo, and native reopen operations.

`sheets-formula` tokenizes and parses A1 references with optional sheet qualifiers. The workbook provider resolves a sheet name to a stable identity and evaluates local or cross-sheet values. The dependency graph uses `(stable_sheet_id, row, column)` keys, so a source change can find dependents across the workbook.

Graph construction, formula replacement, transaction commit, and workbook replacement reject circular dependencies. Reference expansion is bounded to 100,000 cells per formula in reference collection, dependency construction, and evaluation.

## Candidate transactions

Every user mutation begins a workbook transaction. The backend clones the current workbook, dependency graph, protection state, cell locks, and comments into a pending candidate. Mutating commands operate only on that candidate.

Commit follows this order:

1. Rebuild the candidate workbook dependency graph.
2. Derive compact workbook deltas between live and candidate state.
3. Serialize and validate backend sheet state.
4. Check the 200,000-coordinate and 32 MiB per-transaction budgets.
5. Swap the complete candidate state into the live application.
6. Append one undo record, clear redo, and evict old history to the 100-entry and 64 MiB aggregate limits.

If any validation or budget check fails, live workbook state and history remain unchanged. Abort drops the candidate. Commands that attempt a mutation without an active transaction are rejected.

Undo and redo clone live state, apply the complete transaction to the clone, rebuild and validate the dependency and backend state, then swap on success. History moves only after the state is valid. This prevents a multi-delta restore from applying partially.

Opening a new native, XLSX, or JSON workbook is a session replacement rather than a normal edit transaction. It rebuilds the graph and clears history. CSV import is a normal transaction against the active sheet.

## Recovery design

The frontend marks a workbook dirty only after a successful transaction. `RecoveryAutosave` debounces for 750 milliseconds, serializes writes, flushes the mutation queue and transaction tail, then invokes the recovery command with the same native metadata used by Save Workbook.

The backend stores recoveries under the Tauri per-user app data directory:

```text
app data/recovery/<recovery-id>.900sheets.recovery
```

Recovery IDs are validated before path construction. The store rejects a symlink root or target and requires regular files. Writes use a unique create-new temporary path, synchronize file contents, atomically replace the target, and clean up temporary files. Unix synchronizes the parent directory. Windows uses `MoveFileExW` with replace and write-through flags.

Discard is a two-stage operation. The discoverable snapshot is first atomically moved to a `.cleanup-pending` path. Deletion follows. If deletion fails, the stale snapshot cannot masquerade as current recovery data, and Save Workbook can retry cleanup under the retained identity.

Startup discovery returns only current recovery files, newest first. Restoring validates the native payload before replacing the workbook. Invalid data is moved to quarantine. The UI offers each snapshot explicitly, preserves unselected snapshots, and discards only the one a user declines.

The close handler prevents normal close, flushes pending edits, writes a final recovery for dirty state, and destroys the window only after success. A failed final write requires an explicit user decision.

## Persistence and file boundaries

- `sheets-json` owns JSON exchange and native format version 1.
- `sheets-xlsx` owns bounded OOXML import and export.
- `sheets-csv` owns delimited data import and export.
- `sheets-print` owns print layout and PDF output.
- Native file writes use a sibling temporary file, file synchronization, and atomic rename.
- Imported content is validated before it can replace the workbook.
- Recovery never writes to the source path or saved workbook path.

## Other crates

- `sheets-chart`: chart data extraction and SVG previews
- `sheets-pivot`: grouping, aggregation, filters, totals, and pivot output
- `sheets-validation`: validation rules and conditional-format matching
- `sheets-i18n`: locale formatting and accessibility helpers
- `sheets-advanced`: protection, locks, goal seek, scenarios, and comments

## Design rules

1. Rust owns authoritative workbook state.
2. Imports and formulas are bounded before expansion.
3. A failed operation must not partially mutate live state.
4. Sheet-scoped state follows stable sheet identity, not tab position.
5. Recovery is separate from normal saves and source files.
6. No telemetry or account is required.
7. Public compatibility claims must point to deterministic tests.
