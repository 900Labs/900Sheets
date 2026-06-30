# Sprint 11: Advanced Formula Functions

## Goal
Add ~100 additional formula functions across statistical, financial, engineering, date/time, lookup, and array categories.

## What Was Built
- **Extended**: `sheets-formula` functions registry
- Statistical: MEDIAN, MODE, STDEV, VAR, PERCENTILE, QUARTILE, RANK, LARGE, SMALL, etc.
- Financial: NPV, IRR, PV, FV, PMT, RATE, NPER, SLN, SYD, DDB, etc.
- Engineering: BIN2DEC, DEC2BIN, HEX2DEC, etc.
- Date/Time: NETWORKDAYS, WEEKNUM, EDATE, EOMONTH, YEARFRAC, etc.
- Lookup: VLOOKUP, HLOOKUP, INDEX, MATCH, OFFSET, INDIRECT, etc.
- Array: TRANSPOSE, FREQUENCY, GROWTH, TREND, LINEST
- Total: 160+ built-in functions
- Tauri IPC: available through `evaluate_formula`

## Tests
110 unit tests across the formula engine including all new functions.

## Status: Complete ✅
