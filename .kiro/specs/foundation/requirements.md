# Requirements Document

## Introduction
This spec covers master plan Phase 02, the engineering foundation (P02, Tasks 9-14): the monorepo scaffold, the hardened Tauri shell, shared contracts, test tooling and the Dockline fixture, the Kiro setup, CI, `SECURITY.md`, and the Devpost draft. It formalizes the §6 repo layout and module boundaries, NAT-1 for the shell, the §8 `DormbitError` codes, DEM-3 (the Dockline generator), and the §10-11 Kiro configuration. Its properties are the PROP-11 setup (schema round trip) and a determinism property for the Dockline generator. Phase 00 is complete: Rust 1.99.1 is pinned in `rust-toolchain.toml`, Node 24.19.0 and pnpm 11.23.0 are pinned in `.mise.toml`, the public repo `github.com/AmirmLotfy/dormbit` exists with an MIT license, and the feasibility spikes are in `spikes/`.

## Glossary
- **Agent**: the Kiro main agent working in this repo. It is the only writer of generated files (ts-rs output, golden fixtures) and of `scripts/**`.
- **User**: the repo owner, who gives the approvals named in master plan §22.
- **Workspace**: the repo root, with its pnpm workspace (`pnpm-workspace.yaml`) and Cargo workspace (root `Cargo.toml`).
- **Layout_Check**: `scripts/check-layout.sh`, which checks that every §6 folder exists.
- **Dependency_Check**: `scripts/check-deps.sh`, which checks crate dependency boundaries with `cargo tree`.
- **Pin_Check**: `scripts/check-pins.mjs`, which checks that every dependency version is exact and that tool versions agree across files.
- **Strict_Canary**: `packages/schema/test/strict-canary.ts`, a file whose `// @ts-expect-error` lines only hold when the three strict TypeScript flags are on.
- **Lint_Config**: the root `eslint.config.js` flat config plus the Prettier config.
- **Lint_Canary**: a Vitest test that lints fixed source strings with the Lint_Config and checks the expected errors.
- **Core_Lints**: the lint attributes and `crates/dormbit-core/clippy.toml` that apply to `dormbit-core`.
- **Tauri_Shell**: the desktop app in `apps/desktop/src-tauri`, with its `tauri.conf.json`, `build.rs`, and capability files.
- **Capability_File**: a JSON file in `apps/desktop/src-tauri/capabilities/` that grants permissions to a window.
- **Command_Allowlist**: the list passed to `tauri_build::AppManifest::commands` in `build.rs`. Only commands on it can be granted to the WebView.
- **Plan_Command_List**: the 28 command names in master plan §6, from `app_info` to `adapters_list`.
- **Allowlist_Guard**: the Rust integration test `apps/desktop/src-tauri/tests/allowlist.rs` that fails when a plugin permission, a plugin dependency, or an unlisted command appears.
- **Channel**: a `tauri::ipc::Channel<T>` that Rust uses to stream typed events to the WebView.
- **AppInfo**: the value `app_info` reports: name, app version, core version, build profile, and minimum macOS version.
- **Schema_Exporter**: the ts-rs derives in Rust plus `scripts/gen-schema.sh`, which write TypeScript types into `packages/schema/generated/`.
- **Schema_Package**: `packages/schema`: the generated types plus hand-written Zod schemas in `packages/schema/src/`.
- **SafeU64**: a Rust newtype for 64-bit counts and sizes that can't hold a value above 2^53 - 1, so it maps to a TypeScript `number` without losing precision.
- **DormbitClient**: the TypeScript interface in `packages/client` with one method per command in the Plan_Command_List.
- **MockClient**: the in-memory DormbitClient in `packages/client`, used for UI work and tests.
- **Client_Contract**: `runClientContract(makeClient)` in `packages/client`, a test suite every DormbitClient implementation runs.
- **JobEvent**: the event union that job methods stream: `state`, `progress`, `done`, `failed`, `cancelled`.
- **DormbitError**: the error value `{code, message, retryable}` from §8, shared by Rust and TypeScript.
- **ErrorCode**: the closed set of stable `DormbitError` codes.
- **Client-only code**: an ErrorCode that only TypeScript produces and Rust never sends. The only one here is `IPC_SCHEMA_MISMATCH`.
- **Parity_Harness**: `scripts/schema-parity.sh` plus the Rust and Vitest tests it runs to check PROP-11 in both directions.
- **Evidence_Runner**: `scripts/evidence/run.sh`, which runs one command and records the result.
- **Evidence record**: the JSON file the Evidence_Runner writes to `docs/evidence/runs/` for one run.
- **Raw log**: the full output of one run, kept in `.scratch/logs/` and never committed.
- **Dockline_Generator**: the Rust binary package `dockline-gen` in `fixtures/dockline/generator/` that builds the Dockline fixture.
- **Dockline fixture**: the tar file `fixtures/dockline/dockline-invoices.tar` (DEM-3).
- **Sidecar**: `fixtures/dockline/dockline-invoices.sidecar.json`, written next to the tar by the Dockline_Generator.
- **Project tree**: the `dockline-invoices/` folder inside the Dockline fixture: the fictional invoicing app with its `.git`.
- **Agent stores**: the `agent-stores/` folder inside the Dockline fixture, outside the Project tree: a synthetic Claude Code store and a Kiro export zip.
- **Fixture_Check**: the test that regenerates the Dockline fixture into a temp folder and compares it with the committed files.
- **Steering file**: a Markdown file in `.kiro/steering/` with an `inclusion` mode: `always`, `fileMatch`, or `manual`.
- **Custom agent**: a file in `.kiro/agents/*.md` with `tools`, `permissions`, and `resources`.
- **Hook**: a JSON file in `.kiro/hooks/` with `version: "v1"`, plus the script in `scripts/hooks/` it runs.
- **Probe hook**: a temporary `.kiro/hooks/probe-*.json` file from Task 4 that logs its stdin payload to `.scratch/hook-probe/`.
- **Skill**: a folder `.kiro/skills/<name>/` with a `SKILL.md` whose front matter `name` equals the folder name.
- **MCP_Config**: `.kiro/settings/mcp.json`.
- **Check_Config**: `scripts/kiro/check-config.mjs`, which checks the steering, agents, hooks, skills, MCP_Config, and `.kiroignore` against this spec.
- **Recommended permissions file**: `docs/kiro/recommended-permissions.yaml` (workspace scope) and `docs/kiro/recommended-user-permissions.yaml` (user scope).
- **CI_Workflow**: `.github/workflows/ci.yml`.
- **Security_Policy**: the root `SECURITY.md`.
- **Devpost_Draft**: `docs/submission/devpost-draft.md`.
- **Gate note**: `docs/status/gates/P02.md`: what works, what's missing, what's next.

