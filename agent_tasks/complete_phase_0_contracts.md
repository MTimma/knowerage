# Phase 0 - Contracts & Schemas

**Agent**: 1 agent  
**Depends on**: —  
**Deliverable**: Single `contracts.md` (or equivalent) defining all shared specs.

---

## Objective

Produce a contracts document that all subsequent agents consume. No implementation—specification only.

---

## Deliverables

1. **Metadata schema** — Analysis markdown frontmatter contract
2. **Registry schema** — `registry.json` record shape and file format
3. **Status vocabulary** — Freshness states and transitions
4. **MCP tool list** — Tool names, input/output schemas, error codes
5. **Error code convention** — Canonical error codes for validation and runtime failures

---

## 1. Metadata Schema (from Task 01)

### Frontmatter contract

```yaml
source_file: "src/legacy/AuthService.java"   # required, string
covered_lines:                                 # required, array of [start, end]
  - [120, 165]
  - [210, 248]
analysis_date: "2026-03-01T10:25:00Z"        # required, ISO 8601
```

### Rules

- **Identifier**: Analysis file path (`.md`) is the identifier. No `analysis_id`.
- **Line ranges**: Inclusive `[start, end]`, `start >= 1`, `end >= start`, integers only.
- **Normalization**: Parser output must sort and merge overlapping/adjacent ranges.

---

## 2. Registry Schema (from Task 02)

### File

`knowerage/registry.json` — JSON array or object keyed by `analysis_path`. The `knowerage/` folder lives at the **root of the project being documented/analysed** (workspace root).

### Record shape

```json
{
  "analysis_path": "knowerage/analysis/auth/auth-login.md",
  "source_path": "src/legacy/AuthService.java",
  "covered_ranges": [[120,165],[210,248]],
  "analysis_hash": "sha256:...",
  "source_hash": "sha256:...",
  "record_created_at": "2026-03-01T10:26:00Z",
  "record_updated_at": "2026-03-01T10:30:12Z",
  "status": "fresh"
}
```

### Status vocabulary (locked)

| Status        | Meaning                                                |
|---------------|--------------------------------------------------------|
| `fresh`       | Doc and source hashes match record; ranges valid       |
| `stale_doc`   | Analysis doc changed since last record                  |
| `stale_src`   | Source file changed since last record                   |
| `missing_src`  | Source file no longer exists                            |
| `dangling_doc`| Doc exists but record invalid/deleted/malformed         |

---

## 3. MCP Tool List (from Task 03)

| Tool                         | Purpose                                      |
|------------------------------|----------------------------------------------|
| `knowerage.create_or_update_doc`| Create/update analysis document with metadata |
| `knowerage.parse_doc_metadata`  | Parse frontmatter + validate coverage        |
| `knowerage.reconcile_record`  | Reconcile one analysis record                |
| `knowerage.reconcile_all`     | Full rescan/rebuild for all analysis files   |
| `knowerage.get_file_status`   | Analyzed vs missing ranges for one source    |
| `knowerage.list_stale`        | List stale/problematic records               |
| `knowerage.get_tree`          | Tree/grouped coverage for UI                 |
| `registry.export_report`      | Export snapshot (JSON/YAML/TXT/HTML)         |

### `knowerage.get_file_status` — Output schema

**Input**: `{ "source_path": "src/legacy/AuthService.java" }`

**Output**:

```json
{
  "ok": true,
  "source_path": "src/legacy/AuthService.java",
  "line_count": 410,
  "analyzed_ranges": [[120,165],[210,248]],
  "missing_ranges": [[1,119],[166,209],[249,410]],
  "coverage_percent": 20.98,
  "range_attribution": [
    { "range": [120, 165], "analysis_paths": ["knowerage/analysis/auth/auth-login.md"] },
    { "range": [210, 248], "analysis_paths": ["knowerage/analysis/auth/auth-login.md", "knowerage/analysis/auth/auth-logout.md"] }
  ]
}
```

- `range_attribution` (optional): For each merged range in `analyzed_ranges`, lists which analysis files contribute to that range. When multiple analyses cover overlapping lines, the merged range appears with all contributing `analysis_paths`. Implementations may omit this field if not yet supported; UI should degrade gracefully.

---

## 4. Error Codes (from Task 03)

| Code             | Meaning                              |
|------------------|--------------------------------------|
| `E_DOC_PARSE`    | Failed to parse analysis frontmatter |
| `E_RANGE_INVALID`| Invalid or malformed line range      |
| `E_SRC_MISSING`  | Source file not found                |
| `E_PATH_TRAVERSAL`| Path outside workspace root         |
| `E_REGISTRY_IO`  | Registry read/write failure          |

---

## 5. Path & Security Rules

### Registry and analysis locations (locked)

| Item | Path |
|------|------|
| `knowerage` folder | Root of the project being documented/analysed (workspace root) |
| Registry file | `knowerage/registry.json` |
| Analysis markdown files | `knowerage/analysis/**/*.md` |

- The `knowerage` folder must be at the root of the project being documented/analysed.
- All paths must be under workspace root.
- Reject `..` and absolute paths outside root.
- Atomic writes for registry: write to temp file, then rename.
- Single-writer ownership for registry updates.

### Workspace Root Resolution

The MCP server resolves workspace root in this order:

1. **MCP roots** — If the host supports `roots` capability, call `roots/list` and use the first root URI (convert `file://` to path).
2. **KNOWERAGE_WORKSPACE_ROOT** — Environment variable override.
3. **Fallback** — `process.cwd()` (Node) or `std::env::current_dir()` (Rust) / `./`.

**Cursor / VS Code**: Use project-level `.cursor/mcp.json` or `.vscode/mcp.json` with `"env": { "KNOWERAGE_WORKSPACE_ROOT": "${workspaceFolder}" }` so the MCP workspace matches the IDE's open project root.

---

## Acceptance Criteria

- [ ] All schemas are machine-parseable (JSON Schema or equivalent).
- [ ] Status transitions are documented.
- [ ] MCP tools have request/response examples.
- [ ] Error codes are exhaustive for MVP scope.
- [ ] Document is versioned and referenced by all Phase 1/2 tasks.
