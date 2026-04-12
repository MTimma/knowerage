# Phase 1b - Registry & Reconciliation (Agent B)

**Depends on**: Phase 0 (Contracts)  
**Stack**: Rust (core)

---

## Objective

Implement single-writer registry (`knowerage/registry.json`), content hashing, and freshness reconciliation. Supports incremental and full rebuild.

---

## Deliverables

- Registry read/write with atomic updates (temp + rename)
- Content hashing (SHA-256) for analysis doc and source file
- Freshness algorithm: compare hashes, assign status
- Reconcile single record and reconcile-all
- File watcher (triggers full verification and reconciliation on changes)

---

## Implementation Steps

1. Implement atomic write helper for `knowerage/registry.json`
2. Hash analysis content and source content (SHA-256)
3. Compare current hashes with stored record hashes
4. Assign status: `fresh`, `stale_doc`, `stale_src`, `missing_src`, `dangling_doc`
5. Rebuild record when stale; preserve `record_created_at`, update `record_updated_at`
6. Implement reconcile-one and reconcile-all (glob over `knowerage/analysis/**/*.md`)
7. Implement file watcher: while Rust MCP is running (started by Cursor), when the agent makes any changes to contents in the `knowerage` folder or to `knowerage/registry.json`, the file watcher triggers full verification and reconciliation. This includes comparing source files mentioned in `registry.json` against their current content (in the project folder where `knowerage` is located) and detecting hash mismatches (updated content). Reconcile stale records accordingly.

---

## Unit Tests (Expected Spec)

Run during implementation. All must pass before Phase 1b is complete.

| # | Test | Input | Expected |
|---|------|-------|----------|
| 1 | Hash match -> fresh | Record hashes match current files | Status `fresh` |
| 2 | Analysis hash mismatch -> stale_doc | Analysis file changed | Status `stale_doc` |
| 3 | Source hash mismatch -> stale_src | Source file changed | Status `stale_src` |
| 4 | Source deleted -> missing_src | Source file not found | Status `missing_src` |
| 5 | Malformed record, doc exists -> dangling_doc | Record invalid but doc present | Status `dangling_doc` |
| 6 | Atomic write | Write then simulate crash | Either old or new file; never partial |
| 7 | Reconcile-all summary | Mixed status records | Stable counts per status |
| 8 | Large file hash (2k lines) | ~80KB source file | Hash computed in reasonable time (<100ms) |

---

## Acceptance Criteria

- [ ] All 8 unit tests pass
- [ ] Registry writes are crash-safe
- [ ] Hash-based staleness survives git pull/checkout scenarios
- [ ] Single-writer model enforced (or documented)
