# Environment

Captured during Task 1 (P00-T01).

## Eligibility

- Current date/time (Pacific): **2026-11-03 17:20 PST**.
- Kiroween 2026 Submission Period: Oct 30, 2026 09:00 PT – Dec 4, 2026 14:00 PT.
- Status: **inside the Submission Period.** Work is eligible.
- Disclosure: this project's planning (Kiro Plan mode) took place Nov 3, 2026. Recorded here for the Task 52 disclosures.

## System

| Item                     | Value                                                              |
| ------------------------ | ------------------------------------------------------------------ |
| OS                       | macOS 26.6.2 (build 25G83)                                         |
| Architecture             | arm64 (Apple Silicon)                                              |
| Xcode Command Line Tools | installed at `/Library/Developer/CommandLineTools`, version 26.5.0 |
| Xcode.app                | not installed (CLT is sufficient for Tauri desktop builds)         |

## Toolchain (as found)

| Tool                | Version                                                                         | Notes                                                                                                       |
| ------------------- | ------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Node (Homebrew)     | v23.9.0                                                                         | not the pinned version                                                                                      |
| Node (active shell) | v22.13.1                                                                        | via mise/shell; will pin 24 LTS in Task 2                                                                   |
| Node 24 LTS (mise)  | 24.19.0 available                                                               | to be pinned in `.mise.toml`                                                                                |
| pnpm (mise)         | 11.23.0                                                                         | invoked directly; corepack shim on PATH prompts to download, so use the mise binary or pin via `.mise.toml` |
| git                 | system git (CLT)                                                                | present                                                                                                     |
| gh                  | authenticated as `AmirmLotfy` (keyring), scopes: gist, read:org, repo, workflow | enough to create the repo and run Actions                                                                   |
| gitleaks            | present                                                                         | used by pre-commit hook and CI                                                                              |
| ffmpeg / ffprobe    | present                                                                         | video pipeline (Phase 15)                                                                                   |
| whisper-cli         | present                                                                         | captions (Phase 15)                                                                                         |
| ImageMagick         | present                                                                         | image processing                                                                                            |
| **Rust / cargo**    | **NOT installed**                                                               | install in Task 2 (needs approval)                                                                          |

## Toolchain installed in Task 2 (P00-T02), Nov 3–4, 2026

| Tool                  | Version                                                           | How                       | Pinned in                     |
| --------------------- | ----------------------------------------------------------------- | ------------------------- | ----------------------------- |
| rustup + Rust         | rustc 1.99.1, cargo 1.99.1                                        | official rustup installer | `rust-toolchain.toml`         |
| Rust targets          | aarch64-apple-darwin, x86_64-apple-darwin, wasm32-unknown-unknown | `rustup target add`       | `rust-toolchain.toml`         |
| wasm-bindgen-cli      | 0.2.130 (matches the `wasm-bindgen` crate)                        | `cargo install --locked`  | crate version must stay equal |
| cargo-deny            | 0.20.2                                                            | `cargo install --locked`  | —                             |
| binaryen (`wasm-opt`) | 133                                                               | Homebrew                  | —                             |
| Node                  | 24.19.0                                                           | mise                      | `.mise.toml`                  |
| pnpm                  | 11.23.0                                                           | mise                      | `.mise.toml`                  |

Agent shells need `. "$HOME/.cargo/env"` before cargo commands until the IDE terminal reloads PATH.

Current crate versions on crates.io at install time (for Task 3 and Task 9 pins): tauri 2.12.5, tauri-build 2.7.4, tauri-cli 2.12.4, cap-std 4.0.3, git2 0.21.0, sha2 0.11.1, tar 0.4.46, flate2 1.1.10, ignore 0.4.34, unicode-normalization 0.1.26, ts-rs 12.0.1, proptest 1.12.0, insta 1.49.0, uuid 1.29.0, wasm-bindgen 0.2.130.

## Git identity

- Global git identity: a personal name and personal email address (not recorded here because this repo is public).
- Decision (user, Task 1): this repo uses a repo-local identity `Amir Lotfy <178108135+AmirmLotfy@users.noreply.github.com>` so the personal address stays out of the public history.

## Notes / quirks

- The Kiro IDE terminal integration garbles inline stdout for some compound commands (character echo doubling, occasional dropped output with exit -1). Mitigation adopted: route command output to files under `.scratch/` and read them back. This does not affect the commands themselves.
