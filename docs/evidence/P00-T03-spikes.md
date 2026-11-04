# P00-T03 evidence: feasibility spikes

Run on Nov 4, 2026 (PST), macOS 26.6.2 arm64, Command Line Tools 26.5 only (no Xcode.app), Rust 1.99.1, Node 24.19.0, pnpm 11.23.0. Source in `spikes/` (removed in Task 9).

## Spike a1: filesystem primitives (`spikes/fs-spike`)
`cargo test` — **7 passed, 0 failed.**

| Test | Proves |
|---|---|
| `exclusive_rename_commits_when_target_absent` | `renameatx_np(RENAME_EXCL)` moves a staged dir into place |
| `exclusive_rename_refuses_existing_target_and_leaves_it_untouched` | fails with `EEXIST`; existing contents unchanged; staging intact |
| `exclusive_rename_refuses_existing_file_target` | also refuses when the target is a file |
| `cap_std_rejects_parent_traversal` | `../` reads and writes fail; nothing written outside |
| `cap_std_rejects_absolute_paths` | absolute paths fail |
| `cap_std_rejects_symlink_escape` | absolute and relative symlinks pointing outside fail for read and write |
| `cap_std_allows_symlink_inside_root` | a symlink that stays inside the root still works |

Result: **go.** The restore engine's commit primitive and scope confinement work as designed on this machine (APFS).

## Spike a2: Tauri 2 shell (`spikes/tauri-shell`)
- `pnpm tauri build --bundles app` — **exit 0**, 1m38s cold. Tauri 2.12.5, React 19.3.1, Vite 8.3.5, TypeScript 7.0.2.
- Built and bundled with Command Line Tools only. Bundle: 10.2 MiB.
- `codesign -dv`: `Signature=adhoc`, `flags=0x10002(adhoc,runtime)`; `codesign --verify --deep --strict`: OK.
- The release binary was launched and the WebView called the allowlisted `app_info` command; the binary logged `SPIKE app_info invoked from the UI`.
- Command allowlist: `tauri_build::AppManifest::commands(&["app_info", "pick_folder"])` plus a capability granting only those two and `core:default`. No dialog, fs, or shell permissions are granted to the WebView; the folder dialog is opened from Rust via `tauri-plugin-dialog`'s Rust API.
- Not yet exercised: clicking **Choose folder…** (needs a human click). The dialog plugin is mature and the Rust call compiles; it will be exercised for real in Task 15.

Result: **go.**

## Spike b: WebAssembly core (`spikes/wasm-spike`)
- `cargo build --release --target wasm32-unknown-unknown` — **compiled** with sha2, tar, flate2 (rust_backend), ignore, unicode-normalization, uuid, wasm-bindgen 0.2.130.
- `wasm-bindgen 0.2.130` generated a Node module; `node compare.mjs` output:

```
NATIVE    tar_len=7168 tar_sha256=45ea70cce39a3f4aeb44ae49011643bb3f767cbeee92075a8a476f25854e205e ignore[node_modules/a.js]=true ignore[debug.log]=true ignore[keep.log]=false ignore[src/a.ts]=false nfc_eq=true nfc_len=5 uuid=6f1d2a33-8011-4c7e-9a01-23456789abcd
WASM      tar_len=7168 tar_sha256=45ea70cce39a3f4aeb44ae49011643bb3f767cbeee92075a8a476f25854e205e ignore[node_modules/a.js]=true ignore[debug.log]=true ignore[keep.log]=false ignore[src/a.ts]=false nfc_eq=true nfc_len=5 uuid=6f1d2a33-8011-4c7e-9a01-23456789abcd
WEBCRYPTO tar_sha256=45ea70cce39a3f4aeb44ae49011643bb3f767cbeee92075a8a476f25854e205e
WEBCRYPTO_MATCHES_RUST=true
```

- Size: 1,238,424 bytes raw; 779,795 after `wasm-opt -Oz`; 329,520 gzipped.

Result: **go.** See [ADR-001](../adr/ADR-001-wasm-core.md) and [ADR-002](../adr/ADR-002-capsule-container.md).

## Environment notes found during the spikes
- The Tauri CLI rejects `CI=1`; it needs `CI=true`.
- Screen capture preflight is `false` for the agent's shell. Screenshots of native windows (Task 46, Phase 15) need the user to grant Screen Recording to the Kiro app when asked.
- The workspace deny rule blocked `rm -rf .scratch` as intended (only `.scratch/*` cleanup is allowed).