## Requirements

### Requirement 1: Monorepo layout (§6)

**User Story:** As a contributor, I want the repo laid out exactly as the master plan says, so that every spec, agent, and hook can rely on the same paths.

#### Acceptance Criteria

1. THE Workspace SHALL contain `apps/desktop` with a React entry in `apps/desktop/src` and the Tauri app in `apps/desktop/src-tauri`, and SHALL contain `apps/demo`.
2. THE Workspace SHALL contain the crates `crates/dormbit-core`, `crates/dormbit-fs`, `crates/dormbit-git`, `crates/dormbit-agents`, `crates/dormbit-wasm`, and `crates/dormbit-cli`.
3. THE Workspace SHALL contain the packages `packages/schema` (including `packages/schema/generated`), `packages/client`, and `packages/ui`.
4. THE Workspace SHALL contain `fixtures/dockline`, `fixtures/golden`, `fixtures/malicious`, and `e2e`.
5. THE Workspace SHALL contain `docs/product`, `docs/architecture`, `docs/adr`, `docs/security`, `docs/status`, `docs/evidence`, `docs/release`, `docs/submission`, `docs/kiro`, `scripts/`, `.github/workflows/`, `.githooks/`, and `.kiro/`.
6. WHEN the Layout_Check runs and every path in criteria 1-5 exists, THE Layout_Check SHALL exit 0.
7. IF any path in criteria 1-5 is missing, THEN THE Layout_Check SHALL exit 1 and print each missing path.
8. THE root `Cargo.toml` SHALL list as members the six crates in criterion 2, `apps/desktop/src-tauri`, and `fixtures/dockline/generator`.
9. THE root `pnpm-workspace.yaml` SHALL list `apps/*`, `packages/*`, and `e2e`.

### Requirement 2: Module boundaries (§6)

**User Story:** As a security reviewer, I want the core crate isolated from the disk and from native-only crates, so that it stays deterministic and builds for WebAssembly.

#### Acceptance Criteria

1. THE `dormbit-core` crate SHALL depend on no other DORMBIT crate and on none of `tauri`, `git2`, `cap-std`, `libc`, or `wasm-bindgen`.
2. THE `dormbit-wasm` crate SHALL depend on none of `dormbit-fs`, `dormbit-git`, `tauri`, `git2`, or `cap-std`.
3. IF a dependency forbidden by criterion 1 or 2 appears in `cargo tree` output, THEN THE Dependency_Check SHALL exit 1 and print the crate and the forbidden dependency.
4. WHEN `cargo build -p dormbit-core -p dormbit-wasm --target wasm32-unknown-unknown --locked` runs, THE Workspace SHALL build without errors.
5. THE root `Cargo.toml` SHALL declare `git2` with default features off and the `vendored-libgit2` feature on.
6. IF `cargo tree -e features` shows the `https` or `ssh` feature of `git2` for any member, THEN THE Dependency_Check SHALL exit 1.
7. THE root `Cargo.toml` SHALL declare `flate2` with default features off and the `rust_backend` feature on.

### Requirement 3: Pinned versions

**User Story:** As a judge building from source, I want every tool and dependency pinned, so that my build matches the one that was tested.

#### Acceptance Criteria

1. THE root `package.json` SHALL set `packageManager` to `pnpm@11.23.0` and `engines.node` to `24.19.0`.
2. THE Workspace SHALL declare every npm dependency, in every `package.json`, with an exact version (no `^`, `~`, `*`, `x`, range, tag, or URL).
3. THE root `Cargo.toml` SHALL declare every shared Rust dependency in `[workspace.dependencies]` with an exact `=` version requirement, and member crates SHALL use `workspace = true` for them.
4. THE root `Cargo.toml` SHALL pin `wasm-bindgen` to `=0.2.130`, the installed `wasm-bindgen-cli` version.
5. THE Workspace SHALL commit `Cargo.lock` and `pnpm-lock.yaml`.
6. IF a dependency version isn't exact, THEN THE Pin_Check SHALL exit 1 and print the manifest file and the dependency name.
7. IF the Node or pnpm version in `package.json` differs from `.mise.toml`, THEN THE Pin_Check SHALL exit 1 and print both values.
8. THE `rust-toolchain.toml` file SHALL keep channel `1.99.1` with the `aarch64-apple-darwin`, `x86_64-apple-darwin`, and `wasm32-unknown-unknown` targets.

### Requirement 4: Strict TypeScript

**User Story:** As a frontend developer, I want the strictest practical compiler settings everywhere, so that missing values and unchecked indexes are caught before runtime.

#### Acceptance Criteria

1. THE root `tsconfig.base.json` SHALL set `strict`, `noUncheckedIndexedAccess`, and `exactOptionalPropertyTypes` to `true`.
2. THE Workspace SHALL have every `tsconfig.json` under `apps/`, `packages/`, and `e2e/` extend `tsconfig.base.json`.
3. IF any `tsconfig.json` sets `strict`, `noUncheckedIndexedAccess`, or `exactOptionalPropertyTypes` to `false`, THEN THE Pin_Check SHALL exit 1 and print the file.
4. THE Strict_Canary SHALL contain one `// @ts-expect-error` line for each of the three flags, each on code that is an error only when that flag is on.
5. IF any of the three flags is off for `packages/schema`, THEN `pnpm typecheck` SHALL fail on the Strict_Canary.
6. WHEN `pnpm typecheck` runs on the scaffold, THE Workspace SHALL exit 0.

### Requirement 5: ESLint and Prettier

**User Story:** As a security reviewer, I want lint rules that block raw HTML and catch accessibility and hook mistakes, so that untrusted project text can't be injected into the UI (NAT-2).

#### Acceptance Criteria

