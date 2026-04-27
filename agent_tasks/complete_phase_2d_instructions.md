# Phase 1d - MCP Agent Instructions & Documentation

**Depends on**: Phase 0 (Contracts), Phase 1c (MCP Server)  
**Stack**: Documentation (Markdown)

---

## Objective

Produce `INSTRUCTIONS.md` for the Knowerage MCP server so agents and users know when and how to call tools, discover analysis files, and perform full registry recalculation. This file is consumed by the MCP host and provides context to the AI agent.

---

## Deliverables

- `INSTRUCTIONS.md` — placed alongside the MCP server (e.g. `mcps/knowerage/INSTRUCTIONS.md` or in the server package)
- Content covers: agent workflow, tool selection, analysis discovery, reconcile_all use case, and MCP resources

---

## INSTRUCTIONS.md Specification

The following content MUST be generated as `INSTRUCTIONS.md`. Implementers may adjust formatting or add sections as needed, but all specified sections must be present.

---

### 1. Header & Purpose

```markdown
# Knowerage MCP — AI Analysis Coverage Management

Use this MCP server when analyzing legacy code, tracking which source lines are documented, and reconciling analysis freshness. The agent creates `.md` analysis files with coverage metadata and queries the registry for gaps and stale records.
```

---

### 2. Agent Workflow (Getting Started)

```markdown
## Agent Workflow

### Typical flow

1. **Create analysis** → `knowerage_create_or_update_doc` — Create or update an analysis document with frontmatter (source path, covered line ranges, timestamp).
2. **Reconcile** → `knowerage_reconcile_record` — Reconcile the single analysis record into the registry (call after creating/updating one file).
3. **Query coverage** → `knowerage_get_file_status`, `knowerage_list_stale`, or `knowerage_get_tree` — Inspect analyzed vs missing ranges, stale records, or tree view.

### When to call reconcile_record vs reconcile_all

| Use case | Tool | When |
|----------|------|------|
| **Single file** | `knowerage_reconcile_record` | Right after creating or updating one analysis file. |
| **Full recalculation** | `knowerage_reconcile_all` | When you need to rescan all analysis files and refresh the registry (e.g. after git pull, bulk edits, or to check if source code changed and analysis is stale). |

### Implicit workflow (analyze = full workflow)

When the user asks to **analyze** or **document** a legacy source file, the agent should perform the full workflow without the user having to say "record coverage":

1. Create analysis with `knowerage_create_or_update_doc` (source path, covered line ranges, timestamp).
2. Reconcile with `knowerage_reconcile_record`.
3. Append to `knowerage/registry.txt` in format: `{source_file}|{analysis_md_path}|{lines_from-to}` (e.g. `src/legacy/AuthService.java|knowerage/analysis/auth/auth-login.md|120-165,210-248`).

The user does not need to say "record coverage" — the agent infers the full workflow from "analyze X" or "document X".
```

---

### 3. Reconcile All Use Case (Full Recalculation)

```markdown
## Reconcile All Use Case

**Scenario**: Source code has been updated (e.g. git pull, refactor, merge) and you need to verify which analyses are now stale.

**Goal**: Recalculate the entire registry to detect:
- `stale_src` — source file changed since last record
- `stale_doc` — analysis doc changed
- `missing_src` — source file deleted
- `dangling_doc` — doc exists but record invalid

**How to run**:

1. Call `knowerage_reconcile_all` with:
   - `analysis_glob`: `knowerage/analysis/**/*.md` (default)
   - `delete_orphan_records`: `true` (optional, to clean up)

2. Inspect the returned `summary` for counts per status.

3. Optionally call `knowerage_list_stale` to get the list of problematic records for remediation.

**Example request**:
```json
{
  "analysis_glob": "knowerage/analysis/**/*.md",
  "delete_orphan_records": true
}
```

**Example response**:
```json
{
  "ok": true,
  "summary": {
    "total": 182,
    "fresh": 140,
    "stale_md": 12,
    "stale_src": 24,
    "missing_src": 3,
    "orphan_md": 3
  }
}
```
```

---

### 4. Discovering Analysis Files

```markdown
## Discovering Analysis Files

- **Glob pattern**: `knowerage/analysis/**/*.md`
- **Location**: Under workspace root, in `knowerage/analysis/`.
- **Validation**: Use `knowerage_parse_doc_metadata` to parse and validate an analysis file's frontmatter before or after creating it.
- **Full discovery**: `knowerage_reconcile_all` discovers and reconciles all files matching the glob.
```

---

### 5. MCP resources

```markdown
## MCP resources

The current server exposes **tools only** (no `resources/list` or `resources/read`). Use `knowerage_list_registry`, `knowerage_get_file_status`, `knowerage_list_stale`, `knowerage_get_tree`, and `knowerage_coverage_overview` for discovery and coverage data.
```

---

### 6. Tool Quick Reference

```markdown
## Tool Quick Reference

| Tool | Purpose |
|------|---------|
| `knowerage_create_or_update_doc` | Create/update analysis document with metadata |
| `knowerage_parse_doc_metadata` | Parse frontmatter + validate coverage |
| `knowerage_reconcile_record` | Reconcile one analysis record |
| `knowerage_reconcile_all` | Full rescan/rebuild |
| `knowerage_get_file_status` | Analyzed vs missing ranges for one source |
| `knowerage_list_stale` | List stale/problematic records |
| `knowerage_list_registry` | Full registry snapshot (same shape as `registry.json`; sorted keys) |
| `knowerage_get_tree` | Tree/grouped coverage for UI |
| `knowerage_coverage_overview` | Batch overview: per-source coverage, project totals, stale list |
| `registry_export_report` | Export snapshot (JSON/YAML/TXT/HTML) |
| `knowerage_generate_bundle` | Chunked export of selected analyses (`toc*.md`, `combined*.md`, `manifest.json`) |
```

---

### 7. Path & Security Notes

```markdown
## Path & Security

- All paths must be under workspace root.
- Path traversal (`..`) and absolute paths outside root are rejected with `E_PATH_TRAVERSAL`.
- Registry: `knowerage/registry.json`
- Analysis files: `knowerage/analysis/**/*.md`
```

---

## Implementation Steps

1. Create the target directory for the Knowerage MCP server (e.g. `mcps/knowerage/` or equivalent).
2. Generate `INSTRUCTIONS.md` with all sections above, merged into a single coherent document.
3. Ensure the MCP server configuration references this file (per host conventions).
4. Verify the document is discoverable by the agent (e.g. via MCP server metadata or host docs).

---

## Acceptance Criteria

- [ ] `INSTRUCTIONS.md` exists and contains all 7 sections
- [ ] Reconcile_all use case is clearly documented with `knowerage_reconcile_all`
- [ ] Agent workflow (create → reconcile → query) is explicit
- [ ] When to use `reconcile_record` vs `reconcile_all` is documented
- [ ] Glob pattern `knowerage/analysis/**/*.md` is specified
- [ ] MCP resources section is present (even if optional/planned)
- [ ] Tool quick reference matches Phase 0 contracts
