---
inclusion: fileMatch
fileMatchPattern: "**/*.rs"
---

# Rust conventions

- Edition 2024, `rustfmt.toml` style. `clippy::all` is denied workspace-wide; `cargo clippy --workspace --all-targets --locked -- -D warnings` must pass.
- `dormbit-core`: `#![forbid(unsafe_code)]`, no `unwrap`/`expect`/`panic!` outside tests, no `std::fs`/`std::process`/`std::net`/clock (see `crates/dormbit-core/clippy.toml`). Builds for `wasm32-unknown-unknown`.
- Errors that cross IPC are `DormbitError { code, message, retryable }` with one of the 13 `ErrorCode`s; `retryable` comes from `ErrorCode::retryable()`. A new code needs the process in foundation design.md and the user's approval.
- IPC types live in `dormbit-core::model`, derive `Serialize, Deserialize, TS` with `#[ts(export)]`, use snake_case fields, `Option<T>` as `null` (never skipped), `SafeU64` for counts and sizes, and ID newtypes. After changing one: `sh scripts/gen-schema.sh`, update the Zod schema, the MockClient data, and `crates/dormbit-core/tests/support/arb.rs`; then `sh scripts/schema-parity.sh`.
- Tests: unit tests next to the code; property tests with proptest (CI uses 512 cases); snapshots through the `golden!` macro into `fixtures/golden/<area>/`.
- Security modules (paths, scopes, capsule, verify, restore, rename) go through the `rust-security` agent.
