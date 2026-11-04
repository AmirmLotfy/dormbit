# DORMBIT status ledger

The authoritative record of where every task stands. Kiro's task checkboxes can't show these states, so this file does.

**States:** Planned → In progress → Implemented (merged, builds) → Tested (its tests pass in a recorded run) → Verified (reviewer approved and demonstrated with a log or screenshot) → Released (in a tag or deployment) → Submitted.

A state is only moved forward with a link to evidence. Nothing here is "Tested" or "Verified" without a recorded run.

Last updated: Nov 4, 2026 (PT), after Task 8 (P01-T03).

## Phase 00: Feasibility and research

| Task        | Title                                | State       | Evidence                                                                                                                                         |
| ----------- | ------------------------------------ | ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1 (P00-T01) | Eligibility, environment, local repo | Verified    | [environment.md](environment.md); gitleaks hook ran on every commit                                                                              |
| 2 (P00-T02) | Toolchain install                    | Verified    | Rust 1.99.1, wasm-bindgen-cli 0.2.130, cargo-deny 0.20.2, binaryen 133, Node 24.19.0, pnpm 11.23.0 ([environment.md](environment.md))            |
| 3 (P00-T03) | Feasibility spikes                   | Verified    | [P00-T03-spikes.md](../evidence/P00-T03-spikes.md); ADR-001, ADR-002                                                                             |
| 4 (P00-T04) | Kiro feature probe and research      | In progress | [research.md](../product/research.md), [feature-probe.md](../kiro/feature-probe.md). Hook payload fields still need a new session to be observed |
| 5 (P00-T05) | Public GitHub repo                   | Verified    | https://github.com/AmirmLotfy/dormbit is public, MIT license detected, history free of the personal email                                        |

## Phase 01: Requirements and design

| Task        | Title                                            | State       | Evidence                                                                                                                                                      |
| ----------- | ------------------------------------------------ | ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 6 (P01-T01) | PRD and user journeys                            | Implemented | [PRD.md](../product/PRD.md)                                                                                                                                   |
| 7 (P01-T02) | Architecture, threat model, capsule format, ADRs | Implemented | [overview](../architecture/overview.md), [threat-model](../security/threat-model.md), [capsule-format](../architecture/capsule-format.md), ADR-003 to ADR-010 |
| 8 (P01-T03) | Kiro specs and status ledger                     | In progress | 8 specs in `.kiro/specs/`. Analyze Requirements and Correctness runs on the two security specs are pending (user action)                                      |

## Phases 02 to 18

All Planned. Task lists and dependencies live in `.kiro/specs/*/tasks.md`; the master plan is [MASTER_PLAN.md](../MASTER_PLAN.md).

## Known deviations from the master plan

Each is a deliberate change made during execution, recorded here so the plan and reality don't silently diverge.

| #   | Change                                                                                  | Why                                                                                     |
| --- | --------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| D-1 | Workspace permission rules for paths outside the repo moved to the user-scope file      | Kiro skips workspace-scope rules that point outside the workspace (observed in its log) |
| D-2 | Local history rebuilt once before the first push to remove a personal email             | The email appeared in an early environment report; nothing had been pushed              |
| D-3 | Spec property headings use `### Property N: Title (PROP-N)`                             | The Kiro spec validator rejects the `Property N (PROP-N): Title` form                   |
| D-4 | Each `tasks.md` ends with a "Task Dependency Graph" section                             | The validator requires it; Kiro uses it to run independent tasks in parallel waves      |
| D-5 | Specs propose extra error codes (for example `PLAN_INCOMPLETE`, `RESTORE_NOT_VERIFIED`) | Plan §8 has no code for these cases; to be confirmed in Task 11                         |
