# Phase 2e - User Onboarding & "Works Out of the Box"

**Depends on**: Phase 0 (Contracts), Phase 1c (MCP)  
**Stack**: Documentation, MCP config, optional MCP/server changes

---

## Objective

Close gaps so Knowerage works out of the box: clear workspace resolution, first-run bootstrap, copy-paste config, analysis convention, and agent guidance. Users and agents should not need to guess how to use Knowerage.

---

## Gap Summary

| Gap | Impact | Recommendation |
|-----|--------|-----------------|
| Workspace root | MCP may use wrong directory | Document how the MCP host passes workspace roots; add `KNOWERAGE_WORKSPACE_ROOT` and clear error if invalid |
| First-run bootstrap | Empty project has no registry | Consider auto-running `reconcile_all` on first `get_file_status` or `get_tree` when registry is empty |
| Copy-paste config | Users must write config manually | Provide ready-to-paste MCP server config snippet in README |
| Analysis folder | Agent may not know where to put analyses | Document convention (e.g. `knowerage/analysis/`) and/or add `knowerage.create_or_update_doc` default path |
| User rules | Agent must know to append to `knowerage/registry.txt` | Document as suggested MCP client agent rule for users |
| Agent inference | User must say "record coverage" every time | Add rule: "Analyze X" implies full workflow (create, reconcile, record) |

---

## Deliverables

1. **docs/USER_ONBOARDING.md** — One-time setup, MCP config snippet, suggested rules, typical prompts, troubleshooting.
2. **README update** — Link to onboarding; include copy-paste MCP config.
3. **MCP server** — `E_WORKSPACE_ROOT_INVALID` (or equivalent) when workspace root is invalid.
4. **First-run bootstrap** (optional) — Auto-run `reconcile_all` when registry is empty on first `get_file_status` or `get_tree`.
5. **Suggested project rule** — `.cursor/rules/knowerage-analysis.mdc` (or equivalent for the user’s MCP client) or documented snippet in onboarding.

---

## Implementation Steps

1. Create `docs/USER_ONBOARDING.md` with all sections (see spec below).
2. Add MCP config snippet to README (or create README if missing).
3. In MCP server: validate workspace root; return clear error if invalid.
4. (Optional) In MCP or registry: on first `get_file_status`/`get_tree` with empty registry, call `reconcile_all` internally.
5. Create `.cursor/rules/knowerage-analysis.mdc` or document the rule in onboarding.
6. Update `INSTRUCTIONS.md` (Phase 1d) to state: "Analyze X" implies full workflow (create, reconcile, record).

---

## USER_ONBOARDING.md Spec (Reference)

- §1 One-time setup (install, workspace root, first-run bootstrap)
- §2 Copy-paste MCP host config (Node + Rust variants)
- §3 Analysis folder convention
- §4 Suggested agent rules (MCP client guidance)
- §5 Typical prompts (no "record coverage" needed)
- §6 When to reconcile
- §7 Troubleshooting
- §8 Related docs

---

## INSTRUCTIONS.md Addition (Phase 1d)

Add to Agent Workflow section:

```markdown
### Implicit workflow

When the user asks to "analyze" or "document" a legacy source file, the agent should:
1. Create analysis with `knowerage.create_or_update_doc`
2. Reconcile with `knowerage.reconcile_record`
3. Append to `knowerage/registry.txt` in format: `{source_file}|{analysis_md_path}|{lines_from-to}`

The user does not need to say "record coverage" — the agent infers the full workflow.
```

---

## Acceptance Criteria

- [ ] `docs/USER_ONBOARDING.md` exists and covers all gaps
- [ ] README or main docs link to onboarding and include MCP config snippet
- [ ] MCP server returns clear error when workspace root is invalid
- [ ] (Optional) First-run bootstrap implemented when registry empty
- [ ] Suggested project or user-level agent rule created or documented
- [ ] `INSTRUCTIONS.md` includes implicit workflow guidance
