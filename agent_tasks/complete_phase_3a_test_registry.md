# Phase 3a - Test: Registry (Agent G)

**Depends on**: Phase 1b (Registry), Phase 2c (Security)  
**Stack**: Rust

---

## Objective

Integration and regression tests for the Registry component. Complements unit tests done in Phase 1b.

---

## Scope

- Registry read/write
- Hashing and freshness logic
- Reconcile-one and reconcile-all
- Atomic writes and crash recovery
- Status transitions

---

## Integration Tests

| # | Test | Steps | Expected |
|---|------|-------|----------|
| 1 | Reconcile-all after source changes | Create analyses, change source, run reconcile_all | Statuses updated correctly |
| 2 | Interrupted write | Start registry write, kill process | No corrupted JSON; old or new valid |
| 3 | Full rebuild from analysis files | Delete knowerage/registry.json, run reconcile_all with glob | Registry rebuilt from MD files |
| 4 | Mixed status summary | Setup fresh/stale_doc/stale_src/missing_src/dangling_doc | Counts match expected |
| 5 | Large repo incremental | 100+ analysis files | Reconcile completes; no OOM |

---

## Regression Tests

- Hash match -> fresh (re-run from Phase 1b)
- Hash mismatch -> correct stale status
- Atomic write integrity

---

## Acceptance Criteria

- [ ] All integration tests pass
- [ ] No registry corruption under failure scenarios
- [ ] Deterministic results for same inputs
