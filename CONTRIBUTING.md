# Contributing to 900Sheets

We welcome contributions from developers worldwide — especially those in the regions 900Sheets serves. Every line of code from a developer in Lagos, Nairobi, Accra, or Mumbai makes this tool better for the people it's built for.

## Setup

1. Clone the repository
2. Install dependencies:
   ```bash
   npm install --prefix apps/desktop
   ```
3. Run in development mode:
   ```bash
   npm run tauri:dev --prefix apps/desktop
   ```

## Coding Standards

- Rust code must pass `cargo clippy` with zero warnings
- Rust code must pass `cargo test --workspace`
- Frontend code must pass `npm run check --prefix apps/desktop`
- All new features must include unit tests
- All new features must include matching documentation updates

## Sprint Process

1. Each sprint has a defined scope from the roadmap
2. Work in feature branches: `sprint-N-description`
3. After each sprint, run the sprint review:
   - Code audit for correctness, style, and security
   - Test verification — all tests must pass
   - Documentation check — all features documented
   - Performance check — no regressions
   - Sprint record written to `docs/sprints/sprint-N.md`
4. **Do not proceed to the next sprint until the current one has been fixed**

## Pull Request Process

1. Create a feature branch from `main`
2. Make your changes with matching tests and documentation
3. Run `./scripts/verify-local.sh` — must pass with zero warnings
4. Open a pull request with a clear description
5. Address review feedback
6. Squash-merge after approval

## Quick Contribution Ideas

- Add a formula function to `crates/sheets-formula`
- Improve grid rendering performance in `apps/desktop/src/App.svelte`
- Add a test fixture to `crates/sheets-fixtures`
- Improve documentation clarity in `docs/`
- Report bugs in your operating environment
