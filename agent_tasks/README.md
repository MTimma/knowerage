# Agent Tasks - Parallel Work Packages

Tasks reworked for parallel agents. Each phase has clear dependencies and unit test specs.

**User onboarding**: See [docs/USER_ONBOARDING.md](../docs/USER_ONBOARDING.md) for one-time setup, MCP config, and agent guidance.

## Dependency Graph

```
Phase 0 (Contracts)
    ├── Phase 1a (Parser)     ──┬──> Phase 1c (MCP) ──┬──> Phase 1d (Instructions)
    ├── Phase 1b (Registry)  ──┤                     └──> Phase 1e (Node Wrapper)
    │                         └──> Phase 2a (UI), Phase 2b (Export), Phase 2e (Onboarding)
    ├── Phase 1c (MCP)
    ├── Phase 2b (Export)
    └── Phase 2c (Security)  ──> can run in parallel with 1a–2b

Phase 3 (Test) — after Phase 1 & 2
    ├── 3a Test Registry
    ├── 3b Test Parser
    ├── 3c Test MCP
    ├── 3e Test Bundle
    └── 3   Test Plan (CI, rollout)
```

## Files

| File | Agent | Depends on |
|------|-------|------------|
| `phase_0_contracts.md` | 1 | — |
| `phase_1a_parser.md` | A | 0 |
| `phase_1b_registry.md` | B | 0 |
| `phase_1c_mcp.md` | C | 0, 1a, 1b (or stubs) |
| `phase_1d_instructions.md` | C/D | 0, 1c |
| `phase_1e_node_wrapper.md` | H | 0, 1c |
| `phase_2b_export.md` | E | 0, 1b |
| `phase_2c_security.md` | F | 0 |
| `phase_2e_onboarding.md` | — | 0, 1c |
| `phase_3a_test_registry.md` | G | 1b, 2c |
| `phase_3b_test_parser.md` | G | 1a |
| `phase_3c_test_mcp.md` | G | 1a, 1b, 1c |
| `phase_3e_test_bundle.md` | G | 2b, 1b |
| `phase_3_test_plan.md` | G | All |

## Unit Tests

Unit tests are embedded in each Phase 1 and Phase 2 task with expected specs. Run them during implementation; all must pass before the phase is complete.
