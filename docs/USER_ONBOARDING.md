# Knowerage — User Onboarding

This document covers one-time setup, agent guidance, and "works out of the box" configuration so the AI agent knows how to use Knowerage without explicit prompting.

---

## 1. One-Time Setup

### 1.1 Install the MCP Server

- Install the Knowerage MCP server via `npx knowerage-mcp` or build from source (`cargo build --release`). See [README](../README.md) for details.
- Add it to your Cursor MCP configuration (see §2).

### 1.2 Workspace Root

Knowerage needs a clear workspace root. The MCP server resolves it in this order:

1. **MCP roots** — If the host supports `roots` capability, use the first root.
2. **`KNOWERAGE_WORKSPACE_ROOT`** — Environment variable override (recommended for Cursor).
3. **Fallback** — Current working directory.

**Important**: If the workspace root is invalid or missing, the MCP server should return a clear error (e.g. `E_WORKSPACE_ROOT_INVALID`) instead of silently using the wrong directory.

Set `KNOWERAGE_WORKSPACE_ROOT` in your MCP config so it matches your IDE's open project root (see §2).

### 1.2.1 Auto full reconcile (file watcher)

**`KNOWERAGE_AUTO_FULL_RECONCILE`** — When **on** (default if unset or empty), the server watches `knowerage/` and runs a **debounced** `reconcile_all` after changes. This keeps the registry aligned when analysis files or related paths change outside a single MCP session. It does **not** mean “full reconcile after every MCP tool”; it only runs when the filesystem notifies of changes (excluding `registry.json` / temp registry files).

- **Default:** on (`true`) when the variable is unset or blank.
- **Disable:** set to `false`, `0`, or `no` (case-insensitive) in the MCP server `env` block.

### 1.3 First-Run Bootstrap

On first use, the `knowerage/` folder and `registry.json` may not exist. The agent should:

- Call `knowerage.reconcile_all` when `get_file_status` or `get_tree` returns an empty registry — this bootstraps the registry from any existing analysis files.
- Alternatively, the MCP server may auto-run `reconcile_all` on first `get_file_status` or `get_tree` when the registry is empty (implementation choice).

---

## 2. Cursor MCP Config (Copy-Paste)

Add this to your Cursor MCP configuration (e.g. `.cursor/mcp.json` or Cursor Settings → MCP):

```json
{
  "mcpServers": {
    "knowerage": {
      "command": "npx",
      "args": ["knowerage-mcp"],
      "env": {
        "KNOWERAGE_WORKSPACE_ROOT": "${workspaceFolder}",
        "KNOWERAGE_AUTO_FULL_RECONCILE": "true"
      }
    }
  }
}
```

Optional: omit `KNOWERAGE_AUTO_FULL_RECONCILE` to keep the default (watcher on), or set `"false"` to disable background full reconciles.

For Rust binary:

```json
{
  "mcpServers": {
    "knowerage": {
      "command": "/path/to/knowerage-mcp",
      "args": [],
      "env": {
        "KNOWERAGE_WORKSPACE_ROOT": "${workspaceFolder}",
        "KNOWERAGE_AUTO_FULL_RECONCILE": "true"
      }
    }
  }
}
```

Replace `"${workspaceFolder}"` with the actual path if your host does not expand it.

---

## 3. Analysis Folder Convention

| Item | Path |
|------|------|
| Analysis markdown files | `knowerage/analysis/**/*.md` |
| Registry file | `knowerage/registry.json` |

The `knowerage.create_or_update_doc` tool uses `analysis_path` — default convention is `knowerage/analysis/{topic}/{name}.md` (e.g. `knowerage/analysis/auth/auth-login.md`).

---

## 4. Suggested Cursor Rules (Agent Guidance)

Why agents sometimes skip MCP: models often default to built-in read/write tools. Mitigations shipped with Knowerage: (1) server `initialize` **instructions** and stronger **tool descriptions** so the model sees “use this when analyzing”; (2) an **always-on project rule** (below); (3) optionally the same text in **Cursor User Rules** for repositories that do not contain `.cursor/rules/`.

Add a project rule so the agent knows to record coverage when analyzing legacy code, without the user having to say "record coverage" or "use Knowerage" every time.

### Option A: Cursor Rules (`.cursor/rules/`)

Create `.cursor/rules/knowerage-analysis.mdc`:

```markdown
---
description: Knowerage legacy code analysis workflow
alwaysApply: true
---

# Knowerage Analysis Workflow

When analyzing legacy code (Java, XML, or other source files):

1. **Create analysis** — Use `knowerage.create_or_update_doc` to create an analysis document with YAML frontmatter (`source_file`, `covered_lines`, `analysis_date`).
2. **Reconcile** — Call `knowerage.reconcile_record` after creating or updating an analysis file.
3. **Record coverage** — `knowerage.reconcile_record` updates `knowerage/registry.json` with the analysis record (no manual append needed).

"Analyze X" implies full workflow: create analysis, reconcile, and record coverage. No need for the user to say "record coverage" explicitly.
```

### Option B: User Rules (Cursor Settings)

If you prefer user-level rules, add the same content to your Cursor User Rules.

---

## 5. Typical Prompts (No Extra "Record Coverage" Needed)

With the suggested rule, users can say:

| User says | Agent infers |
|-----------|--------------|
| "Analyze src/legacy/AuthService.java" | Create analysis MD, reconcile (updates registry.json) |
| "Document the login flow in AuthService" | Same as above |
| "Review src/legacy/ConfigParser.xml" | Same as above |

Users do **not** need to say "and record coverage" — the agent knows the Knowerage workflow.

---

## 6. When to Reconcile

| Scenario | Tool |
|----------|------|
| After creating/updating one analysis file | `knowerage.reconcile_record` |
| After git pull, bulk edits, or to refresh registry | `knowerage.reconcile_all` |
| First run, empty registry | `knowerage.reconcile_all` (bootstrap) |

---

## 7. Troubleshooting

| Issue | Check |
|-------|-------|
| Wrong directory / paths | `KNOWERAGE_WORKSPACE_ROOT` set correctly in MCP config |
| Empty registry on first use | Run `knowerage.reconcile_all` or ensure first-run bootstrap |
| Agent doesn't record coverage | Add suggested Cursor rule (§4) |
| MCP not found | Verify MCP config path and command |

---

## 8. Related Docs

- [INSTRUCTIONS.md](../INSTRUCTIONS.md) — MCP server instructions for the agent (tool reference, workflow).
- [contracts/contracts.md](../contracts/contracts.md) — Schemas, paths, error codes.
- [docs/PRACTICES_RUST.md](PRACTICES_RUST.md), [docs/PRACTICES_JS.md](PRACTICES_JS.md) — Implementation practices.
