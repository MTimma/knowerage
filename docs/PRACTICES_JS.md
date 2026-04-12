# JavaScript / Node.js Best Practices — Knowerage

Practical guidelines for code style, testing, and security. Prioritizes **usability** and **ease of setup**. Avoid over-engineering.

---

## 1. Code Style

| Rule | Why |
|------|-----|
| Use `prettier` with default config | Consistent formatting |
| Use `eslint` with recommended rules | Basic linting without noise |
| Prefer `async/await` over raw Promises | Readable async flow |
| Use `path.join()` for paths, never string concat | Cross-platform |
| Validate env vars at startup | Fail fast with clear message |

**Setup**: Minimal `.eslintrc` and `.prettierrc` (or rely on defaults).

---

## 2. Testing

| Rule | Why |
|------|-----|
| Use `node:test` (built-in) or `vitest` | Minimal setup, fast |
| One test file per source file | Easy mapping |
| Mock file system with `memfs` or temp dirs | Isolated tests |
| Test path validation and error responses | Security baseline |

**Run**: `npm test` or `npx vitest run`.

---

## 3. Security (Critical)

### Path Safety

| Rule | Implementation |
|------|----------------|
| All paths must resolve under workspace root | `path.resolve(root, input)` then check `result.startsWith(root)` |
| Reject `..` in path segments | Split and check each segment |
| Reject absolute paths outside root | Compare resolved path to root |
| Use canonical paths before comparison | Avoid symlink tricks |

**Error**: Return `E_PATH_TRAVERSAL` and stop; never proceed.

### File Writes

- Atomic writes for registry/reports: write to `*.tmp`, then `fs.renameSync`
- Single writer for `knowerage/registry.json` — one process owns writes
- No secrets in MD, registry, or reports — validate/sanitize before write

### Input Validation

- Validate all MCP tool params — reject unknown/invalid fields per contract
- Sanitize strings before file writes — no control chars, no path injection
- Limit string lengths (e.g. 4KB for paths) — prevent DoS

---

## 4. Error Handling

- Return `{ ok: false, error_code: "...", message: "..." }` per API contract
- Never expose stack traces or internal paths to MCP client

---

## 5. Usability & Setup

| Goal | Approach |
|------|----------|
| One-command run | `npx knowerage-mcp` |
| Registry location | `knowerage/registry.json` at root of project being documented/analysed |
| Clear error when misconfigured | "Workspace root not found: X" |
| Pin versions | Keep `package-lock.json` committed |

### Workspace Root Resolution

Resolve workspace root in this order (per Phase 0):

1. **MCP roots** — If the host supports `roots` capability, call `roots/list` and use the first root URI (convert `file://` to path).
2. **KNOWERAGE_WORKSPACE_ROOT** — Environment variable override.
3. **Fallback** — `process.cwd()`.

**Cursor / VS Code**: Use project-level `.cursor/mcp.json` or `.vscode/mcp.json` with `"env": { "KNOWERAGE_WORKSPACE_ROOT": "${workspaceFolder}" }` so the MCP workspace matches the IDE's open project root.

### Logging

- Use `console` or `pino` — control via `LOG_LEVEL`
- Never log secrets, tokens, or full file contents

---

## 6. What to Avoid

- **No** custom DSLs or config formats — use JSON/YAML
- **No** database for MVP — `knowerage/registry.json` is enough
- **No** complex tree structures until UI needs them — flat list first
- **No** extra test frameworks — `node:test` or `vitest` is enough

---

## 7. Checklist Before Merge

- [ ] `npm test` passes
- [ ] `eslint` passes
- [ ] Path params validated against workspace root
- [ ] No secrets in logs or exported files
- [ ] Atomic writes for registry/report
- [ ] Error codes match contract

---

## References

- Phase 0: `agent_tasks/phase_0_contracts.md`
- Phase 2c Security: `agent_tasks/phase_2c_security.md`
