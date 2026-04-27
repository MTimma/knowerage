# Knowerage Contracts & Schemas

## Metadata Schema (Frontmatter)

Every analysis `.md` file must begin with YAML frontmatter containing:

```yaml
source_file: "src/legacy/AuthService.java"   # required, string
covered_lines:                                 # required, array of [start, end]
  - [120, 165]
  - [210, 248]
analysis_date: "2026-03-01T10:25:00Z"        # required, ISO 8601
```

### Rules

- The analysis file path (`.md`) is the identifier. There is no `analysis_id` field.
- Line ranges are inclusive `[start, end]` where `start >= 1`, `end >= start`, integers only.
- Normalization: parser output must sort and merge overlapping/adjacent ranges.

---

## Registry Schema

**File:** `knowerage/registry.json` — JSON object keyed by analysis path string (each value is a record). The `knowerage/` folder lives at the workspace root. The reference implementation serializes a map of path → record; see [examples/registry.sample.json](../examples/registry.sample.json) for a concrete multi-record example.

### Record Shape

```json
{
  "analysis_path": "knowerage/analysis/auth/auth-login.md",
  "source_path": "src/legacy/AuthService.java",
  "covered_ranges": [[120, 165], [210, 248]],
  "analysis_hash": "sha256:...",
  "source_hash": "sha256:...",
  "record_created_at": "2026-03-01T10:26:00Z",
  "record_updated_at": "2026-03-01T10:30:12Z",
  "status": "fresh"
}
```

---

## Status Vocabulary

| Status         | Meaning                                                  |
| -------------- | -------------------------------------------------------- |
| `fresh`        | Doc and source hashes match record; ranges valid         |
| `stale_doc`    | Analysis doc changed since last record                   |
| `stale_src`    | Source file changed since last record                    |
| `missing_src`  | Source file no longer exists                             |
| `dangling_doc` | Doc exists but record invalid/deleted/malformed          |

---

## MCP Tool List

| Tool                              | Purpose                                          |
| --------------------------------- | ------------------------------------------------ |
| `knowerage_create_or_update_doc`  | Create/update analysis document with metadata    |
| `knowerage_parse_doc_metadata`    | Parse frontmatter + validate coverage            |
| `knowerage_reconcile_record`      | Reconcile one analysis record                    |
| `knowerage_reconcile_all`         | Full rescan/rebuild for all analysis files        |
| `knowerage_get_file_status`       | Analyzed vs missing ranges for one source        |
| `knowerage_list_stale`            | List stale/problematic records                   |
| `knowerage_list_registry`         | Full registry snapshot (`records` = same shape as `registry.json`) |
| `knowerage_get_tree`              | Tree/grouped coverage for UI                     |
| `knowerage_coverage_overview`     | Batch coverage overview for all sources          |
| `registry_export_report`          | Export snapshot (JSON/YAML/TXT/HTML)             |
| `knowerage_generate_bundle`       | Export selected analyses to `toc*.md` + `combined*.md` + `manifest.json` |

### `knowerage_generate_bundle`

**Purpose:** Package selected analysis markdown files (with YAML frontmatter) for bulk ingestion (e.g. NotebookLM). Writes under `output_dir` relative to the workspace.

**Security:** Each `analysis_paths` entry and `output_dir` are validated with the same workspace rules as other tools (`..` rejected; paths must resolve under the workspace). Files are read only after validation.

**Limits (reference implementation):**

| Constant | Value | Behavior |
| -------- | ----- | -------- |
| `MAX_ANALYSIS_PATHS` | 10_000 | Exceeding count fails the tool call with `E_DOC_PARSE`. |
| `MAX_BYTES_PER_ANALYSIS_FILE` | 52_428_800 (50 MiB) | Larger files are skipped with an entry in `manifest.errors`. |
| `MAX_COMBINED_PART_BYTES` | 52_428_800 (50 MiB) | When the next document would exceed this size for the current part, a new part is started (`combined_2.md`, `toc_2.md`, …). |

**On-disk layout**

