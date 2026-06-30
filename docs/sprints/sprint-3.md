# Sprint 3: Formula Engine — Parser & Basic Functions

## Goal
Build a custom formula engine in Rust with tokenizer, parser, evaluator, dependency graph, and 60+ built-in functions.

## Scope Delivered

- [x] Tokenizer (lexer) — numbers, strings, booleans, cell refs, range refs, functions, operators
- [x] AST — Expr enum with CellRef, RangeRef, BinOp, UnaryOp, Function, Error nodes
- [x] Parser — Pratt parser with operator precedence, parenthesization, function calls
- [x] Evaluator with type coercion (Number, String, Boolean, Error, Empty)
- [x] CellProvider trait for cell value lookup
- [x] Dependency graph with cycle detection and topological sort
- [x] 60+ built-in functions:
  - Math: SUM, AVERAGE, MIN, MAX, ABS, ROUND, FLOOR, CEILING, POWER, SQRT, MOD, INT, EXP, LN, LOG10, PI, RAND, SIN, COS, TAN, ASIN, ACOS, ATAN, ATAN2, DEGREES, RADIANS, COUNT, COUNTA, PRODUCT, FACT, SIGN
  - Logical: IF, AND, OR, NOT, TRUE, FALSE, IFERROR, IFNA, XOR
  - Text: LEN, UPPER, LOWER, PROPER, TRIM, LEFT, RIGHT, MID, CONCATENATE, SUBSTITUTE, REPT, FIND, SEARCH, REPLACE, TEXT, VALUE
  - Info: ISNUMBER, ISTEXT, ISLOGICAL, ISERROR, ISEMPTY, ISBLANK, ISNONTEXT, ISODD, ISEVEN, NA, TYPE
- [x] Error types matching Excel conventions (#DIV/0!, #VALUE!, #REF!, #NAME?, #NUM!, #N/A, #ERROR!)
- [x] IFERROR/IFNA special handling (no error short-circuit)
- [x] 74 unit tests (54 new in sheets-formula)

## Validation Results

- cargo test --workspace: 94 tests pass (20 core + 74 formula)
- cargo clippy --workspace -- -D warnings: clean
- cargo fmt --all -- --check: clean
- npm run check: 0 errors, 0 warnings

## Decisions

- Manual FormulaError impl (no thiserror) for Excel-style error display
- Pratt parser for operator precedence
- FunctionRegistry with fn pointers (no dynamic dispatch overhead)
- Dependency graph tracks both forward deps and reverse dependents
- Topological sort for evaluation order
- IFERROR/IFNA bypass error short-circuiting in evaluator

## Issues Found

None.

## Carry-Over

- Wire formula evaluation into Tauri backend (Sprint 4+)
- Range references in functions need proper 2D array support (currently flattened)
- Date/time functions deferred (chrono dependency available)
- LOOKUP/VLOOKUP/HLOOKUP deferred to Sprint 7
