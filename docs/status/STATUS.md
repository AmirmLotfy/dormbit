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

## Phase 02: Foundation (`foundation` spec)

| Task         | Title                                             | State       | Evidence                                                                                                                                                                                                                                                                                                                                                      |
| ------------ | ------------------------------------------------- | ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 9 (P02-T01)  | Monorepo scaffold                                 | Done        | `beb01cf`; lint, typecheck, test, build, layout, deps, and pin checks pass                                                                                                                                                                                                                                                                                    |
| 10 (P02-T02) | Hardened Tauri shell                              | Done        | `3d92331`; Allowlist_Guard 13/13 with self-tests; ad-hoc signed `.app` built and checked with `codesign`                                                                                                                                                                                                                                                      |
| 11 (P02-T03) | Shared contracts                                  | Done        | Rust models, ID newtypes, `JobEvent<R>`, ts-rs export (byte-stable), 64 strict Zod schemas with compile-time type equality, `DormbitClient` (28 methods), MockClient, Client_Contract (30 tests), desktop mock mode, PROP-11 `scripts/schema-parity.sh` passing at 512 samples per type in both directions (and shown to fail on a planted mismatch)          |
| 12 (P02-T04) | Test setup and Dockline fixture                   | Done        | Evidence_Runner (15/15 self-checks); `golden!` snapshots under `fixtures/golden/`; Vitest node and jsdom projects; Playwright + axe smoke 16/16 across Chromium, WebKit, Firefox, iPhone 13; Dockline generator with Fixture_Check, determinism property (8 cases), and `scripts/check-dockline.sh` (11/11, gitleaks clean). Records in `docs/evidence/runs/` |
| 13 (P02-T05) | Kiro config                                       | In progress | 12 steering files, 6 agents (deny-except write scopes), 5 hooks + 4 skills, Check_Config passing (fails on planted errors), 22/22 hook self-checks + shellcheck clean (records at `5bb3769`). Remaining user steps: paste `docs/kiro/mcp.json` and `docs/kiro/kiroignore.txt`, apply permissions                                                              |
| 14 (P02-T06) | CI and Devpost draft                              | Planned     |                                                                                                                                                                                                                                                                                                                                                               |
| —            | Landing page (judge-demo Requirement 22, ADR-011) | Done early  | `189d7f5`; interactive 3D capsule, gradient, phone and desktop checked in headless Chrome                                                                                                                                                                                                                                                                     |

## Phases 03 to 18

All Planned. Task lists and dependencies live in `.kiro/specs/*/tasks.md`; the master plan is [MASTER_PLAN.md](../MASTER_PLAN.md).

## Known deviations from the master plan

Each is a deliberate change made during execution, recorded here so the plan and reality don't silently diverge.

| #   | Change                                                                                                                                                  | Why                                                                                                                                                                                             |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D-1 | Workspace permission rules for paths outside the repo moved to the user-scope file                                                                      | Kiro skips workspace-scope rules that point outside the workspace (observed in its log)                                                                                                         |
| D-2 | Local history rebuilt once before the first push to remove a personal email                                                                             | The email appeared in an early environment report; nothing had been pushed                                                                                                                      |
| D-3 | Spec property headings use `### Property N: Title (PROP-N)`                                                                                             | The Kiro spec validator rejects the `Property N (PROP-N): Title` form                                                                                                                           |
| D-4 | Each `tasks.md` ends with a "Task Dependency Graph" section                                                                                             | The validator requires it; Kiro uses it to run independent tasks in parallel waves                                                                                                              |
| D-5 | Specs propose extra error codes (for example `PLAN_INCOMPLETE`, `RESTORE_NOT_VERIFIED`)                                                                 | Resolved in Task 11: only the 13 §8 codes plus the client-only `IPC_SCHEMA_MISMATCH` ship. Owning specs map their cases onto these codes or request a new one.                                  |
| D-6 | Visual direction replaced (ADR-011): new tokens, fonts, and status colors                                                                               | The §9 cream-and-serif palette matched the catalogued AI-template look. The user approved the "Flight Recorder" direction on Nov 4, 2026                                                        |
| D-7 | `apps/demo` has two pages: a landing page at `/dormbit/` and the judge demo at `/dormbit/demo/` (judge-demo Requirements 20.5, 22)                      | The user asked for a landing page with an interactive 3D hero. It was built ahead of the judge-demo phase                                                                                       |
| D-8 | `eslint-plugin-jsx-a11y@6.10.2` declares a peer range of ESLint up to 9, and we run ESLint 10.14.0                                                      | No newer release exists. The lint canary test proves its rules still fire. Revisit if a 10-compatible release ships                                                                             |
| D-9 | PROP-11 Rust samples come from a small hand-written deterministic generator (`crates/dormbit-core/tests/support/arb.rs`) instead of proptest strategies | Struct fields are checked by the compiler, list sizes stay bounded (proptest defaults made multi-MB files), and seeds are per type so failures reproduce. `PROPTEST_CASES` still sets the count |
