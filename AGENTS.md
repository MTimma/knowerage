# Agent instructions — Knowerage

This repository ships the Knowerage MCP server. When Knowerage MCP tools are available in your session, use them for any task that analyzes, documents, or explains legacy or source files in this workspace. Do not bypass Knowerage by writing `knowerage/analysis/**/*.md` with generic file-edit tools only, and never hand-edit `knowerage/registry.json`.

When analyzing legacy code (Java, XML, or other source files):

1. **Create analysis** — `knowerage.create_or_update_doc` with YAML frontmatter (`source_file`, `covered_lines`, `analysis_date`).
2. **Reconcile** — `knowerage.reconcile_record` after every create or update to that analysis file (required).
3. **Coverage** — Reconcile updates `knowerage/registry.json`; no manual registry edits.

For a full registry snapshot (same shape as `registry.json`), use `knowerage.list_registry` instead of opening the file by hand. **“Analyze X”** means the full workflow above; the user does not need to say “record coverage” or “use the MCP.”

More detail: [INSTRUCTIONS.md](INSTRUCTIONS.md), [contracts/contracts.md](contracts/contracts.md).