1. THE Lint_Config SHALL apply the `eslint-plugin-jsx-a11y` recommended rules as errors to every `.ts` and `.tsx` file under `apps/` and `packages/`.
2. THE Lint_Config SHALL set the `eslint-plugin-react-hooks` rules `rules-of-hooks` and `exhaustive-deps` to `error`.
3. THE Lint_Config SHALL report an error for any JSX attribute or object property named `dangerouslySetInnerHTML`.
4. THE Lint_Config SHALL set `linterOptions.noInlineConfig` to `true`, so `eslint-disable` comments have no effect.
5. WHEN the Lint_Canary lints a source string that uses `dangerouslySetInnerHTML`, one that omits a hook dependency, and one with an `<img>` without `alt`, THE Lint_Canary SHALL find at least one error from the matching rule in each.
6. WHEN `pnpm lint` runs on the scaffold, THE Workspace SHALL run ESLint with `--max-warnings 0` and exit 0.
7. WHEN `pnpm format:check` runs, THE Workspace SHALL run `prettier --check` on `.ts`, `.tsx`, `.css`, `.json`, and `.md` files and exit 0.

### Requirement 6: Rust format and lint

**User Story:** As a security reviewer, I want unsafe code and panics kept out of the core by the compiler, so that the rules in `security.md` are enforced, not only written down.

#### Acceptance Criteria

1. WHEN `cargo fmt --all --check` runs, THE Workspace SHALL exit 0, using the root `rustfmt.toml`.
2. WHEN `cargo clippy --workspace --all-targets --locked -- -D warnings` runs, THE Workspace SHALL exit 0.
3. THE `crates/dormbit-core/src/lib.rs` file SHALL begin with `#![forbid(unsafe_code)]`.
4. THE root `Cargo.toml` SHALL set the workspace lint `unsafe_code = "deny"`, and every member SHALL inherit workspace lints.
5. THE Core_Lints SHALL deny `clippy::unwrap_used`, `clippy::expect_used`, and `clippy::panic` in non-test code of `dormbit-core`.
6. THE Core_Lints SHALL list `std::fs`, `std::process`, and `std::net` items in `crates/dormbit-core/clippy.toml` as disallowed, so any use in `dormbit-core` fails clippy.

### Requirement 7: Spike removal (Task 3 follow-up)

**User Story:** As a judge reading the repo, I want throwaway spike code gone and its results kept, so that the repo shows only shipped code plus the recorded evidence.

#### Acceptance Criteria

1. WHEN Task 9 is done, THE Workspace SHALL NOT contain a `spikes/` folder.
2. THE commit that removes `spikes/` SHALL name `P02-T01` in its message and state the three spike results: filesystem primitives go, Tauri shell go, WebAssembly core go.
3. THE files `docs/evidence/P00-T03-spikes.md`, `docs/adr/ADR-001-wasm-core.md`, and `docs/adr/ADR-002-capsule-container.md` SHALL stay in the repo unchanged by that commit.
4. WHEN `pnpm -r build` and `cargo build --workspace --locked` run after the removal, THE Workspace SHALL exit 0 for both.

### Requirement 8: Hardened Tauri shell configuration (NAT-1)

**User Story:** As a user, I want the WebView to have no access to the filesystem or shell, so that untrusted project text rendered in it can't reach my files.

#### Acceptance Criteria

1. THE Tauri_Shell SHALL set `app.security.csp` in `tauri.conf.json` to the CSP string in design.md, which contains no `'unsafe-inline'`, no `'unsafe-eval'`, and no remote origin.
2. THE Tauri_Shell SHALL set `app.security.freezePrototype` to `true` and `app.withGlobalTauri` to `false`.
3. THE Tauri_Shell SHALL NOT depend on `tauri-plugin-fs` or `tauri-plugin-shell`, and `apps/desktop/package.json` SHALL NOT depend on `@tauri-apps/plugin-fs` or `@tauri-apps/plugin-shell`.
4. THE Tauri_Shell SHALL have one Capability_File, which applies only to the window labeled `main` and grants only the permissions in the Allowlist_Guard's expected set.
5. THE Command_Allowlist SHALL be `["app_info"]` when this spec is done; later specs add commands only from the Plan_Command_List.
6. WHERE the build is a release build, THE Tauri_Shell SHALL NOT enable the `tauri` crate's `devtools` feature and SHALL NOT call any devtools-opening API.
7. THE Tauri_Shell SHALL set `identifier` to `io.github.amirmlotfy.dormbit`.
8. THE Tauri_Shell SHALL set `bundle.macOS.signingIdentity` to `"-"` (ad-hoc signing).
9. THE Tauri_Shell SHALL set `bundle.macOS.minimumSystemVersion` to `"13.0"`.
10. THE Tauri_Shell SHALL load the WebView only from its bundled assets in release builds, with no remote URL in any window configuration.

### Requirement 9: `app_info` over a Channel

**User Story:** As a developer, I want one command that round-trips from the UI to Rust and streams over a Channel, so that later job commands have a working pattern to follow.

#### Acceptance Criteria

1. WHEN the UI invokes `app_info` with a Channel, THE Tauri_Shell SHALL send exactly one `info` event carrying the AppInfo, then one `done` event, and SHALL return the same AppInfo.
2. THE AppInfo SHALL contain `name` = `"DORMBIT"`, `version` = the `apps/desktop/src-tauri` package version, `core_version` = the `dormbit-core` package version, `profile` = `debug` or `release`, and `min_macos` = `"13.0"`.
3. THE `app_info` command SHALL take no argument other than the Channel and SHALL return no filesystem path.
4. WHEN the main window receives the `info` event, THE desktop UI SHALL validate it with the Zod `AppInfoEventSchema` and show "DORMBIT" followed by the version it received from Rust.
5. IF the `info` event or the return value fails Zod validation, THEN THE desktop UI SHALL show an error state with the code `IPC_SCHEMA_MISMATCH` and SHALL NOT show a version.
6. WHEN a component test feeds a mocked `app_info` response with version `9.9.9`, THE desktop UI SHALL show `9.9.9`, which proves the version comes from Rust and not from a JS constant.

### Requirement 10: Allowlist guard (NAT-1)

**User Story:** As a security reviewer, I want a test that fails the moment a plugin permission or an unlisted command appears, so that the shell can't be widened by accident.

#### Acceptance Criteria

