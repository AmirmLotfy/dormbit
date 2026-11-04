---
inclusion: always
---

# Repo layout and boundaries

```
crates/dormbit-core     pure logic: models, capsule format, plans, verification; no I/O, no clock (ADR-003)
crates/dormbit-fs       cap-std filesystem access and the one `unsafe` (renameatx_np, ADR-006)
crates/dormbit-git      libgit2 reads, no network (ADR-004)
crates/dormbit-agents   metadata-only agent adapters, consent-gated (ADR-008)
crates/dormbit-wasm     wasm-bindgen wrapper of core for the web demo (ADR-001)
crates/dormbit-cli      developer CLI
apps/desktop            Tauri shell (src-tauri) and React UI
apps/demo               landing page (/dormbit/) and judge demo (/dormbit/demo/), GitHub Pages (ADR-009)
packages/schema         ts-rs output in generated/ plus strict Zod schemas in src/
packages/client         DormbitClient, MockClient, Client_Contract
packages/ui             design system and screens (native-experience)
fixtures/dockline       fictional sample project and its generator; fixtures/golden for snapshots
e2e                     Playwright and axe tests
docs                    plan, ADRs, specs support, evidence records, status
```

- `dormbit-core` never uses `std::fs`, `std::process`, `std::net`, or the clock (clippy enforces it). Hosts inject a Vfs, a Clock, and an IdGen.
- Commands are allowlisted: Rust handlers, `build.rs`, and `capabilities/main.json` change together, and `apps/desktop/src-tauri/tests/allowlist.rs` checks all three.
- **Generated files are regenerated, never hand-edited:** `packages/schema/generated/` (`sh scripts/gen-schema.sh`), `fixtures/dockline/*.tar|*.json` (`cargo run -p dockline-gen --locked -- --out fixtures/dockline`), `fixtures/golden/**` (insta, reviewed), `docs/evidence/runs/` (the Evidence_Runner only).
- Each task's commit message names its task ID (for example `P02-T03`).
