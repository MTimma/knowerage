# Rust Best Practices — Knowerage

Practical guidelines for code style, testing, and security. Prioritizes **usability** and **ease of setup**. Avoid over-engineering.

---

## 1. Code Style

| Rule | Why |
|------|-----|
| Use `rustfmt` (default config) | Consistent style, zero config |
| Use `clippy` with default lints | Catches common mistakes |
| Prefer `Result<T, E>` over panics in library code | Caller decides how to handle errors |
| Use `thiserror` or simple enums for error types | Clear, matchable error codes |
| Keep functions under ~50 lines | Easier to test and reason about |
| Use `Path`/`PathBuf` for file paths, not `String` | Correct handling of separators |

**Setup** — Add to `Cargo.toml`:
```toml
[package]
edition = "2021"

[lints.rust]
unsafe_code = "forbid"
```

---

## 2. Testing

| Rule | Why |
|------|-----|
| Unit tests next to code (`#[cfg(test)] mod tests`) | Easy to find, run with `cargo test` |
| One test file per module when tests grow | Keeps `lib.rs` clean |
| Use `#[test]` + `assert!` / `assert_eq!` | No extra deps for MVP |
| Test error paths, not only happy path | Catches regressions |
| Use `tempfile` crate for file I/O tests | No manual cleanup |

**Critical test cases** (from project phases):

- Parser: valid frontmatter, missing keys, malformed ranges, overlap/merge
- Registry: hash match → fresh, hash mismatch → stale, atomic write, reconcile summary
- MCP: path traversal rejected, schema validation, success paths

**Run**: `cargo test` — no extra setup.

---

## 3. Security (Critical)

### Path Safety

- All paths must resolve under workspace root — canonicalize and check prefix
- Reject `..` in path segments
- Reject absolute paths outside root
- Use `std::path::Path::canonicalize` where appropriate; handle symlinks

**Error**: Return `E_PATH_TRAVERSAL` and stop; never proceed.

### File Writes

- Atomic writes for registry/reports: write to `*.tmp`, then `std::fs::rename`
- Single writer for `knowerage/registry.json` — one process owns writes
- No secrets in MD, registry, or reports — validate/sanitize before write

### Input Validation

- Validate all MCP tool params — reject unknown/invalid fields per contract
- Sanitize strings before file writes — no control chars, no path injection
- Limit string lengths (e.g. 4KB for paths) — prevent DoS

---

## 4. Error Handling

- Use contract error codes: `E_DOC_PARSE`, `E_RANGE_INVALID`, `E_SRC_MISSING`, `E_PATH_TRAVERSAL`, `E_REGISTRY_IO`
- Include short message for debugging
- Log errors server-side; return codes + minimal message to client

---

## 5. Usability & Setup

| Goal | Approach |
|------|----------|
| One-command run | `cargo run` |
| Registry location | `knowerage/registry.json` at root of project being documented/analysed; analysis in `knowerage/analysis/` |
| Clear error when misconfigured | "Workspace root not found: X" |
| Pin versions | Keep `Cargo.lock` committed |

### Workspace Root Resolution

Resolve workspace root in this order (per Phase 0):

1. **MCP roots** — If the host supports `roots` capability, call `roots/list` and use the first root URI (convert `file://` to path).
2. **KNOWERAGE_WORKSPACE_ROOT** — Environment variable override.
3. **Fallback** — `std::env::current_dir()` / `./`.

**Cursor / VS Code**: Use project-level `.cursor/mcp.json` or `.vscode/mcp.json` with `"env": { "KNOWERAGE_WORKSPACE_ROOT": "${workspaceFolder}" }` so the MCP workspace matches the IDE's open project root.

### Dependencies

- Minimize deps — faster build, fewer supply-chain risks
- Prefer std lib first — only add crate when std is insufficient

### Logging

- Use `env_logger` or `tracing` — control via `RUST_LOG`
- Never log secrets, tokens, or full file contents

---

## 6. What to Avoid

- **No** custom DSLs or config formats — use JSON/YAML
- **No** database for MVP — `knowerage/registry.json` is enough
- **No** complex tree structures until UI needs them — flat list first
- **No** Git dependency for MVP — hash-based freshness only
- **No** extra test frameworks — `cargo test` is enough

---

## 7. Checklist Before Merge

- [ ] `cargo test` passes
- [ ] `cargo clippy` passes
- [ ] Path params validated against workspace root
- [ ] No secrets in logs or exported files
- [ ] Atomic writes for registry/report
- [ ] Error codes match contract

---

## References

- Phase 0: `agent_tasks/phase_0_contracts.md`
- Phase 2c Security: `agent_tasks/phase_2c_security.md`
