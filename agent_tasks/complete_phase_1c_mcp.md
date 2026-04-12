# Phase 1c - MCP Server (Agent C)

**Depends on**: Phase 0 (Contracts), Phase 1a (Parser), Phase 1b (Registry)  
**Stack**: Rust (MCP server); may use stubs for 1a/1b if parallel

---

## Objective

Implement MCP server exposing tools for analysis creation, reconciliation, and coverage queries. Wraps parser and registry.

---

## Deliverables

- MCP server (stdio transport) with tool dispatch
- All 8 tools implemented per contract
- Path validation (workspace root, no traversal)
- Request/response schemas per tool
- Error codes in responses

---

## Tool Implementation

| Tool | Calls | Path params |
|------|-------|-------------|
| `knowerage.create_or_update_doc` | Parser validation, file write | `analysis_path`, `source_path` |
| `knowerage.parse_doc_metadata` | Parser | `analysis_path` |
| `knowerage.reconcile_record` | Registry | `analysis_path` |
| `knowerage.reconcile_all` | Registry | `analysis_glob` (default: `knowerage/analysis/**/*.md`) |
| `knowerage.get_file_status` | Registry | `source_path` |
| `knowerage.list_stale` | Registry | — |
| `knowerage.get_tree` | Registry | `root`, `group_by` |
| `registry.export_report` | Registry + file write | `output_path`, `format` |

---

## Unit Tests (Expected Spec)

Run during implementation. All must pass before Phase 1c is complete.

| # | Test | Input | Expected |
|---|------|-------|----------|
| 1 | Request schema validation | Unknown/invalid fields in request | Rejected or ignored per contract |
| 2 | Path traversal rejected | `analysis_path: "../../etc/passwd"` | `E_PATH_TRAVERSAL` |
| 3 | Out-of-root path rejected | Absolute path outside workspace | `E_PATH_TRAVERSAL` |
| 4 | Export format validation | `format: "invalid"` | Error with clear message |
| 5 | create_or_update_doc success | Valid params | `ok: true`, file created |
| 6 | parse_doc_metadata success | Valid analysis path | Parsed metadata returned |
| 7 | reconcile_record success | Valid analysis path | Status + record returned |
| 8 | get_file_status success | Valid source path | `analyzed_ranges`, `missing_ranges`, `coverage_percent` |

---

## Stubs / Interfaces

If working in parallel with 1a/1b, define:
- Parser interface: `parse(path) -> Result<ParsedMetadata, Error>`
- Registry interface: `reconcile(path)`, `reconcile_all(glob)`, `get_file_status(path)`, etc.

Implement real calls once 1a and 1b are available.

---

## Acceptance Criteria

- [ ] All 8 unit tests pass
- [ ] Every path parameter validated against workspace root
- [ ] Deterministic response shape for success and error
- [ ] MCP server runs via stdio for Cursor/host
