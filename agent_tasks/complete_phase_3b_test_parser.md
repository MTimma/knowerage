# Phase 3b - Test: Parser (Agent G)

**Depends on**: Phase 1a (Parser)  
**Stack**: Rust

---

## Objective

Integration and regression tests for the Metadata Parser. Complements unit tests done in Phase 1a.

---

## Scope

- Frontmatter extraction
- Range validation and normalization
- Error code mapping
- Edge cases and malformed input

---

## Integration Tests

| # | Test | Steps | Expected |
|---|------|-------|----------|
| 1 | Parse real analysis file | Read existing analysis .md from disk | Parsed metadata matches file |
| 2 | Parse with extra frontmatter keys | YAML with unknown keys | Unknown keys ignored; required parsed |
| 3 | Missing closing `---` | Malformed frontmatter | `E_DOC_PARSE` or equivalent |
| 4 | Empty file | Zero-byte .md | Clear error |
| 5 | Body-only no frontmatter | No `---` block | `E_DOC_PARSE` |

---

## Regression Tests

- All Phase 1a unit tests re-run
- Overlapping/adjacent range merge correctness
- Deterministic output for same input

---

## Acceptance Criteria

- [ ] All integration tests pass
- [ ] Error codes match contract
- [ ] Parser handles real-world analysis files