1. IF any Capability_File grants a permission that isn't in the Allowlist_Guard's expected set, THEN THE Allowlist_Guard SHALL fail and name the file and the permission.
2. IF any Capability_File grants a permission whose prefix is `fs:`, `shell:`, `dialog:`, `http:`, `opener:`, `process:`, or `core:path:`, THEN THE Allowlist_Guard SHALL fail and name the permission.
3. IF a command passed to `tauri::generate_handler!` isn't in the Command_Allowlist, or the Command_Allowlist has a command that isn't registered, THEN THE Allowlist_Guard SHALL fail and name the command.
4. IF the Command_Allowlist contains a name that isn't in the Plan_Command_List, THEN THE Allowlist_Guard SHALL fail and name it.
5. IF `apps/desktop/src-tauri/Cargo.toml` depends on any crate named `tauri-plugin-fs` or `tauri-plugin-shell`, or `apps/desktop/package.json` depends on `@tauri-apps/plugin-fs` or `@tauri-apps/plugin-shell`, THEN THE Allowlist_Guard SHALL fail and name the dependency.
6. IF the `tauri` dependency of `apps/desktop/src-tauri` enables the `devtools` feature, THEN THE Allowlist_Guard SHALL fail.
7. THE Allowlist_Guard SHALL include self-tests that feed it a capability containing `fs:default`, a handler list with an extra command, and a manifest with `tauri-plugin-shell`, and that assert it reports each one.

### Requirement 11: Rust-to-TypeScript type export

**User Story:** As a developer, I want one definition of every IPC type, so that Rust and TypeScript can't drift apart silently.

#### Acceptance Criteria

1. THE Schema_Exporter SHALL export every type that crosses IPC, including the §8 models, AppInfo, AppInfoEvent, JobEvent, DormbitError, and ErrorCode, to `packages/schema/generated/`.
2. WHEN `scripts/gen-schema.sh` runs twice on the same source, THE Schema_Exporter SHALL produce byte-identical files.
3. IF the files in `packages/schema/generated/` differ from a fresh run of `scripts/gen-schema.sh`, THEN THE CI_Workflow SHALL fail and print the differing files.
4. THE Schema_Exporter SHALL export each Rust `Option<T>` field as a required `T | null` field, and Rust SHALL serialize `None` as `null`, never by omitting the field.
5. THE Schema_Exporter SHALL export every SafeU64 field as `number`.
6. IF code tries to build a SafeU64 from a value above 2^53 - 1, THEN THE SafeU64 constructor SHALL return an error and create no value.
7. THE Schema_Exporter SHALL use snake_case JSON field names, matching §8.

### Requirement 12: Zod schemas

**User Story:** As a frontend developer, I want every IPC response validated at runtime by a schema that provably matches the generated type, so that a malformed response is rejected instead of rendered.

#### Acceptance Criteria

1. THE Schema_Package SHALL export a Zod 4 schema named `<Type>Schema` for every type in `packages/schema/generated/`.
2. THE Schema_Package SHALL include, for each schema, a type-level test that `z.infer<typeof <Type>Schema>` equals the generated `<Type>`, checked by `pnpm typecheck`.
3. IF a generated type has no matching Zod schema, THEN a Vitest test in `packages/schema` SHALL fail and name the type.
4. THE Schema_Package SHALL define every object schema as strict, so a value with an unknown key fails to parse.
5. THE Schema_Package SHALL define `ErrorCodeSchema` as an enum of exactly the ErrorCode values plus the Client-only code.

### Requirement 13: DormbitClient and MockClient

**User Story:** As a frontend developer, I want a client interface and a mock that returns schema-valid data, so that screens can be built before the Rust commands exist.

#### Acceptance Criteria

1. THE DormbitClient SHALL have exactly one method for each command in the Plan_Command_List, named in camelCase (for example `scope_pick` becomes `scopePick`).
2. THE DormbitClient SHALL NOT have any method parameter that carries a filesystem path; folders are referred to by grant ID (SCP-1).
3. THE MockClient SHALL implement every DormbitClient method with in-memory data based on the Dockline project.
4. WHEN the Client_Contract runs against the MockClient, every value the MockClient returns or streams SHALL parse under the matching Zod schema.
5. WHEN a MockClient job method runs, THE MockClient SHALL stream at least one `state` event, at least one `progress` event with real counts from its data, and then one `done` event.
6. WHEN `jobCancel` is called for a running MockClient job, THE MockClient SHALL stream one `cancelled` event and no `done` event for that job.
7. WHERE a test configures an override for a method, THE MockClient SHALL return the override's value or reject with the override's DormbitError.
8. WHEN the desktop app runs in development mode with `VITE_DORMBIT_CLIENT=mock`, THE desktop UI SHALL show a list of MockClient projects, each parsed with `ProjectRecordSchema`.

### Requirement 14: DormbitError codes (§8)

**User Story:** As a user, I want errors to carry stable codes and an honest retry hint, so that the UI can explain what happened and whether trying again makes sense.

#### Acceptance Criteria

1. THE ErrorCode set SHALL contain exactly `SCOPE_DENIED`, `PATH_UNSAFE`, `PLAN_DIGEST_MISMATCH`, `FILE_CHANGED_DURING_CAPTURE`, `CAPSULE_MALFORMED`, `CAPSULE_TRUNCATED`, `CAPSULE_HASH_MISMATCH`, `CAPSULE_VERSION_UNSUPPORTED`, `RESTORE_DEST_EXISTS`, `RESTORE_COLLISION`, `RESTORE_VOLUME_UNSUPPORTED`, `RESTORE_INSUFFICIENT_SPACE`, and `CANCELLED` when this spec is done; later specs add codes through the process in design.md.
2. THE DormbitError SHALL serialize as `{code, message, retryable}`, with `code` as the SCREAMING_SNAKE_CASE string.
3. THE DormbitError constructor SHALL set `retryable` from the fixed table in design.md: `true` for `FILE_CHANGED_DURING_CAPTURE` and `RESTORE_INSUFFICIENT_SPACE`, `false` for every other code.
4. THE Schema_Package SHALL define `IPC_SCHEMA_MISMATCH` as the only Client-only code, with `retryable` `false`.
5. IF Rust sends an error whose `code` isn't in `ErrorCodeSchema`, THEN THE client code SHALL reject with `IPC_SCHEMA_MISMATCH`.
6. IF the Rust ErrorCode set and the TypeScript set minus the Client-only code differ, THEN a parity test SHALL fail and name each code that is in only one of them.
7. THE Tauri_Shell SHALL replace the home folder prefix with `~` in every DormbitError message before it crosses IPC.

