# Architecture

900Sheets uses a Rust-owned workbook model behind a Tauri v2 command boundary. The Svelte 5 frontend is a projection of that state and coordinates user interactions, transactions, recovery scheduling, and bounded grid rendering.

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

## Virtualized grid and navigation

The workbook and desktop grid share the same zero-based bounds: 1,000,000 rows and 16,384 columns. The UI does not create a DOM node for every coordinate. It derives a small row and column window from the scroll position, viewport size, zoom, frozen panes, hidden rows, and fixed overscan. Spacer columns preserve the full scroll geometry while keeping the rendered DOM bounded.

**Go To** parses strict A1 cell or range input and rejects an address outside `A1:XFD1000000`. Selecting a distant address updates logical selection, reveals the appropriate virtual window, and returns focus to the single grid focus target. The grid exposes total row and column counts plus active-cell and selection semantics to assistive technology.

## Workbook and formula graph

`sheets-core` stores sheets sparsely. Each sheet has a stable identity that is independent of its position and display name. Stable identities keep formula dependencies, comments, protection, locks, and frontend feature metadata attached to the correct sheet through rename, delete, undo, redo, and native reopen operations.

`sheets-formula` tokenizes and parses A1 references with optional sheet qualifiers. The workbook provider resolves a sheet name to a stable identity and evaluates local or cross-sheet values. The dependency graph uses `(stable_sheet_id, row, column)` keys, so a source change can find dependents across the workbook.

Graph construction, formula replacement, transaction commit, and workbook replacement reject circular dependencies. Reference expansion is bounded to 100,000 cells per formula in reference collection, dependency construction, and evaluation.

Dependency cycle detection walks iteratively with a visited set, so shared branches are not repeatedly traversed and long graphs do not consume the call stack. Formula parsing also bounds bytes, tokens and expression depth. Desktop reads use a fresh evaluation session with memoized formula values, active-reference detection and depth/work budgets. The cache is local to one read, so it does not retain results across workbook edits.

Sheet projection includes populated cells and independently stored formatted blanks without duplicate coordinates. Each projected numeric result supplies a finite `numeric_value` separately from source text and formatted display text. The UI uses that value for selection statistics and numeric conditional rules, including formulas formatted as currencies or percentages. This remains a full sparse-sheet projection; viewport virtualization alone does not bound IPC payload or transaction cloning cost.

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

## Recovery and backup design

The frontend marks a workbook dirty only after a successful transaction. `RecoveryAutosave` uses a configurable debounce interval, serializes writes, flushes the mutation queue and transaction tail, then invokes the recovery command with the same native metadata used by Save Workbook. Changing the interval invalidates and reschedules a pending timer without allowing a stale write to pass an in-flight flush.

The backend stores recoveries under the Tauri per-user app data directory:

```text
app data/recovery/<recovery-id>.900sheets.recovery
```

Recovery IDs are validated before path construction. The store rejects a symlink root or target and requires regular files. Writes use a unique create-new temporary path, synchronize file contents, atomically replace the target, and clean up temporary files. Unix synchronizes the parent directory. Windows uses `MoveFileExW` with replace and write-through flags.

Discard is a two-stage operation. The discoverable snapshot is first atomically moved to a `.cleanup-pending` path. Deletion follows. If deletion fails, the stale snapshot cannot masquerade as current recovery data, and Save Workbook can retry cleanup under the retained identity.

Startup discovery returns only current recovery files, newest first. Restoring validates the native payload before replacing the workbook. Invalid data is moved to quarantine. Startup prompts preserve unrelated snapshots. The **Recovery and Backups** panel can also inspect sheet summaries, restore, or delete a selected snapshot after startup.

The close handler prevents normal close, flushes pending edits, writes a final recovery for dirty state, and destroys the window only after success. A failed final write requires an explicit user decision.

Rotating native backups serve a different purpose. After a primary `.900sheets` save succeeds, the backend writes a private backup envelope under the per-user app data directory. The envelope stores the native workbook, a generated backup identity, the saved document's filename, an opaque document grouping key, creation time, and size. It does not persist the source path. The store keeps the newest five backups for each document.

Backup writes use the same private-directory, regular-file, no-follow, synchronization, and atomic-replacement rules as other durable writes. A backup or rotation failure does not turn an already successful primary save into a failure. The UI reports it as a separate warning. Backup restore validates the complete native candidate before replacing live state. The **Recovery and Backups** panel can inspect, restore, or delete one version explicitly.

Recovery and rotating backups are intentionally separate:

- Recovery follows dirty session state and protects work that may not have been saved.
- A normal save retires the current recovery.
- A rotating backup is created only after a successful native save and represents saved versions of one document.
- Neither store writes over the user's source file during inspection or restore.

## Persistence and file boundaries

- `sheets-json` owns JSON exchange and native format version 1.
- `sheets-xlsx` owns bounded OOXML import and export.
- `sheets-csv` owns delimited data import and export.
- `sheets-print` owns print layout and PDF output.
- Native file writes use a sibling temporary file, file synchronization, and atomic rename.
- Imported content is validated before it can replace the workbook.
- Recovery never writes to the source path or saved workbook path.

XLSX import produces a document containing the workbook plus sheet-aligned feature records. The desktop maps supported data-validation and conditional-formatting records into stable sheet-scoped state. Export consumes the same aligned records, rejects mapping mismatches and unsupported rule types, and emits validation elements, conditional rules, and differential styles. This is a bounded subset, not a general OOXML preservation layer.

Before any export file is selected, the desktop asks the Rust backend for an `ExportPreflight` result. CSV, JSON, and PDF report the dense coordinate area they would process and remain subject to their 5,000,000-cell safety limits. XLSX reports stored cell and format coordinates because it stays sparse. A blocked preflight never starts the exporter.

## Release workflow boundary

The release workflow validates that a tag matches the workspace version, builds macOS and Windows artifacts, and records SHA-256 files and signing provenance. macOS Developer ID signing and notarization require Apple credentials. Without them, the workflow records ad hoc signing and no notarization, uploads an inspection artifact, and refuses GitHub Release publication. Publication requires Developer ID signing, accepted notarization, stapling, and verification. Windows Authenticode signing likewise requires configured credentials; an unsigned installer can proceed only with explicit unsigned provenance.

Those branches describe workflow behavior. A successful signed, notarized, installed, or clean-machine-tested release is claimed only after the specific tag and artifacts have been verified.

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
5. Recovery is separate from normal saves, rotating backups, and source files.
6. No telemetry or account is required.
7. Public compatibility claims must point to deterministic tests.
