---
inclusion: always
---

# Stack and commands

- **Rust** 1.99.1 (`rust-toolchain.toml`; targets aarch64/x86_64-apple-darwin, wasm32-unknown-unknown). Tauri 2.12.5, git2 0.21.0 (vendored, no network), cap-std 4.0.3, sha2, tar, flate2 (rust_backend), ts-rs 12.0.1, serde, thiserror, proptest 1.12.0, insta 1.49.0. Every `[workspace.dependencies]` entry is pinned with `=`.
- **TypeScript** 6.0.3 (not 7: typescript-eslint 8.72.1 caps at <6.1), Node 24.19.0 and pnpm 11.23.0 via `.mise.toml`, React 19.3.1, Vite 8.3.5, Zod 4.6.7, Vitest 5.1.0, Playwright 1.64.1, three 0.187.0 (landing page only). npm versions are exact.
- Full version list and reasons: `docs/product/tech.md`.

| Purpose | Command |
| --- | --- |
| Rust checks | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` |
| TS checks | `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm format:check`, `pnpm build` |
| Contracts | `sh scripts/gen-schema.sh` (regenerate types), `sh scripts/schema-parity.sh` (PROP-11) |
| Fixture | `cargo test -p dockline-gen --locked`, `sh scripts/check-dockline.sh` |
| E2E | `pnpm --filter e2e exec playwright test` |
| Repo rules | `sh scripts/check-layout.sh`, `sh scripts/check-deps.sh`, `node scripts/check-pins.mjs`, `node scripts/kiro/check-config.mjs` |
| Evidence | `scripts/evidence/run.sh <command> [args...]` with `EVIDENCE_TASK=PXX-TYY` |

Shell notes: load `~/.cargo/env` before cargo in a fresh shell; Tauri's CLI rejects `CI=1`, use `CI=true`.
