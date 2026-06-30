# Sprint Process

## Sprint Workflow

1. **Sprint planning**: Define scope from the roadmap
2. **Implementation**: Work in feature branch `sprint-N-description`
3. **Sprint review** (mandatory before next sprint):
   - Code audit for correctness, style, and security
   - Test verification — all unit and integration tests pass
   - Documentation check — all new features documented
   - Performance check — no regressions on target hardware
   - Sprint record written to `docs/sprints/sprint-N.md`
4. **Fix gate**: Any issues found in review must be fixed before starting the next sprint
5. **Quality gate**: `./scripts/verify-local.sh` must pass with zero warnings

## Sprint Record Format

Each sprint record in `docs/sprints/sprint-N.md` must include:
- Sprint goal
- Scope delivered (checklist)
- Validation results (test output summary)
- Decisions made
- Issues found and fixed
- Carry-over items (if any)
