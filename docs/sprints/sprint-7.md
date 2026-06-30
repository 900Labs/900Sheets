# Sprint 7: Data Tools — Sort, Filter, Find/Replace

## Goal
Implement data manipulation tools: sorting, filtering, and find/replace operations on sheet data.

## Scope Delivered

### Sort (sheets-core/src/data_tools.rs)
- [x] Sort sheet rows by a specified column (ascending/descending)
- [x] Numeric vs text comparison (numbers sort before text)
- [x] Boolean comparison
- [x] Empty cell handling (empty sorts first)
- [x] In-place sort (apply_sort) that rewrites sheet data
- [x] Non-destructive sort (sort_sheet) that returns sorted cell list
- [x] Configurable row range (start_row to end_row)
- [x] 4 sort tests

### Filter
- [x] FilterCriteria with column and condition
- [x] FilterCondition variants: Equals, Contains, StartsWith, EndsWith, NotEquals, NotContains, GreaterThan, LessThan, GreaterThanOrEqual, LessThanOrEqual, IsEmpty, IsNotEmpty
- [x] Case-insensitive text matching
- [x] Multiple criteria (AND logic)
- [x] Non-destructive filter (returns matching row indices)
- [x] apply_filter (returns filtered cell data with compacted rows)
- [x] 6 filter tests

### Find/Replace
- [x] Find all matches in sheet (case-sensitive and case-insensitive)
- [x] Find first match
- [x] Partial match support (substring search)
- [x] Replace with count
- [x] Case-insensitive replace
- [x] Replace all occurrences
- [x] Preserves cell type on replace (text, number, formula)
- [x] 7 find/replace tests

## Validation Results

- cargo test --workspace: 178 tests pass (62 core + 74 formula + 10 xlsx + 18 csv + 14 json)
- cargo clippy --workspace -- -D warnings: clean
- cargo fmt --all -- --check: clean
- npm run check: 0 errors, 0 warnings

## Decisions

- Data tools implemented in sheets-core (no new crate needed)
- Sort: numbers < booleans < text (lexicographic), empty cells sort first
- Filter: AND logic for multiple criteria
- Find: returns SearchResult with row, col, matched_text
- Replace: preserves cell type (text/number/formula) when possible
- Case-insensitive replace uses custom implementation (no regex dependency)

## Issues Found

None.

## Carry-Over

- Wire data tools into Tauri IPC commands (Sprint 8)
- Frontend UI for sort/filter/find/replace deferred to Sprint 8
- Multi-column sort deferred
- Advanced filter (OR logic, custom predicates) deferred
