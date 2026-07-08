# Security Policy

## Reporting a Vulnerability

To report a security vulnerability, email security@900labs.com.

Do not open a public GitHub issue for security vulnerabilities.

## Response Time

- We will acknowledge receipt within 48 hours
- We will provide an initial assessment within 7 days
- We will work with you to coordinate disclosure after a fix is available

## Scope

Security vulnerabilities in 900Sheets include:
- File parsing vulnerabilities (XLSX, CSV, JSON import)
- Memory safety issues in Rust code
- IPC boundary issues between Rust and the WebView
- Path traversal in file operations
- Any issue that could compromise user data on the local machine

## Out of Scope

- Vulnerabilities in dependencies (report upstream)
- Issues requiring physical access to the device
- Social engineering attacks

## Known `cargo audit` Warnings

Last verified on 2026-07-08, `cargo audit` reports no failing vulnerabilities
and exits successfully. It does report 17 warnings from transitive dependencies
introduced by the Tauri/wry Linux GTK/WebKit stack and parser tooling. These are
not in 900Sheets application code.

**Tracked warning groups:**

- GTK3 binding crates: `atk`, `atk-sys`, `gdk`, `gdk-sys`, `gdkwayland-sys`,
  `gdkx11`, `gdkx11-sys`, `gtk`, `gtk-sys`, `gtk3-macros`
- `glib` unsound iterator advisory: RUSTSEC-2024-0429
- `proc-macro-error` unmaintained advisory: RUSTSEC-2024-0370
- `unic-*` unmaintained advisories: RUSTSEC-2025-0075, RUSTSEC-2025-0080,
  RUSTSEC-2025-0081, RUSTSEC-2025-0098, RUSTSEC-2025-0100

**Mitigation:**

- Track Tauri, wry, and Linux GTK/WebKit dependency updates and bump them when
  upstream replacements are available
- Treat any future `cargo audit` vulnerability failure as release-blocking
- Keep import parsers and Tauri transitive dependencies current before release

**To review the current state:**

```bash
cargo install cargo-audit --locked
cargo audit
```
