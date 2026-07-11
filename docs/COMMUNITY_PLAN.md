# Community contribution plan

This plan starts from the v0.5.0 community-use baseline and lists small, reviewable follow-up contributions. It does not assign dates or promise inclusion in a specific release.

## v0.5.0 baseline

The v0.5.0 source includes:

- A local first-run guide that offers a blank workbook or one of four starter templates
- School budget, small-business accounts, community project, and household planning templates with invented data
- Formula rebasing when a template is inserted away from `A1`
- Full-grid **Go To** navigation and virtualized rendering
- Persisted English, Swedish, and Spanish grid-navigation and core accessibility labels
- Recovery and rotating-backup management

First-run completion is stored locally as a single preference. It does not include a name, email address, workbook content, analytics identifier, or device information. If local preference storage is unavailable, onboarding remains available and the editor continues to work.

## Starter templates

All included examples use invented information and work offline.

| Template | Intended task | Useful completion check |
| --- | --- | --- |
| School Budget | Compare planned and actual term spending | Total and remaining formulas update after a value changes |
| Small Business Accounts | Record money in and out | The running balance updates for every sample transaction |
| Community Project | Assign work and track a simple budget | Owners, dates, status, and remaining funds are visible together |
| Household Planning | Compare a monthly plan with actual costs | Category and total differences update |

Templates are reusable definitions rather than bundled personal workbooks. Formula references are rebased when a table is inserted away from A1.

## Task-based documentation

The user-facing documentation for v0.5.0 should answer these tasks directly:

- Start a workbook from a template.
- Replace invented sample values with local information.
- Enter a formula and understand the result.
- Save a native workbook and export a supported exchange format.
- Recover unsaved work and manage retained recovery snapshots.
- Navigate to a distant cell with the name box or Go To control.
- Report a reproducible problem without sharing private workbook data.

Screenshots should be captured from the reviewed release candidate. They must show invented data, contain no personal paths or account details, and match the shipped interface.

## Suggested follow-up milestone

Use a milestone for post-v0.5 community improvements once maintainers choose the next release boundary. Do not assign an issue to a version before that scope is agreed.

The following issues are deliberately sized so contributors can work independently. Maintainers should confirm that an issue is still unclaimed before implementation starts.

### Documentation and templates

1. **Document four template workflows**
   - Add one short task walkthrough for each template.
   - Use invented values throughout.
   - Capture reviewed release-candidate screenshots after the UI is stable.

2. **Improve one starter template**
   - Start with a concrete user task and keep the change within one template.
   - Use only invented data.
   - Preserve formula-rebasing tests and add a focused regression when behavior changes.

3. **Test the first-run guide with a new user**
   - Ask a tester to create, edit, calculate, and save an invented workbook using published instructions.
   - Record unclear steps as documentation issues.
   - Do not collect account, device, or workbook-content analytics.

### Accessibility

4. **Audit keyboard access for the first-run and template panels**
   - Verify logical tab order, visible focus, Escape behavior, and focus return.
   - Add automated checks for the reliable interactions.

5. **Label the grid editing workflow for screen readers**
   - Announce the active cell, edit mode, formula content, and validation errors.
   - Record manual checks with at least one current screen reader.

6. **Verify zoom, contrast, and reduced motion**
   - Test the editor at 200 percent zoom.
   - Correct focus and text contrast failures.
   - Avoid motion that ignores the operating system preference.

### Community quality

7. **Expand the public-fixture privacy check**
   - Add safe detections for new fixture or screenshot formats without flagging invented examples.
   - Keep the check in the public-release gate.

8. **Create a compatibility report template**
   - Record the application version, operating system, source format, tested actions, and observed result.
   - Warn contributors not to attach private workbooks.

9. **Add a manual compatibility report**
   - Use the compatibility report template with a publishable invented workbook.
   - Record the exact application and operating-system versions.
   - Separate observed Excel or LibreOffice behavior from automated test evidence.

## Completion rules

An issue is complete only when behavior is tested at the closest reliable layer, user-facing claims match the implementation, and public artifacts contain no private information. Release inclusion still requires the repository quality gate and independent review.
