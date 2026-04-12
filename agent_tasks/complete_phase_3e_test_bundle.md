# Phase 3e - Test: Export Bundle (Agent G)

**Depends on**: Phase 2b (Export), Phase 1b (Registry)  
**Stack**: Node.js or Rust

---

## Objective

Integration tests for NotebookLM export bundle. Verifies selection, generation, and manifest.

---

## Scope

- Selection strategies
- Bundle structure (`combined.md`, `toc.md`, zip)
- Export manifest
- Error handling for invalid input

---

## Integration Tests

| # | Test | Steps | Expected |
|---|------|-------|----------|
| 1 | Export 50 selected analyses | Select top 50, generate bundle | Bundle contains up to 50, valid structure |
| 2 | Bundle structurally valid | Generate bundle, parse output | Valid markdown, toc present |
| 3 | Mixed valid/invalid input | Select with some missing paths | Partial success; errors reported |
| 4 | Manifest traceability | Export, read manifest | All selected files + hashes in manifest |
| 5 | Deterministic output | Same selection twice | Identical bundle (or documented diff) |

---

## Regression Tests

- All Phase 2b unit tests re-run
- De-duplication, selection parser

---

## Acceptance Criteria

- [ ] All integration tests pass
- [ ] Bundle suitable for NotebookLM ingestion
- [ ] No crash on invalid input
