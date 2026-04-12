# Phase 2c - Security & Operational Hardening (Agent F)

**Depends on**: Phase 0 (Contracts)  
**Stack**: Rust (core), applies to Node where relevant

---

## Objective

Apply baseline security controls and operational reliability. Can run in parallel with Phase 1/2 once contracts exist.

---

## Deliverables

- Input validation policy for all MCP tools
- Registry write safety strategy (atomic writes, single-writer)
- Operational runbook for local deployment and recovery

---

## Security Controls

- Enforce workspace-root path allowlist
- Reject path traversal (`..`) and absolute path escapes
- Never persist secrets/tokens in markdown, registry, or reports
- Validate and sanitize user-provided strings before file writes
- Keep logs free of confidential payload data

---

## Operational Controls

- Single-writer lock or ownership for registry updates
- Atomic writes for all registry/report updates
- Periodic reconcile_all for self-healing
- Graceful handling of deleted/moved files and malformed metadata

---

## Unit Tests (Expected Spec)

Run during implementation. All must pass before Phase 2c is complete.

| # | Test | Input | Expected |
|---|------|-------|----------|
| 1 | Path validator rejects `../` | `"../../etc/passwd"` | Rejected |
| 2 | Path validator rejects out-of-root | Absolute path outside workspace | Rejected |
| 3 | Atomic write helper | Write then crash simulation | Either old or new file; never partial |
| 4 | Concurrent reconcile requests | Two simultaneous reconcile_all | Serialized; no corruption |
| 5 | Crash during write recovery | Kill process mid-write | Registry intact (old or new) |
| 6 | No secret-like values in report | Export report with sensitive-looking data | No credentials/tokens in output |

---

## Acceptance Criteria

- [ ] All 6 unit tests pass
- [ ] Path traversal attacks fail safely
- [ ] Concurrent updates do not corrupt state
- [ ] Recovery flow can rebuild registry from analysis files