### Requirement 15: Schema round trip harness (PROP-11 setup)

**User Story:** As a developer, I want a two-way round-trip test between Rust and Zod, so that any type mismatch is caught with a concrete failing value.

#### Acceptance Criteria

1. WHEN the Parity_Harness runs, the Rust side SHALL write generated samples of every exported type, one JSON value per line, to `target/schema-parity/rust/<Type>.jsonl`.
2. WHEN the Vitest side reads a Rust sample, it SHALL parse it with the matching Zod schema and check that re-serializing the parsed value gives JSON equal to the sample.
3. WHEN the Parity_Harness runs, the TypeScript side SHALL write fast-check samples of every exported type to `target/schema-parity/ts/<Type>.jsonl`.
4. WHEN the Rust side reads a TypeScript sample, it SHALL deserialize it into the Rust type and check that re-serializing gives JSON equal to the sample.
5. IF any sample fails to parse or round-trips to different JSON, THEN THE Parity_Harness SHALL exit 1 and print the type name and the failing sample.
6. THE Parity_Harness SHALL take its case count from `PROPTEST_CASES` (Rust) and `FC_NUM_RUNS` (TypeScript), defaulting to 256 per type, and the CI_Workflow SHALL set both to 512.

### Requirement 16: Test tooling

**User Story:** As a QA engineer, I want every test framework the plan names installed and wired up, so that each later spec can add tests without setup work.

#### Acceptance Criteria

1. THE root `Cargo.toml` SHALL declare `proptest` `=1.12.0` and `insta` `=1.49.0` as workspace dev-dependencies.
2. THE Workspace SHALL configure insta to store snapshots under `fixtures/golden/`, and IF a snapshot differs in CI, THEN the test SHALL fail without writing a new snapshot.
3. THE Workspace SHALL configure Vitest, Testing Library, and fast-check so that `pnpm test` runs every TypeScript test in `apps/` and `packages/`.
4. THE `e2e` package SHALL depend on `@playwright/test` and `@axe-core/playwright` and define Chromium, WebKit, and Firefox projects.
5. THE Agent SHALL install Playwright browsers only after the User approves the download.
6. WHEN the sample Playwright test opens the built `apps/demo` placeholder through `vite preview`, it SHALL find zero axe violations with impact `critical`.

### Requirement 17: Evidence runner (§13-14 recording rules)

**User Story:** As a judge, I want every claimed test result backed by a record of the exact command, its exit code, and the commit, so that I can trust the numbers in the submission.

#### Acceptance Criteria

1. WHEN the Evidence_Runner is called as `scripts/evidence/run.sh <command> [args...]`, it SHALL run the command with its arguments as separate argv entries, without `eval` or `sh -c`.
2. THE Evidence_Runner SHALL exit with the command's exit code.
3. WHEN a run ends, THE Evidence_Runner SHALL write one Evidence record to `docs/evidence/runs/<UTC timestamp>-<slug>.json` with the command, exit code, test counts, git SHA, whether the tree was dirty, start time, end time, and duration.
4. THE Evidence_Runner SHALL replace the home folder prefix with `~` in every field of the Evidence record and in the Raw log.
5. THE Evidence_Runner SHALL write the Raw log to `.scratch/logs/<same name>.log` and record that relative path in the Evidence record.
6. WHEN the output contains a cargo test, Vitest, or Playwright summary, THE Evidence_Runner SHALL record passed, failed, and skipped counts summed over all summaries and name the source format.
7. IF no known summary is found, THEN THE Evidence_Runner SHALL record the counts as `null` with source `unknown`.
8. WHEN `PROPTEST_CASES`, `FC_NUM_RUNS`, or `EVIDENCE_TASK` is set, THE Evidence_Runner SHALL record its value.
9. IF the Evidence_Runner is called with no command, THEN it SHALL print usage, write no record, and exit 2.
10. IF the command isn't found, THEN THE Evidence_Runner SHALL record exit code 127 and exit 127.
11. WHEN `shellcheck scripts/evidence/run.sh` runs, it SHALL report no findings.

### Requirement 18: Dockline fixture contents (DEM-3)

**User Story:** As a judge, I want a realistic but fully fictional project to test DORMBIT on, so that every demo and golden test uses the same known input and no real data.

#### Acceptance Criteria

1. THE Dockline fixture SHALL contain exactly two top-level folders: `dockline-invoices/` (the Project tree) and `agent-stores/`.
2. THE Project tree SHALL contain manifests for a React + Vite + TypeScript frontend, a Fastify API, and SQLite via Drizzle (`package.json`, `tsconfig.json`, `vite.config.ts`, `drizzle.config.ts`), with no installed packages.
3. THE Project tree's `.git` SHALL hold exactly 24 commits, authored by two fictional authors with `.invalid` email addresses, with author and committer dates between 2025-03-03 and 2025-05-23, including at least one commit whose summary starts with `WIP`.
4. THE Project tree SHALL have the branches `main` and `feature/pdf-export`, with HEAD on `feature/pdf-export`.
5. THE Project tree SHALL have exactly 2 modified tracked files, 0 staged changes, 1 untracked note file, and 1 stash entry.
6. THE Project tree SHALL contain `README.md`, `docs/ROADMAP.md`, and `CHANGELOG.md`.
7. THE Project tree SHALL contain `src/pdf/renderInvoice.ts` with at least one `TODO` and one `FIXME` comment, a test file with one skipped test, and `.kiro/specs/pdf-export/tasks.md` with 7 tasks of which 3 are checked.
8. THE Project tree SHALL contain `.env` with values that contain the word `fake`, and `.env.example` with the same keys and empty values.
9. THE Project tree SHALL contain `data/dev.sqlite`, and `dist/` and `node_modules/` folders with small placeholder files, all three ignored by the Project tree's `.gitignore`.
10. THE Agent stores SHALL contain a synthetic Claude Code store under `agent-stores/claude/projects/` and a Kiro export zip under `agent-stores/kiro-export/` that contains `session.json` and `messages.jsonl`.
11. THE Project tree's `.git/hooks/` folder SHALL contain no files.
12. WHEN gitleaks scans the extracted Dockline fixture, it SHALL report zero findings.
13. THE Dockline fixture SHALL NOT contain any real person's name or email address, or the User's home folder path.

