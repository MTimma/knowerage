# Phase 2b - NotebookLM Export Bundle (Agent E)

**Depends on**: Phase 0 (Contracts), Phase 1b (Registry)  
**Stack**: Node.js or Rust

---

## Objective

Provide export workflow that packages selected analysis files (e.g. 50) for NotebookLM ingestion. Export-first; direct API upload only if supported later.

---

## Deliverables

- Selection strategy (manual list, top N, newest)
- Bundle generator: `combined.md`, `toc.md`, optional zip
- Export metadata manifest for traceability

---

## Implementation Steps

1. Accept file selection input (explicit list or rule-based query)
2. Validate selected files and metadata completeness
3. Generate `toc.md` with source links and range summaries
4. Generate single merged markdown or archive package
5. Persist export manifest with timestamp and file hashes

---

## Unit Tests (Expected Spec)

Run during implementation. All must pass before Phase 2b is complete.

| # | Test | Input | Expected |
|---|------|-------|----------|
| 1 | Selection parser fixed count | "top 50" or limit 50 | Returns up to 50 paths |
| 2 | Duplicate paths de-duplicated | `["a.md","a.md","b.md"]` | `["a.md","b.md"]` |
| 3 | Export manifest contains files | Selected 10 analyses | Manifest lists all 10 with hashes |
| 4 | Export manifest timestamp | Any export | `created_at` present and valid ISO 8601 |
| 5 | Invalid file in selection | Mixed valid/invalid paths | Partial success; invalid reported |
| 6 | Empty selection | `[]` | Empty bundle or clear error |

---

## Integration Test

- Generated bundle structurally valid and readable
- Mixed valid/invalid input produces partial-failure report without crash

---

## Caveat

Direct NotebookLM API upload may be unavailable; keep export-first until official API support verified.

---

## Acceptance Criteria

- [ ] All 6 unit tests pass
- [ ] Export output deterministic for same selection
- [ ] Missing/invalid files reported with clear errors
- [ ] Bundle includes enough metadata for downstream use
