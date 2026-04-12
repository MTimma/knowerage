# Phase 3c - Test: MCP Server (Agent G)

**Depends on**: Phase 1a (Parser), Phase 1b (Registry), Phase 1c (MCP)  
**Stack**: Rust

---

## Objective

Integration and end-to-end tests for the MCP server. Verifies tool dispatch, path validation, and full flow.

---

## Scope

- All 8 MCP tools
- Path validation and security
- Request/response schema
- MCP protocol compliance (stdio)

---

## Integration Tests

| # | Test | Steps | Expected |
|---|------|-------|----------|
| 1 | create_or_update_doc -> reconcile -> get_file_status | Full flow | Analysis created, reconciled, coverage returned |
| 2 | Malformed analysis surfaces parse error | Invalid frontmatter in create | `E_DOC_PARSE` in response |
| 3 | Path traversal in tool call | `analysis_path: "../../x"` | Request rejected |
| 4 | list_stale returns filtered items | Setup mixed statuses, call list_stale | Only requested statuses returned |
| 5 | get_tree with group_by | Call with `group_by: "directory"` | Grouped nodes returned |
| 6 | export_report produces valid file | Call with format and path | File created, valid content |

---

## End-to-End Tests

| # | Test | Steps | Expected |
|---|------|-------|----------|
| 1 | User workflow via MCP host | Cursor/agent starts MCP, calls tools | Tools respond correctly |
| 2 | stdio transport | Launch server, send JSON-RPC over stdin | Responses on stdout |

---

## Acceptance Criteria

- [ ] All integration tests pass
- [ ] E2E flow works with MCP host (if available)
- [ ] No path traversal or security bypass
