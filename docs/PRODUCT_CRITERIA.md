# Product acceptance criteria

900Sheets exists for schools, small businesses and community organizations that need a useful spreadsheet without a paid subscription, an account or dependable internet access. A smaller interface alone is insufficient: dependable file handling, understandable controls and measured resource use are the product contract.

These criteria were established during the 2026-09-07 audit. Hardware figures below are engineering targets, not verified minimum requirements. See [competitor research](COMPETITOR_RESEARCH.md) for source-backed comparisons and [audit evidence](audits/2026-09-07-product-readiness.md) for observed results.

## Target machines and workloads

The primary validation target is a refurbished 64-bit, two-core computer with 4 GiB RAM, integrated graphics, a SATA disk or modest SSD, and a 1024 × 768 display. Record the exact CPU model, storage, OS version, system webview and app revision. Test 2 GiB systems separately as an exploratory tier; do not advertise support based on an untested assumption. Modern ARM macOS results and CPU-throttled browser tests cannot substitute for an older x86 computer.

Windows and Linux are essential reach targets. A supported, maintained OS matters alongside hardware age. No Windows 7/8 support, universal old-CPU support, 32-bit support or Linux package availability is claimed by this document. Choose and test the Linux distribution, CPU instruction baseline and package before claiming support.

Use only invented fixtures:

| Workload | Contents | Purpose |
| --- | --- | --- |
| Everyday | 1,000 rows × 10 columns with labels, numbers, 1,000 arithmetic/SUM formulas, dates and currency formatting | School budget or shop accounts |
| Working dataset | 10,000 rows × 10 columns, a header, values, number formats and one cross-sheet summary | Import, sort, copy, save and reopen |
| Sparse navigation | 50,000 stored cells dispersed over engine bounds | Distant navigation without dense allocation |
| Stress | Existing 500 × 500 dense benchmark and maximum-bound selection | Budget rejection, failure clarity and responsiveness |

## Required behavior

| Criterion | Acceptance rule | Verification |
| --- | --- | --- |
| Useful first session | A new user can create or choose a template, enter a total, format it, undo, save and reopen without reading developer docs | Observed task walkthrough and UI regressions |
| Offline use | With outbound traffic unavailable, first launch, template creation, edit, formula evaluation, native save/reopen and recovery work without accounts or remote assets | Packaged application test on each target OS; browser mocks prove only UI behavior |
| Offline installation | A complete installer can be transferred by USB and installed with networking disconnected, including an absent webview runtime | Clean Windows machine without WebView2; repeat launch after reboot |
| Workbook safety | Cancel/defer retains recoveries; failed mutations leave source work intact; unsupported edits are rejected before data loss | Rust and browser regression tests |
| Compatibility clarity | Supported XLSX subset and feature loss are documented; no full Excel/Numbers parity claim; originals can be retained | Compatibility matrix plus manual Excel/Calc round trip |
| Clear document state | Filename, unsaved state, save progress and primary Open/Save actions remain understandable | UI tests and visual review |
| Small display | At 1024 × 768, main controls, formula entry, grid and sheet tabs remain reachable; 800 × 500 is a constrained fallback | Browser layout tests, then native checks |
| Accessible controls | Keyboard operation, visible focus, control labels, selection state, reduced motion and high-contrast review | Automated checks plus manual screen-reader verification |
| Bounded work | Large selections, formulas and imports fail explicitly before unbounded allocation; ordinary formatting and navigation do not create a full-grid DOM | Unit/integration/browser regressions |
| Open-source maintainability | Behavior changes include close-layer regressions, user documentation and compatibility evidence; no public claims from mock-only or unrun tests | Independent review and complete quality gate |

The Windows configuration now selects Tauri's `offlineInstaller` mode. This includes the WebView2 runtime installer rather than relying on a download during installation. It increases the distribution payload; record actual installer size per release. It does not prove that an offline installation succeeded. See [Tauri's Windows installer documentation](https://v2.tauri.app/distribute/windows-installer/#offline-installer).

## Resource gates

`node scripts/check-resource-budgets.mjs` runs after the frontend build in the local gate and CI. It enforces aggregate JavaScript at or below 100 KiB gzip, CSS at or below 20 KiB gzip, and raw emitted assets at or below 1 MiB. It also protects the Windows offline-runtime configuration. These are growth guardrails, not measurements of installed size, native process memory or speed.

For the primary physical target, use these provisional acceptance thresholds:

| Measurement | Target | Method |
| --- | --- | --- |
| Cold launch to editable blank grid | ≤ 5 seconds | Five cold launches; record every result and median |
| Everyday edit/navigation feedback | p95 ≤ 100 ms | 100 edits/navigation actions after workbook load |
| Everyday native open/save | ≤ 2 seconds each | Five repetitions; primary save and backup work reported separately |
| Working-dataset open/save | ≤ 5 seconds each | Five repetitions with identical fixture and storage |
| Idle total app memory | ≤ 250 MiB | Include webview/helper processes after 60 seconds |
| Working-dataset total app memory | ≤ 500 MiB | Include helper processes; record peak and post-idle use |
| Long operation behavior | No unexplained freeze or partial mutation | Record operations exceeding one second; progress/cancel work remains a follow-up |

Record power mode, other active applications, warm/cold cache and any OS paging. Lowering the test machine's resolution or throttling a modern CPU alone does not prove these targets. If a target fails, record the failure, fix the bottleneck or explicitly narrow the supported workload. Do not silently raise the budget.

## Release gate and remaining validation

The code quality gate, independent review and per-platform packaged checks must pass before publication. A modern-host benchmark, mocked browser suite, or static installer setting cannot close physical-hardware, native accessibility, offline installation, Linux packaging or Microsoft Excel verification gates. The audit report tracks those remaining items explicitly.