### Requirement 19: Dockline determinism and Sidecar (DEM-3)

**User Story:** As a developer, I want the fixture generator to be deterministic, so that regenerating the fixture proves the committed tar is what the code produces.

#### Acceptance Criteria

1. WHEN `cargo run -p dockline-gen --locked -- --out <dir>` runs, THE Dockline_Generator SHALL write `<dir>/dockline-invoices.tar` and `<dir>/dockline-invoices.sidecar.json` and nothing else outside a temp folder it creates.
2. THE Dockline_Generator SHALL write tar entries in ustar format, sorted by path bytes, with mtime `2025-05-23T00:00:00Z`, uid and gid 0, empty user and group names, and mode `0755` for directories and marked executables and `0644` for all other files.
3. THE Dockline_Generator SHALL use an explicit fixed signature for every commit and stash, write the Project tree's `.git/config` with fixed content, and run Git operations with `HOME` and `XDG_CONFIG_HOME` set to an empty temp folder.
4. THE Sidecar SHALL contain `schema_version`, `fixture_sha256` (SHA-256 of the tar), `demo_clock` = `2025-06-02T16:00:00Z`, `git_snapshot` (a §8 `GitSnapshot` of the Project tree), `agent_metadata`, and `expected_hashes` (SHA-256 of every regular file, keyed by tar path).
5. WHEN the Fixture_Check runs, it SHALL regenerate the fixture into a temp folder and compare it with `fixtures/dockline/`: entry paths, types, modes, sizes, and contents for every entry except `dockline-invoices/.git/index`, and the Sidecar except `fixture_sha256` and the `expected_hashes` entry for `dockline-invoices/.git/index`.
6. IF the Fixture_Check finds a difference, THEN it SHALL fail and print each differing path or Sidecar field.
7. THE Sidecar `git_snapshot` SHALL report branch `feature/pdf-export`, not detached, 2 modified, 0 staged, 1 untracked, 0 deleted, 0 conflicted, stash count 1, no upstream, no submodules, no LFS, no risky config, and no remotes.
8. WHEN the Dockline fixture is extracted under `.scratch/dockline/`, `git -C .scratch/dockline/dockline-invoices status --porcelain` SHALL list exactly the 2 modified files and the 1 untracked note.

### Requirement 20: Steering files (§10)

**User Story:** As the Agent, I want project rules loaded at the right moments, so that every turn follows the truthfulness, security, and evidence rules.

#### Acceptance Criteria

1. THE `.kiro/steering/` folder SHALL contain `product.md`, `tech.md`, `structure.md`, `security.md`, `definition-of-done.md`, and `project-status.md`, each with `inclusion: always`.
2. THE `.kiro/steering/` folder SHALL contain `rust.md` matching `**/*.rs`, `frontend.md` matching `apps/**` and `packages/**`, `ui-design.md` matching `packages/ui/**`, and `testing.md` matching test files, each with `inclusion: fileMatch`.
3. THE `.kiro/steering/` folder SHALL contain `hackathon.md` and `release.md`, each with `inclusion: manual`.
4. THE `project-status.md` file SHALL include `#[[file:docs/status/HANDOFF.md]]`.
5. THE `security.md` file SHALL state these hard rules: no `std::fs` in core, no git CLI, no fs or shell plugins, no raw HTML, no `unwrap` in core, never read real agent stores, and delegate security modules to `rust-security`.
6. THE `product.md` file SHALL state truthfulness, the four BRF-2 outcomes with their exact labels, no deletion, no code execution, and local-first.
7. THE `definition-of-done.md` file SHALL list the seven ledger statuses (Planned, In progress, Implemented, Tested, Verified, Released, Submitted) and state that a result counts only if it was recorded through the Evidence_Runner or CI.
8. THE `hackathon.md` file SHALL contain the rules checklist from §0 and every approval gate from §22.
9. IF a Steering file named in criteria 1-3 is missing or has a different inclusion mode, THEN THE Check_Config SHALL exit 1 and name the file.

### Requirement 21: Custom agents (§10)

**User Story:** As the User, I want each custom agent able to write only its own area, so that security code has a single writer and review stays independent.

#### Acceptance Criteria

1. THE `.kiro/agents/` folder SHALL contain `architect.md`, `rust-security.md`, `frontend.md`, `qa.md`, `release.md`, and `reviewer.md`.
2. THE Custom agents SHALL have no `model` field and no `hooks` field.
3. THE Custom agents SHALL list in `resources` every `always` Steering file plus the extra Steering files and Skills given for that agent in design.md.
4. THE Custom agents SHALL limit writes with an `fs_write` deny on `**` that excludes only these paths: `architect` `docs/adr/**`, `docs/architecture/**`, `.kiro/specs/**`; `rust-security` `crates/**`, `fixtures/malicious/**`, `docs/security/**`; `frontend` `apps/**`, `packages/**`; `qa` test folders, `e2e/**`, `docs/evidence/**`; `release` `.github/**`, `docs/release/**`, `docs/submission/**`, `media/**`, `README.md`.
5. THE `frontend` agent SHALL be denied writes to `packages/schema/generated/**`.
6. THE `reviewer` agent SHALL have no write permission.
7. THE Custom agents SHALL limit shell use to: `architect` none; `rust-security` `cargo *`; `frontend` `pnpm *`; `qa` and `reviewer` the test commands in design.md; `release` the build and media commands in design.md, with `gh *` set to ask.
8. THE `reviewer` agent's prompt SHALL require its final line to be exactly `APPROVED` or `NEEDS_CHANGES`.
9. WHEN the `architect` agent is asked to write `crates/dormbit-core/src/lib.rs` in Task 13, Kiro SHALL deny the write, and THE Agent SHALL record the denial in `docs/kiro/feature-probe.md`.
10. IF a Custom agent has a `model` or `hooks` field, or a write scope other than criterion 4, THEN THE Check_Config SHALL exit 1 and name the agent.

### Requirement 22: Hooks (§11)