- Part 1: `toc.md`, `combined.md`
- Part 2 and above: `toc_N.md`, `combined_N.md` (N matches `part_index`)
- Always: `manifest.json` (includes `parts[]` with `part_index`, `toc_file`, `combined_file`, `combined_byte_length`, `analysis_paths` per part, plus top-level `files`, `errors`, `created_at`)

**Input**

| Field | Type | Required | Description |
| ----- | ---- | -------- | ----------- |
| `analysis_paths` | `string[]` | yes | Relative paths to analysis `.md` files, in export order. |
| `output_dir` | `string` | yes | Relative directory; created if missing. |

**Output (tool result JSON)**

| Field | Type | Description |
| ----- | ---- | ----------- |
| `ok` | bool | `true` on success. |
| `output_dir` | string | Relative `output_dir`. |
| `files_written` | `string[]` | Basenames written (e.g. `toc.md`, `combined.md`, `manifest.json`, or additional `toc_2.md` …). |
| `manifest` | object | Same structure as `manifest.json` (files include `part_index`; `parts` lists chunk metadata). |

**Example request**

```json
{
  "analysis_paths": [
    "knowerage/analysis/auth/login.md",
    "knowerage/analysis/billing/invoice.md"
  ],
  "output_dir": "knowerage/export/notebooklm-2026-04-06"
}
```

**Example success snippet**

```json
{
  "ok": true,
  "output_dir": "knowerage/export/notebooklm-2026-04-06",
  "files_written": ["toc.md", "combined.md", "manifest.json"],
  "manifest": {
    "created_at": "2026-04-06T12:00:00Z",
    "files": [
      {
        "analysis_path": "knowerage/analysis/auth/login.md",
        "source_path": "src/auth/Login.java",
        "content_hash": "sha256:…",
        "part_index": 1
      }
    ],
    "errors": [],
    "parts": [
      {
        "part_index": 1,
        "toc_file": "toc.md",
        "combined_file": "combined.md",
        "combined_byte_length": 1200,
        "analysis_paths": ["knowerage/analysis/auth/login.md", "knowerage/analysis/billing/invoice.md"]
      }
    ]
  }
}
```

**Partial failure:** Unreadable paths, parse errors, or oversize files are listed in `manifest.errors`; valid paths are still exported.

### `knowerage_get_file_status` Output Schema

```json
{
  "source_path": "src/legacy/AuthService.java",
  "total_lines": 350,
  "analyzed_ranges": [[120, 165], [210, 248]],
  "missing_ranges": [[1, 119], [166, 209], [249, 350]],
  "coverage_percent": 24.57,
  "range_attribution": [
    {
      "range": [120, 165],
      "analysis_path": "knowerage/analysis/auth/auth-login.md"
    },
    {
      "range": [210, 248],
      "analysis_path": "knowerage/analysis/auth/auth-login.md"
    }
  ]
}
```

### `knowerage_list_registry`

**Purpose:** Return the full registry for agents in one JSON value. Prefer this over reading `knowerage/registry.json` directly.

**Input (all optional)**

| Field                    | Type         | Description |
| ------------------------ | ------------ | ----------- |
| `analysis_path_prefix`   | `string`     | If non-empty, only entries whose key (analysis path) starts with this prefix. |
| `statuses`               | `string[]`   | If set, only records whose `status` is one of: `fresh`, `stale_doc`, `stale_src`, `missing_src`, `dangling_doc`. |

**Output**

