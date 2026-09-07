# Roadmap

The roadmap is ordered by user risk and community value. It does not assign dates or promise delivery.

## v0.5.0 baseline

The v0.5.0 baseline includes:

- The v0.4.0 workbook-safety foundation: cross-sheet formulas, stable sheet identities, bounded atomic undo and redo, and serialized crash recovery
- A virtualized editor covering the engine bounds of 1,000,000 rows and 16,384 columns
- Bounded cell and range navigation through **Go To**
- Export preflight for XLSX, CSV, JSON, and PDF
- A post-startup **Recovery and Backups** panel and configurable recovery timing
- Private five-version native backup rotation, managed separately from crash recovery
- XLSX import and export for the supported validation and conditional-formatting subset
- Four starter templates with invented data and a local first-run guide
- Persisted English, Swedish, and Spanish grid-navigation and accessibility labels
- Keyboard-focus and screen-reader improvements for the virtualized grid and dialogs
- A release workflow that can produce macOS and Windows artifacts, verify provenance, and publish only after the macOS app is Developer ID signed, notarized, stapled, and reverified

The exact release boundary is documented in [COMPATIBILITY.md](COMPATIBILITY.md), with test evidence in [COMPATIBILITY_MATRIX.md](COMPATIBILITY_MATRIX.md).

## v0.5.0 release readiness

Live status checked 2026-09-07: PR #8 merged on 2026-07-25. PRs #9 through #11 subsequently added Excel table/chart preservation and fixes. CI for main commit `91fee5d` passed. Historical release-prep counts are not current acceptance evidence. The [product audit](audits/2026-09-07-product-readiness.md) records subsequent fixes and verification. Publication remains gated on the items below.

- **Apple signing and notarization.** The macOS publish gate requires a Developer ID signed, notarized, and stapled artifact. This needs an Apple Developer account and six repository secrets: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID`. Credential availability was not reverified in this audit. Without them the build falls back to ad hoc signing and the public-release verification step blocks by design.
- **Microsoft Excel open/edit/save/reopen manual check.** Currently marked "Not checked" in the compatibility matrix. Requires a Windows/macOS machine running Excel.
- **README release artifacts.** Capture an invented-data application screenshot, add it to the README with a task example, and rerun the privacy scan.
- **Hosted CI.** Require green CI and independent review on the current change. Validate the Windows installer through the release workflow separately; main CI does not prove a packaged offline installation.
- **Ship.** Use a reviewed merge commit, create the annotated version-matched tag, verify notarization/Windows provenance/hashes/artifacts, and publish only after every release gate passes. Reconcile milestone issues against delivered behavior. Repository operations use the `900Labs` GitHub identity.

## Product readiness priorities

The [competitor comparison](COMPETITOR_RESEARCH.md) and [acceptance criteria](PRODUCT_CRITERIA.md) define the next work by user impact:

1. Preserve work and calculation correctness: recovery, atomic clipboard actions, supported structural edits and bounded formula evaluation.
2. Make existing tools usable: visible workbook/save state, selection-aware formatting, practical menus and small-screen access without runtime dependencies.
3. Prove the mission: packaged offline installation/use, physical 4 GiB older-computer measurements, and a declared Linux package/CPU baseline.
4. Deepen everyday editing: reference-aware cut across formula workbooks and sheets; metadata-aware row/column edits; incremental sheet projection and long-operation progress/cancellation.
5. Expand compatibility and accessibility using exact fixtures and observed target applications. Complete localization and native screen-reader checks remain explicit work.

## Next priorities

### Compatibility depth

- Build a repeatable, publishable Microsoft Excel desktop verification procedure using invented data.
- Add Excel tables, native chart objects, and pivot metadata in separate, bounded increments. (Tables and native charts are preserved through XLSX round trips; pivot metadata is the remaining increment.)
- Expand formula and feature fixtures across multiple LibreOffice and Excel versions.
- Define explicit preservation or rejection behavior for images, shapes, embedded objects, and external links.

### Distribution confidence

- Complete clean-machine install, upgrade, launch, and uninstall checks for each published macOS and Windows artifact.
- Add packaged GUI smoke coverage where hosted runners are reliable.
- Define supported Linux distributions and create packages for that set.
- Publish platform signing policy and certificate-rotation procedures.

### Large-workbook workflows

- Add name-box history, named-range navigation, and recently visited locations.
- Profile and tune large copy, paste, sort, filter, formula refresh, and export operations.
- Add cancellation and progress reporting for operations that can take noticeable time.
- Expand benchmark baselines across supported operating systems.

### Accessibility and language

- Complete translation coverage beyond grid navigation and core accessibility labels.
- Add more screen-reader regression coverage and manual verification notes.
- Test high contrast, reduced motion, zoom, and platform-specific keyboard conventions.
- Invite reviewed community translations with a documented acceptance process.

### Community use

- Add more task-based examples and release screenshots.
- Expand the starter-template catalog through reviewed, contributor-sized changes.
- Improve template formatting and accessible instructions without adding real personal or financial data.
- Use the community milestone and issue plan to keep contributions small, testable, and nonduplicative.

## Historical records

The sprint files in `docs/sprints/` describe earlier implementation phases. They are historical notes, not the current release contract.