**User Story:** As the User, I want automatic formatting, type checks, secret blocking, and release gating, so that common mistakes are caught when they happen.

#### Acceptance Criteria

1. WHEN Task 13 starts, THE Agent SHALL record the payload field names for each trigger from `.scratch/hook-probe/` in `docs/kiro/feature-probe.md` before writing any new Hook.
2. WHEN the payload fields are recorded, THE Agent SHALL remove every Probe hook, after the User approves the change to `.kiro/hooks/`.
3. THE `.kiro/hooks/` folder SHALL contain the Hooks `format`, `stop-gate`, `secret-scan`, `security-tests`, `task-tests`, `review-reminder`, and `release-gate`, each with `version: "v1"` and the trigger and matcher from §11.
4. WHEN the `format` Hook runs, `scripts/hooks/format.sh` SHALL read the path from the stdin JSON, resolve it, and run rustfmt or Prettier on it only if it is a regular file inside the repo, passing the path as a separate argument after `--`.
5. IF the path from the stdin JSON is outside the repo or doesn't match the matcher's extensions, THEN `scripts/hooks/format.sh` SHALL exit 0 without running a formatter.
6. WHEN the `stop-gate` Hook runs and `cargo check --workspace` or `pnpm typecheck` fails, `scripts/hooks/stop-gate.sh` SHALL print the errors, increment the counter in `.scratch/stop-gate.count`, and exit 1.
7. IF the `stop-gate` counter reaches 3 consecutive failures, THEN `scripts/hooks/stop-gate.sh` SHALL print that it is giving up, reset the counter, and exit 0.
8. WHEN both stop-gate checks pass, or the toolchain or workspace isn't set up, `scripts/hooks/stop-gate.sh` SHALL reset the counter and exit 0.
9. WHEN the `secret-scan` Hook runs, `scripts/hooks/secret-scan.sh` SHALL pipe the content being written to `gitleaks stdin` with redaction, and IF gitleaks reports a finding, THEN it SHALL exit 2 and print the rule ID without the secret value.
10. IF gitleaks isn't available, THEN `scripts/hooks/secret-scan.sh` SHALL exit 2 with a message saying the scan couldn't run.
11. WHEN the `security-tests` Hook runs, it SHALL run the fast security test subset and print any failures.
12. WHEN the `task-tests` Hook runs, it SHALL run the finished task's spec tests through the Evidence_Runner.
13. WHEN a task finishes, THE `review-reminder` Hook SHALL add the prompt "Run the reviewer sub-agent on this task's diff and record the verdict in STATUS.md."
14. IF a task in `release-and-submission` for Phase 12 or later is about to start and the latest Evidence record isn't green, gitleaks finds anything, `LICENSE` is missing, or `git check-ignore .kiro` prints a path, THEN THE `release-gate` Hook SHALL exit 2 and name each failed condition.
15. IF the recorded `PreTaskExec` or `PostTaskExec` payload doesn't identify the task, THEN THE Agent SHALL implement `release-gate` and `task-tests` as Skills that the `release` and `qa` agents run explicitly, instead of as Hooks.
16. THE Hook scripts SHALL NOT read or write any folder the User picks inside DORMBIT, and SHALL NOT interpolate payload values into a shell string.

### Requirement 23: Skills, MCP, `.kiroignore`, and permissions (§11)

**User Story:** As the User, I want browser automation off by default and agent access to private folders denied, so that my accounts and transcripts stay safe.

#### Acceptance Criteria

1. THE `.kiro/skills/` folder SHALL contain `visual-qa/SKILL.md`, `release-gate/SKILL.md`, and `demo-video/SKILL.md`, each with front matter `name` equal to its folder name and a `description`.
2. THE MCP_Config SHALL define a `playwright` server that runs `@playwright/mcp` at an exact version with `--isolated`, output to `.scratch/playwright`, and allowed origins limited to `http://localhost` ports and `https://amirmlotfy.github.io`.
3. THE MCP_Config SHALL define a `playwright-chrome` server with `--extension` and `disabled: true`.
4. THE MCP_Config SHALL NOT define a GitHub MCP server.
5. THE root `.kiroignore` SHALL list `target/`, `node_modules/`, `media/raw/`, and `.scratch/`.
6. THE Agent SHALL ask the User before the Recommended permissions file is applied, and the User SHALL copy it into place, because Kiro can't write `~/.kiro/workspace-roots/`.
7. WHEN the permissions are applied, THE Agent SHALL run one allowed command, one asked command, and one denied command from the Recommended permissions file and record each outcome in `docs/kiro/feature-probe.md`.
8. WHEN the Check_Config runs on a correct setup, it SHALL exit 0; IF any check in Requirements 20-23 fails, THEN it SHALL exit 1 and name each failure.

### Requirement 24: Continuous integration

**User Story:** As a judge, I want every push checked on macOS by the same commands the specs name, so that a green badge means the code was tested.

#### Acceptance Criteria