| Field           | Type   | Description |
| --------------- | ------ | ----------- |
| `schema_note`   | string | Explains that `records` matches the on-disk registry root object. |
| `registry_file` | string | Always `knowerage/registry.json`. |
| `record_count`  | number | Number of entries in `records`. |
| `records`       | object | Same shape as the root object of `registry.json`: keys = analysis paths, values = [record shape](#record-shape). Keys are sorted lexicographically. |

### `knowerage_coverage_overview`

**Input (optional)**

| Field          | Type       | Description |
| -------------- | ---------- | ----------- |
| `extensions`   | `string[]` | File extensions **without** a leading dot (e.g. `java`, `xml`). Omit the field or pass `[]` to use the default allowlist: `java`, `xml`, `properties`, `gradle`, `kt`, `groovy`, `scala`, `kts`. |

When `extensions` is set, it applies consistently to: rows in `sources`, entries in `stale_records`, and the **project scan** used for `project_files` / `project_lines`.

**Project scan**

- Walks the workspace recursively from the project root.
- **Skips** directories named (at any depth): `.git`, `target`, `node_modules`, `dist`, `build`, `knowerage` (so analysis artifacts are not counted as project legacy sources).
- Counts only files whose extension matches the effective extension list (case-insensitive).
- **Line count** per file: `content.lines().count()` (same as `knowerage_get_file_status`). Files that cannot be read as UTF-8 are **skipped** (not counted).

**Behavior**

Returns a batch overview. **Coverage calculation uses fresh records only**; stale/dangling records are listed in `stale_records` but do not contribute to `analyzed_ranges` / `coverage_percent` / `covered_lines`. `summary.stale_count` is the number of stale records **after** the extension filter. `summary.total_records` is always the full registry size. `summary.total_sources` and `summary.tracked_files` are the same: distinct registry source paths matching the extension filter.

**Derived percentages** (denominator zero → `0.0`):

- `pct_files_tracked` = `tracked_files / project_files × 100`
- `pct_lines_covered_vs_project` = `covered_lines / project_lines × 100`

```json
{
  "summary": {
    "total_sources": 5,
    "tracked_files": 5,
    "total_records": 8,
    "avg_coverage_pct": 42.5,
    "stale_count": 2,
    "missing_src_count": 1,
    "project_files": 120,
    "project_lines": 45000,
    "tracked_lines": 2100,
    "covered_lines": 890,
    "pct_files_tracked": 4.17,
    "pct_lines_covered_vs_project": 1.98,
    "extensions_applied": ["java", "xml", "properties", "gradle", "kt", "groovy", "scala", "kts"]
  },
  "sources": [
    {
      "source_path": "src/legacy/AuthService.java",
      "total_lines": 350,
      "analyzed_ranges": [[120, 165], [210, 248]],
      "missing_ranges": [[1, 119], [166, 209], [249, 350]],
      "coverage_percent": 24.57,
      "range_attribution": [
        { "range": [120, 165], "analysis_path": "knowerage/analysis/auth/auth-login.md" }
      ]
    }
  ],
  "stale_records": [
    { "analysis_path": "...", "source_path": "...", "status": "stale_src", "..." : "..." }
  ]
}
```

### MCP App Limitations

- The iframe sandbox cannot open local workspace files. Analysis file paths are shown as copyable text with a copy-to-clipboard button.
- The MCP App requires a host that supports `_meta.ui.resourceUri` on tool definitions and the `ui://` resource protocol. Non-supporting hosts still receive a text tool result and can use `registry_export_report` for offline HTML reports or `knowerage_generate_bundle` for chunked analysis markdown export.

---

## Error Codes

| Code               | Meaning                              |
| ------------------ | ------------------------------------ |
| `E_DOC_PARSE`      | Failed to parse analysis frontmatter |
| `E_RANGE_INVALID`  | Invalid or malformed line range      |
| `E_SRC_MISSING`    | Source file not found                |
| `E_PATH_TRAVERSAL` | Path outside workspace root          |
| `E_REGISTRY_IO`    | Registry read/write failure          |

---

## Path & Security Rules

- Registry and analysis files live under workspace root in `knowerage/`.
- All paths must resolve to locations under the workspace root.
- Reject `..` segments and absolute paths that resolve outside root.
- Atomic writes for registry: write to a temp file, then rename.
- Registry updates are serialized in-process (shared lock on `registry.json` read/write); concurrent MCP and file-watcher paths must not perform unsynchronized read–modify–write.
- Workspace root resolution order:
  1. MCP roots (if provided)
  2. `KNOWERAGE_WORKSPACE_ROOT` environment variable
  3. `process.cwd()` / current working directory
- **`KNOWERAGE_AUTO_FULL_RECONCILE`** (MCP server `env`): Default **off** (unset, blank, or non-truthy). Set to `1`, `true`, `yes`, or `on` (trimmed, case-insensitive) to enable: the server watches `knowerage/` and runs a debounced `reconcile_all` on relevant file changes. Not the same as running `reconcile_all` after every MCP tool call.
