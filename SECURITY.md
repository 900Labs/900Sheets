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

## Known `cargo audit` Advisories

As of the latest audit, `cargo audit` reports warnings from transitive dependencies
introduced by the Tauri/wry Linux GTK/WebKit stack. These are **not** in application
code — they come from platform UI framework transitive crates.

**Tracked advisories (all transitive):**

- `glib` — unsound advisory (RUSTSEC-2023-0073)
- `unic` — unmaintained crate (RUSTSEC-2024-0336)
- `proc-macro-error` — unmaintained crate (RUSTSEC-2024-0341)
- Additional GTK/WebKit transitive warnings — 14 total

**Mitigation:**

- These only affect Linux builds using the GTK/WebKit backend
- macOS and Windows builds use native WebView backends without these dependencies
- We track Tauri and wry updates and will bump versions when upstream fixes land
- No application-level code is affected

**To review the current state:**

```bash
cargo install cargo-audit --locked
cargo audit
```
