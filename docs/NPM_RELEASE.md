# npm release (GitHub Actions)

You do **not** create the workflow in the GitHub UI. Workflows are ordinary files under [`.github/workflows/`](../.github/workflows/) in the repo; Actions runs them after you **push** (or merge) to GitHub.

## One-time setup (GitHub UI only for the secret)

1. On [npmjs.com](https://www.npmjs.com), create an **Access Token** with permission to publish packages (classic token type “Automation” or granular publish for your packages).
2. In the GitHub repo: **Settings → Secrets and variables → Actions → New repository secret**.
3. Name: **`NPM_TOKEN`**, value: the npm token.

Without `NPM_TOKEN`, the **Release** workflow still builds artifacts on tags, but the **publish** job fails at `npm publish`.

## How a release runs

The [Release workflow](../.github/workflows/release.yml) starts when **either**:

- You **push a tag** matching `v*` (e.g. `v1.0.0`), or  
- You open **Actions → Release → Run workflow** and run **`workflow_dispatch`** (pick the branch, usually `main`).

Then: six parallel **build** jobs (Linux x64/ARM64, macOS ARM64/x64, Windows x64/ARM64), then **publish** downloads those folders and runs `npm publish --access public` for each platform package and finally the main **`@mtimma/knowerage-mcp`** package (scoped packages require public access on first publish).

### Checklist before each run

1. Bump **`version`** in every published `package.json` so they match (wrapper `npm/knowerage-mcp/package.json` → **`@mtimma/knowerage-mcp`**, all six `npm/knowerage-mcp-*/package.json` → **`@mtimma/knowerage-mcp-*`**, and optionally `Cargo.toml`).
2. Commit and push the branch you will run from (e.g. `main`).
3. **Tag path (optional but common):** create and push the tag, e.g. `git tag v1.0.0 && git push origin v1.0.0` (tag name is only for Git; npm uses `version` in `package.json`).

If you **manually run** the workflow without bumping versions, `npm publish` can fail when that version is **already** on the registry (**npm never allows overwriting an existing version**).

Platform helper packages intentionally have **no `bin` field**; the main `knowerage-mcp` package resolves the native binary by path. That avoids npm’s “`bin[...]` script name was cleaned” publish warnings.

## CI without publishing

The [CI workflow](../.github/workflows/ci.yml) runs on pushes and pull requests to `main`: `cargo fmt`, `clippy`, `test`, and the Node wrapper test. It does **not** publish to npm.

## If a matrix job fails

- **Linux ARM64:** needs `gcc-aarch64-linux-gnu` (installed in the workflow) and [`.cargo/config.toml`](../.cargo/config.toml) linker settings.
- **Windows ARM64:** cross-compiles with `aarch64-pc-windows-msvc` on `windows-latest`; if the runner toolchain changes, check the job logs for missing MSVC components.