1. WHEN a commit is pushed to any branch or a pull request is opened, THE CI_Workflow SHALL run on `macos-15`.
2. THE CI_Workflow SHALL run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo test --workspace --locked` with `PROPTEST_CASES=512`.
3. THE CI_Workflow SHALL run `pnpm install --frozen-lockfile`, `pnpm lint`, `pnpm typecheck`, and `pnpm test` with `FC_NUM_RUNS=512`.
4. THE CI_Workflow SHALL build `dormbit-wasm` for `wasm32-unknown-unknown` and run `wasm-bindgen` 0.2.130 on the output.
5. THE CI_Workflow SHALL run the Playwright suite in `e2e/` on Chromium, WebKit, and Firefox.
6. THE CI_Workflow SHALL run gitleaks over the full Git history.
7. THE CI_Workflow SHALL run `cargo deny check` with the root `deny.toml`.
8. THE CI_Workflow SHALL run `pnpm audit --audit-level high`.
9. THE CI_Workflow SHALL run the Layout_Check, Dependency_Check, Pin_Check, Check_Config, schema drift check, Parity_Harness, Fixture_Check, `actionlint`, and `shellcheck` on `scripts/**/*.sh` and `.githooks/*`.
10. THE CI_Workflow SHALL set top-level `permissions: contents: read` and pin every third-party action to a full commit SHA.
11. IF any step fails, THEN THE CI_Workflow SHALL mark the run failed.
12. THE README SHALL show the CI_Workflow status badge.

### Requirement 25: Security policy

**User Story:** As a security researcher, I want to know how to report a problem and what DORMBIT does and doesn't protect, so that I can report responsibly and judge the claims.

#### Acceptance Criteria

1. THE Security_Policy SHALL state that only the latest release is supported.
2. THE Security_Policy SHALL tell reporters to use GitHub private vulnerability reporting on `AmirmLotfy/dormbit` and not to open public issues for vulnerabilities.
3. THE Agent SHALL ask the User before enabling private vulnerability reporting on the repo.
4. THE Security_Policy SHALL list the stated limits from §7: scope enforcement is inside the app with no OS sandbox in v1, there is no encryption in v1, and reading files can update their access times.
5. THE Security_Policy SHALL state that SHA-256 shows integrity, not authorship (VER-2), and that DORMBIT has no telemetry (NAT-2).

### Requirement 26: Devpost draft

**User Story:** As the User, I want an early submission draft, so that the final submission is a review of known content and not a rush.

#### Acceptance Criteria

1. THE Devpost_Draft SHALL contain the project name, the tagline "Pick up where your project stopped.", the story sections, built-with tags, the category Frankenstein, and placeholders for the demo URL, video URL, and Kiro write-up.
2. THE Devpost_Draft SHALL mark every number or claim that hasn't been measured yet as `TBD (Task N)`, naming the task that will measure it.
3. THE Agent SHALL ask the User whether they have teammates and which bonus prizes they qualify for, and SHALL record the answers in the Devpost_Draft.
4. THE User SHALL save the draft on Devpost themselves; THE Agent SHALL NOT sign in to Devpost in this phase.

### Requirement 27: Phase 02 exit and approvals

**User Story:** As the User, I want a clear gate at the end of Phase 02, so that Phase 03 starts only when the foundation works.

#### Acceptance Criteria

1. WHEN Phase 02 ends, THE Agent SHALL write the Gate note with the CI run link, the hook summary, and what is missing.
2. WHEN Phase 02 ends, THE Agent SHALL update `docs/status/STATUS.md` and `docs/status/HANDOFF.md`.
3. THE Agent SHALL mark Phase 02 done only when the first CI_Workflow run on `main` is green, `app_info` round-trips in the desktop app, the Dockline fixture is committed, and every Hook has fired once with its expected result.
4. THE Agent SHALL name the task ID (P02-T01 to P02-T06) in every commit message for this phase.
5. THE Agent SHALL ask the User before installing anything outside the repo, including Playwright browsers.

## Open questions
1. **Sidecar file name and corrupted capsule.** DEM-3 and Task 12 name a JSON sidecar and a prepared corrupted capsule but give no paths. This spec fixes the Sidecar as `fixtures/dockline/dockline-invoices.sidecar.json`. The capsule format is written in Task 20, so Task 12 can't produce a capsule. The corrupted Dockline capsule is left to `capsule-and-verification` (its `fixtures/malicious/corrupt-object.dormbit` and Dockline golden capsule).
2. **Agent stores layout.** `intelligence-and-brief` requires the synthetic stores to sit outside the Project tree. This spec puts them in `agent-stores/` next to `dockline-invoices/` in the same tar. The synthetic Claude Code field names follow the official docs where documented; Task 32 may regenerate them if it confirms different names.
3. **`.git/index` and the Sidecar.** Because `.git/index` holds machine-specific stat data, the tar's SHA-256 and the index file's hash also change on every regeneration. The Fixture_Check skips exactly those two Sidecar fields.
4. **Error codes proposed by other specs.** `scope-and-discovery` (`STATE_IO`), `capsule-and-verification` (`PLAN_INCOMPLETE`, `PLAN_ITEM_LOCKED`, `CAPSULE_LIMIT_EXCEEDED`, `CAPSULE_EXISTS`, `IO_FAILED`), `transactional-restore` (`RESTORE_NOT_VERIFIED`, `RESTORE_TARGET_FORBIDDEN`, `RESTORE_VERIFY_FAILED`, `RESTORE_IO_FAILED`), and `intelligence-and-brief` (`EXPORT_DEST_EXISTS`, `EXPORT_WRITE_FAILED`) propose codes that §8 doesn't list. This spec ships only the §8 set plus the Client-only `IPC_SCHEMA_MISMATCH` (confirming the `native-experience` proposal). Each owning task adds its codes in Rust and TypeScript together and updates §8, with the User's approval of the plan change.
5. **Retryable values.** §8 defines the `retryable` field but not its value per code. The table in design.md is a conservative choice: only two codes are retryable, so the UI never retries a cancel or a security refusal automatically.
6. **Atomic JSON helper.** `capsule-and-verification` expects an atomic JSON helper from foundation Task 11, while `scope-and-discovery` defines the `StateStore` (temp file, `sync_all`, rename) in Task 15. The plan doesn't assign a helper to P02, so this spec doesn't add one. `capsule-and-verification` should use the `StateStore` from `scope-and-discovery`.
7. **JSON field case.** `native-experience` shows `RestoreInput` with camelCase fields. This spec uses snake_case for every JSON field, to match §8 and the Sidecar, and camelCase only for top-level Tauri command argument names, which Tauri converts. The owning specs should follow that rule.
8. **`ui-design.md` token reference.** The token file arrives in `native-experience` task 1 (Task 34), after Task 13. Until then `ui-design.md` points to §9; the `#[[file:...]]` reference is added when the file exists.
9. **`.kiroignore` and `.scratch/`.** §11 lists `.scratch/` in `.kiroignore`, but agents read command output from `.scratch/` as a terminal workaround, and the stop-gate counter lives there. If `.kiroignore` blocks reads and not only indexing, Task 13 records that and asks the User whether to keep the entry.
10. **Core permissions.** The Task 3 spike granted `core:default`. That set includes `core:path:default`, which lets the WebView resolve folder paths such as the home folder. This spec starts with no `core:*` permission and adds one only when a test shows the window needs it, never `core:path:*`.
11. **Custom agent front matter.** The exact keys for `tools`, `permissions`, and `resources` in `.kiro/agents/*.md` are confirmed in Task 13 against Kiro's docs, as `docs/kiro/feature-probe.md` notes.
