# Sprint 15: Advanced Features

## Goal
Implement sheet protection, goal seek, scenario manager, and cell comments.

## What Was Built
- **Crate**: `sheets-advanced`
- **Sheet protection**: `SheetProtection` with password hashing, granular permissions (select, format, insert, delete, sort, filter, pivot), `protect()`/`unprotect()` with password verification
- **Cell locking**: `CellLockManager` with per-cell and range-based lock/unlock, default locked state
- **Goal seek**: `goal_seek()` using Newton's method with numerical derivatives, bisection fallback with range expansion. `GoalSeekConfig` with target cell, target value, input cell, max iterations, tolerance. Returns `GoalSeekResult` with success status, input value, achieved value, iteration count
- **Scenario manager**: `ScenarioManager` for what-if analysis - add/remove/apply named scenarios, `create_from_sheet()` to capture current state, `summary()` report generation, active scenario tracking
- **Cell comments**: `CommentManager` with add/get/remove/update/toggle visibility, author tracking, iteration
- Tauri IPC: `protect_sheet`, `unprotect_sheet`, `goal_seek_cmd`, `apply_scenario`, `get_cell_comment`, `add_cell_comment`, `remove_cell_comment`, `list_comments`

## Tests
32 unit tests covering protection, locking, goal seek, scenarios, and comments.

## Status: Complete ✅
