# Knowerage — Test Plan & Rollout

## Test Pyramid

| Level | Scope | Count | Runner |
|-------|-------|-------|--------|
| Unit | Parser, Registry, Security, MCP, Export | 45 | `cargo test` (lib) |
| Integration | Registry, Parser, MCP, Export | 23 | `cargo test` (tests/) |
| Unit (Node) | npm wrapper | 5 | `node --test` |
| **Total** | | **73** | |

## Quality Gates

All must pass before merge:

- [ ] `cargo test` — all 68 Rust tests pass
- [ ] `cargo clippy --all-targets -- -D warnings` — no warnings
- [ ] `cargo fmt -- --check` — formatted
- [ ] `node --test npm/knowerage-mcp/test.js` — 5 Node tests pass
- [ ] No path traversal/security bypass in integration tests
- [ ] Deterministic output for same inputs

## Test Coverage by Component

### Parser (18 tests)
- 11 unit tests: valid parsing, missing fields, malformed ranges, normalization
- 7 integration tests: real files, extra keys, edge cases, regression

### Registry (13 tests)
- 8 unit tests: freshness states, atomic write, reconcile-all, large file
- 5 integration tests: source changes, rebuild, mixed status, large repo

### Security (12 unit tests)
- Path traversal rejection, atomic write, concurrent lock, secret detection, sanitization

### MCP Server (14 tests)
- 8 unit tests: all 8 tool handlers, path validation, schema validation
- 6 integration tests: full flow, malformed input, tree view, export

### Export (11 tests)
- 6 unit tests: selection, dedup, manifest, timestamp, partial success, empty
- 5 integration tests: bulk export, structure, mixed input, traceability, determinism

### Node Wrapper (5 tests)
- Platform resolution, missing platform, shell:false, exit codes, platform coverage

## Phased Rollout

### Phase A: Core MCP Tools (MVP)
- Metadata parser + registry + reconcile CLI/MCP tools
- 8 MCP tools operational via stdio
- Acceptance: create → reconcile → query flow works end-to-end

### Phase B: Watcher + Stale Handling
- File watcher triggers reconcile on changes
- Full rebuild from analysis files
- Acceptance: git pull → stale records detected automatically

### Phase C: Coverage UI/Reports
- Export report (JSON/YAML/TXT/HTML)
- get_tree grouping for directory views
- Acceptance: reports generated, tree view populated

### Phase D: Export Bundle + npm Distribution
- NotebookLM export bundle (toc.md, combined.md, manifest)
- npm wrapper package with platform binaries
- Acceptance: `npx @mtimma/knowerage-mcp` runs the server

## Rollback Strategy

1. Registry is a single JSON file — backup before updates
2. Analysis markdown files are the source of truth — registry can always be rebuilt via `reconcile_all`
3. No database migrations — schema changes are additive
4. npm packages versioned — rollback by installing previous version
