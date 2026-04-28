<div align="center">
  <img src="docs/assets/icon.png" alt="Knowerage project icon" width="128" />
</div>

# Knowerage — AI Analysis Coverage Management

Local-first MCP server that tracks legacy code analysis coverage and freshness.

**Source:** [github.com/MTimma/knowerage](https://github.com/MTimma/knowerage)

## Quick Start

**Requirements:** Node.js **18 or newer** — `npx` must be on your `PATH` (it comes with npm, which is included with Node).

## MCP server configuration

Register Knowerage wherever your MCP host expects server definitions (for example some clients use `.cursor/mcp.json` or `.vscode/mcp.json`; others use environment variables or a UI—follow your host’s documentation). Use the same server entry shape:

```json
{
  "mcpServers": {
    "knowerage": {
      "command": "npx",
      "args": ["@mtimma/knowerage"],
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

## How to use Knowerage (plain language)

After the MCP server is configured, you talk to your assistant in normal sentences. You do not need to memorize tool names.

### Analyse or document code

Point at files, classes, or behaviour you care about. For example:

- *Using Knowerage, analyse the logical algorithm workflow in `main.java`.*
- *Analyze the data entity reconciliation and versioning logic in the ETL service.*

The assistant creates or updates markdown under `knowerage/analysis/` and records coverage in `knowerage/registry.json` (see **How It Works** below).

### Coverage and gaps (same project, later chat or another agent)

When you already have analyses in the tree, you can ask:

- *In percentage, how much of the code has our analysis covered?*
- *What part of this codebase is not yet analysed?*

Knowerage answers these from the registry and coverage helpers (for example overview, per-file status, and stale lists)—not from hand-waving over the repo.

## Alternative approaches
### Install via npm
```
npx @mtimma/knowerage
```

### Or build from source
```
cargo build --release
./target/release/knowerage-mcp
```


## How It Works

1. AI agent creates analysis `.md` files with YAML frontmatter declaring source file and covered line ranges
2. Registry (`knowerage/registry.json`) tracks analysis records with SHA-256 hashes for freshness
3. MCP tools expose create, reconcile, query, and export operations
4. Agent says "analyze X" → full workflow runs automatically (create → reconcile → record)

### Registry file shape (`knowerage/registry.json`)

The on-disk format is a **JSON object** whose keys are analysis paths (strings). Each value is one **record** (see [contracts/contracts.md](contracts/contracts.md)). A full sample with two records lives at [examples/registry.sample.json](examples/registry.sample.json).

```mermaid
flowchart TB
  subgraph file["knowerage/registry.json"]
    O["Top-level JSON object"]
    O --> K["Each key: analysis markdown path, e.g. knowerage/analysis/.../topic.md"]
    K --> V["Value: one RegistryRecord"]
  end

  subgraph rec["RegistryRecord fields"]
    ap["analysis_path · source_path"]
    cr["covered_ranges: [[start,end], ...]"]
    h["analysis_hash · source_hash (sha256:… )"]
    t["record_created_at · record_updated_at (ISO 8601)"]
    st["status: fresh | stale_doc | stale_src | missing_src | dangling_doc"]
  end

  V --> rec
```

Frontmatter for analysis `.md` files is specified separately in the contracts doc (**metadata schema**), not inside `registry.json`.

## MCP Tools

| Tool | Purpose |
|------|---------|
| `knowerage_create_or_update_doc` | Create/update analysis document |
| `knowerage_parse_doc_metadata` | Parse and validate frontmatter |
| `knowerage_reconcile_record` | Reconcile one analysis record |
| `knowerage_reconcile_all` | Full rescan/rebuild |
| `knowerage_get_file_status` | Analyzed vs missing ranges |
| `knowerage_list_stale` | List stale/problematic records |
| `knowerage_list_registry` | Full registry snapshot (same shape as `registry.json`, sorted keys) |
| `knowerage_get_tree` | Tree/grouped coverage |
| `registry_export_report` | Export snapshot (JSON/YAML/TXT/HTML) |
| `knowerage_generate_bundle` | Chunked export of selected analyses (`toc*.md`, `combined*.md`, `manifest.json`) |

## Project Structure

```
knowerage/                  # Created per-project
├── analysis/              # Analysis markdown files
│   └── **/*.md
└── registry.json          # Coverage registry

src/                       # Rust MCP server
├── main.rs
├── lib.rs
├── types.rs
├── parser.rs
├── registry.rs
├── mcp.rs
├── security.rs
└── export.rs
```

## Documentation

- [User Onboarding](docs/USER_ONBOARDING.md) — Setup, config, typical usage
- [INSTRUCTIONS.md](INSTRUCTIONS.md) — MCP agent instructions
- [Rust Practices](docs/PRACTICES_RUST.md)
- [JS Practices](docs/PRACTICES_JS.md)
- [Contracts](contracts/contracts.md) — Schemas and API contracts (registry + frontmatter)
- [Example registry JSON](examples/registry.sample.json) — Sample `registry.json` contents

## Security

- All paths validated against workspace root
- Path traversal (`..`) rejected
- Atomic writes for registry (crash-safe)
- No secrets in analysis files or reports
- SHA-256 hash-based freshness (survives git pull)

## License

[MIT](LICENSE) — copyright Martins Timma.

Parts of this project were written or refined with **generative AI coding assistants**. Human review applies to design, security-sensitive behavior, and releases.
