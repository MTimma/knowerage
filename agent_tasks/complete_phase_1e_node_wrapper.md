# Phase 1e - Node Wrapper & npm Package (Agent H)

**Depends on**: Phase 0 (Contracts), Phase 1c (MCP)  
**Stack**: Node.js (thin wrapper), npm (package + optionalDependencies)

---

## Objective

Provide an npm package that installs the Knowerage MCP server via `npx @mtimma/knowerage-mcp`. The Node wrapper spawns the Rust binary; binaries are distributed as platform-specific optionalDependencies under the `@mtimma` scope (no postinstall downloads from external URLs).

---

## Deliverables

- Thin Node.js wrapper that locates and spawns the Rust binary
- npm package `@mtimma/knowerage-mcp` with `bin` entry
- Platform-specific packages via `optionalDependencies` (see table below)
- `mcpName` in package.json for MCP Registry compatibility
- Clear error when binary for current platform is missing

---

## Package Structure

```
knowerage-mcp/                    # Main package (wrapper)
├── package.json
├── index.js                      # Entry: spawn binary, forward stdio
├── bin/
│   └── knowerage-mcp             # Symlink or script to index.js
└── optionalDependencies:
    ├── knowerage-mcp-darwin-arm64
    ├── knowerage-mcp-darwin-x64
    ├── knowerage-mcp-linux-x64
    ├── knowerage-mcp-linux-arm64-gnu
    ├── knowerage-mcp-win32-x64
    └── knowerage-mcp-win32-arm64  # optional, add when needed
```

---

## Platform Packages (optionalDependencies)

| Package name | Target | OS | Arch |
|-------------|--------|-----|------|
| `knowerage-mcp-darwin-arm64` | `aarch64-apple-darwin` | macOS | Apple Silicon |
| `knowerage-mcp-darwin-x64` | `x86_64-apple-darwin` | macOS | Intel |
| `knowerage-mcp-linux-x64` | `x86_64-unknown-linux-gnu` | Linux | x64 |
| `knowerage-mcp-linux-arm64-gnu` | `aarch64-unknown-linux-gnu` | Linux | ARM64 |
| `knowerage-mcp-win32-x64` | `x86_64-pc-windows-msvc` | Windows | x64 |
| `knowerage-mcp-win32-arm64` | `aarch64-pc-windows-msvc` | Windows | ARM64 |

Each platform package contains only the binary (e.g. `knowerage-mcp` or `knowerage-mcp.exe`) and a minimal `package.json` with `bin` pointing to it.

---

## Wrapper Logic

1. Resolve platform: `process.platform` (darwin, linux, win32) + `process.arch` (arm64, x64)
2. Require the matching platform package (e.g. `require('@mtimma/knowerage-mcp-darwin-arm64')`)
3. Get binary path from the package's `bin` or exported path
4. Spawn: `child_process.spawn(binaryPath, process.argv.slice(2), { stdio: 'inherit', shell: false })`
5. Forward exit code: `process.exit(child.exitCode ?? 1)`
6. If platform package is missing: print clear error and exit 1

---

## Cross-Platform Builds

**Can we build for Windows, Mac, Linux from the same OS?**

| Approach | Feasibility | Notes |
|----------|-------------|-------|
| **Pure Rust, no C deps** | Yes | `rustup target add` + `cargo build --target` |
| **cross (Docker)** | Linux↔Windows, Linux↔Linux | Works well; no macOS target |
| **macOS from Linux** | Possible but complex | Requires osxcross + macOS SDK (licensing) |
| **GitHub Actions (recommended)** | Yes | Native runners: `ubuntu-latest`, `macos-latest`, `windows-latest` — each builds for its own platform. No cross-compilation needed. |

**Recommended**: Use GitHub Actions matrix strategy. Each runner builds the Rust binary natively for its OS, then publishes the corresponding platform package. Simple, reliable, no cross-compilation toolchain.

---

## Security Requirements

- Use `spawn()` with `shell: false` — no shell injection
- Do not pass unsanitized user input as spawn arguments (MCP host controls argv)
- No postinstall scripts that download from external URLs (S3, CDN)
- Binaries must be bundled inside npm packages (npm registry integrity)

---

## package.json (Main Wrapper)

```json
{
  "name": "knowerage-mcp",
  "version": "0.1.0",
  "description": "MCP server for AI analysis coverage management",
  "bin": { "knowerage-mcp": "./bin/knowerage-mcp.js" },
  "mcpName": "io.github.USERNAME/knowerage",
  "optionalDependencies": {
    "knowerage-mcp-darwin-arm64": "0.1.0",
    "knowerage-mcp-darwin-x64": "0.1.0",
    "knowerage-mcp-linux-x64": "0.1.0",
    "knowerage-mcp-linux-arm64-gnu": "0.1.0",
    "knowerage-mcp-win32-x64": "0.1.0"
  },
  "engines": { "node": ">=18" }
}
```

---

## Unit Tests (Expected Spec)

| # | Test | Input | Expected |
|---|------|-------|----------|
| 1 | Platform resolution | `darwin` + `arm64` | Resolves to `knowerage-mcp-darwin-arm64` |
| 2 | Missing platform package | Unsupported platform | Clear error message, exit 1 |
| 3 | Spawn uses shell: false | Any | No shell invoked |
| 4 | Exit code forwarded | Child exits 0 | Wrapper exits 0 |
| 5 | Exit code forwarded | Child exits 1 | Wrapper exits 1 |

---

## CI / Publish Workflow

1. GitHub Actions: matrix `[ubuntu, macos, windows]`
2. Each job: build Rust binary for that OS, pack into platform package
3. Publish platform packages to npm (version in sync with main package)
4. Publish main `knowerage-mcp` package (depends on platform packages)

---

## Acceptance Criteria

- [ ] `npx @mtimma/knowerage-mcp` runs the Rust MCP server on supported platforms
- [ ] No postinstall downloads from external URLs
- [ ] `mcpName` present for MCP Registry
- [ ] Clear error when platform unsupported or binary missing
- [ ] All 5 unit tests pass
- [ ] GitHub Actions builds and publishes all platform packages
