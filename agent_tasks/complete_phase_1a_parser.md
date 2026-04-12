# Phase 1a - Metadata Parser (Agent A)

**Depends on**: Phase 0 (Contracts)  
**Stack**: Rust (core)

---

## Objective

Implement metadata parser and validation for analysis markdown frontmatter. Analysis file path (`.md`) is the identifier; no `analysis_id`.

---

## Deliverables

- Parser that extracts YAML frontmatter from analysis `.md` files
- Validation for required keys: `source_file`, `covered_lines`, `analysis_date`
- Range normalization: sort, merge overlapping/adjacent ranges
- Error model returning contract error codes (`E_DOC_PARSE`, `E_RANGE_INVALID`)

---

## Implementation Steps

1. Parse YAML frontmatter (between `---` delimiters)
2. Validate required keys present
3. Validate `covered_lines` format: array of `[start, end]`, integers, `start >= 1`, `end >= start`
4. Normalize ranges: sort by start, merge overlaps and adjacent intervals
5. Return structured result or error with code + message

---

## Unit Tests (Expected Spec)

Run during implementation. All must pass before Phase 1a is complete.

| # | Test | Input | Expected |
|---|------|-------|----------|
| 1 | Valid frontmatter parses | Valid YAML with `source_file`, `covered_lines`, `analysis_date` | Success; normalized ranges returned |
| 2 | Missing `source_file` | Frontmatter without `source_file` | `E_DOC_PARSE` or validation error |
| 3 | Malformed `covered_lines` (string) | `covered_lines: "invalid"` | `E_RANGE_INVALID` |
| 4 | Malformed `covered_lines` (null) | `covered_lines: null` | `E_RANGE_INVALID` |
| 5 | Malformed `covered_lines` (object) | `covered_lines: {a: 1}` | `E_RANGE_INVALID` |
| 6 | Inverted range | `[200, 120]` | `E_RANGE_INVALID` |
| 7 | Overlapping ranges normalize | `[[1,10],[5,15]]` | Merged to `[[1,15]]` |
| 8 | Adjacent ranges merge | `[[1,10],[11,20]]` | Merged to `[[1,20]]` |
| 9 | Duplicate ranges collapse | `[[1,10],[1,10]]` | Single `[[1,10]]` |
| 10 | Non-integer in range | `[1.5, 10]` or `["1", 10]` | `E_RANGE_INVALID` |
| 11 | Zero or negative start | `[0, 10]` or `[-1, 10]` | `E_RANGE_INVALID` |

---

## Acceptance Criteria

- [ ] All 11 unit tests pass
- [ ] Parser output is deterministic for same input
- [ ] Error codes match contract
- [ ] No `analysis_id` in schema; analysis path is identifier
