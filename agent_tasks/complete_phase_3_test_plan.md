# Phase 3 - Test Plan & Rollout (Agent G)

**Depends on**: All Phase 1 and Phase 2 tasks  
**Deliverable**: CI gates, phased rollout, overall test strategy

---

## Objective

Define test pyramid, CI quality gates, and rollout criteria. Orchestrates Phase 3a–3e and cross-cutting tests.

---

## Test Pyramid

| Level | Scope | Owner |
|-------|-------|-------|
| Unit | Per component (Parser, Registry, MCP, UI, Bundle, Security) | Phase 1a–2c |
| Integration | Component + dependencies | Phase 3a–3e |
| E2E | Full system via MCP host | Phase 3c |

---

## CI Quality Gates

- [ ] All unit tests pass (Rust: `cargo test`, Node: `npm test`)
- [ ] All integration tests pass
- [ ] No critical security validation failures
- [ ] Deterministic snapshot tests for key JSON outputs
- [ ] Lint/type-check: `cargo clippy`, `npm run lint`

---

## Cross-Cutting Integration Tests

| # | Test | Components | Expected |
|---|------|-------------|----------|
| 1 | Create analysis -> reconcile -> query coverage | Parser, Registry, MCP | Full flow succeeds |
| 2 | Reconcile-all across mixed statuses | Registry | Summary counts correct |
| 3 | File watcher (knowerage/registry changes) | Registry | Stale detection works |
| 4 | Local UI reflects registry | UI, Registry | Data consistent |
| 5 | Export bundle for 50 analyses | Export, Registry | Valid bundle |

---

## Phased Rollout

1. **Phase A**: Metadata + reconcile CLI/MCP tools (1a, 1b, 1c)
2. **Phase B**: Watcher + stale handling + full rebuild (1b extended)
3. **Phase C**: Coverage UI/report (2a)
4. **Phase D**: Export bundle (2b) + optional Git enrichment

---

## Acceptance Criteria

- [ ] New deployment can bootstrap from existing analysis files
- [ ] Regression suite catches stale detection and coverage math errors
- [ ] Rollback strategy documented for schema/tool changes
