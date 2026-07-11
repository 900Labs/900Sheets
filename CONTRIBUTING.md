# Contributing to 900Sheets

Contributions are welcome from spreadsheet users, developers, technical writers, translators, educators, and compatibility testers.

## Before you start

- Search open issues and pull requests to avoid duplicate work.
- Open an issue before a large feature or architecture change.
- Keep fixes focused on one behavior.
- Never add a workbook containing personal, customer, financial, or confidential data.

Good first contributions include documentation corrections, small accessibility improvements, formula tests, import and export fixtures with invented data, and focused performance fixes.

## Development setup

Install Rust 1.92.0, a supported Node.js version, and the Tauri v2 system prerequisites. Then run:

```bash
git clone https://github.com/900Labs/900Sheets.git
cd 900Sheets
npm ci --prefix apps/desktop
npm run tauri:dev --prefix apps/desktop
```

## Working rules

- Rust must pass `cargo fmt` and clippy with warnings denied.
- Behavior changes need tests at the closest reliable layer.
- File-format regressions need a minimal fixture or generated test archive.
- User-visible changes need matching documentation.
- Do not claim compatibility that the tests do not prove.
- Keep new prose direct and do not use em dashes.
- Preserve unrelated changes in the worktree.

## Validate your change

Run the complete gate:

```bash
./scripts/verify-local.sh
```

For public-release or packaging changes, also run:

```bash
./scripts/verify-public-release.sh
npm run tauri:build --prefix apps/desktop
```

## Pull requests

1. Create a branch from the current `main` branch.
2. Make the smallest complete change.
3. Add tests and documentation.
4. Run the local quality gate.
5. Complete the pull request template with exact verification evidence.
6. Address review feedback without mixing unrelated cleanup into the branch.

Maintainers may ask for an Excel or LibreOffice fixture when a change affects XLSX behavior. Fixtures must use invented data and must be safe to publish.

## Reporting problems

Use the issue forms and follow [SUPPORT.md](SUPPORT.md). Security vulnerabilities must follow [SECURITY.md](SECURITY.md) and must not be posted publicly.
