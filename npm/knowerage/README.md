<div align="center">
  <img src="https://raw.githubusercontent.com/MTimma/knowerage/main/docs/assets/icon.png" alt="Knowerage project icon" width="128" />
</div>

# Knowerage — AI Analysis Coverage Management

Local MCP server that tracks legacy code analysis coverage and freshness.

**Repository:** [github.com/MTimma/knowerage](https://github.com/MTimma/knowerage)

## Quick start

**Requirements:** Node.js **18 or newer** — `npx` must be on your `PATH` (it comes with npm, which is included with Node).

## MCP server configuration

Register Knowerage wherever your MCP host expects server definitions (for example some clients use `.cursor/mcp.json` or `.vscode/mcp.json`; others use environment variables or a UI—follow your host’s documentation). Use the same server entry shape:

```json
{
  "mcpServers": {
    "knowerage": {
      "command": "npx",
      "args": ["-y","@mtimma/knowerage"],
      "env": {
        "KNOWERAGE_WORKSPACE_ROOT": "${workspaceFolder}",
        "KNOWERAGE_AUTO_FULL_RECONCILE": "true"
      }
    }
  }
}
```

Replace `${workspaceFolder}` with your project root if your host does not expand that variable.

`KNOWERAGE_AUTO_FULL_RECONCILE` is optional: when **unset**, **empty**, or not a truthy value, the file watcher defaults to **off**. Set to `1`, `true`, `yes`, or `on` (trimmed, case-insensitive) to enable. When **on**, the server watches `knowerage/` and, after a short debounce, runs `knowerage_reconcile_all` on filesystem changes. That is **not** the same as running a full reconcile after every MCP tool call—it only reacts to file changes under `knowerage/`. Registry writes to `registry.json` are ignored by the watcher so saves do not loop.

## How to use Knowerage

After the MCP server is configured, you talk to your assistant in normal sentences. You do not need to memorize tool names.

### Analyse or document code

Point at files, classes, or behaviour you care about. For example:

- *Using Knowerage, Analyze the data entity reconciliation and versioning logic in the ETL service.*

The assistant creates or updates markdown under `knowerage/analysis/` and records coverage in `knowerage/registry.json` (see **How it works** below).

### Coverage and gaps

When you already have Knowerage generated documentation in the repository, you can ask another agent:

- *In percentage, how much of the code has our analysis covered?*
- *Which parts of the codebase is not yet analysed?*

Knowerage answers these from the registry and coverage helpers (for example overview, per-file status, and stale lists) 
Agent does not need to spend tokens to go over the whole codebase the second time and compare to the analysis docs

## Alternative approaches

### Install via npm

```
npx @mtimma/knowerage
```

### Or build from source

See the [main README](https://github.com/MTimma/knowerage#readme) on GitHub.

## How it works

1. AI agent creates analysis `.md` files with YAML frontmatter declaring source file and covered line ranges
2. Registry (`knowerage/registry.json`) tracks analysis records with SHA-256 hashes for freshness
3. MCP tools expose create, reconcile, query, and export operations
4. Agent says "analyze X" → full workflow runs automatically (create → reconcile → record)

## Documentation (full project)

On npm you only get this wrapper; the full docs, contracts, and examples live in the repo:

- [User onboarding](https://github.com/MTimma/knowerage/blob/main/docs/USER_ONBOARDING.md)
- [INSTRUCTIONS.md](https://github.com/MTimma/knowerage/blob/main/INSTRUCTIONS.md) — MCP agent instructions
- [Contracts](https://github.com/MTimma/knowerage/blob/main/contracts/contracts.md) — Schemas and API contracts
- [Example `registry.json`](https://github.com/MTimma/knowerage/blob/main/examples/registry.sample.json)

## MCP tools (overview)

| Tool | Purpose |
| --- | --- |
| `knowerage_create_or_update_doc` | Create/update analysis document |
| `knowerage_parse_doc_metadata` | Parse and validate frontmatter |
| `knowerage_reconcile_record` | Reconcile one analysis record |
| `knowerage_reconcile_all` | Full rescan/rebuild |
| `knowerage_get_file_status` | Analyzed vs missing ranges |
| `knowerage_list_stale` | List stale/problematic records |
| `knowerage_list_registry` | Full registry snapshot |
| `knowerage_get_tree` | Tree/grouped coverage |
| `registry_export_report` | Export snapshot (JSON/YAML/TXT/HTML) |
| `knowerage_generate_bundle` | Chunked export of selected analyses |

## Security

- All paths validated against workspace root
- Path traversal (`..`) rejected
- Atomic writes for registry (crash-safe)
- No secrets in analysis files or reports
- SHA-256 hash-based freshness (survives git pull)

## License

[MIT](https://github.com/MTimma/knowerage/blob/main/LICENSE) — copyright Martins Timma.

Parts of this project were written or refined with **generative AI coding assistants**. Human review applies to design, security-sensitive behavior, and releases.
