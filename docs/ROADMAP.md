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
