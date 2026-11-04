# Environment

Captured during Task 1 (P00-T01).

## Eligibility
- Current date/time (Pacific): **2026-11-03 17:20 PST**.
- Kiroween 2026 Submission Period: Oct 30, 2026 09:00 PT – Dec 4, 2026 14:00 PT.
- Status: **inside the Submission Period.** Work is eligible.
- Disclosure: this project's planning (Kiro Plan mode) took place Nov 3, 2026. Recorded here for the Task 52 disclosures.

## System
| Item | Value |
|---|---|
| OS | macOS 26.6.2 (build 25G83) |
| Architecture | arm64 (Apple Silicon) |
| Xcode Command Line Tools | installed at `/Library/Developer/CommandLineTools`, version 26.5.0 |
| Xcode.app | not installed (CLT is sufficient for Tauri desktop builds) |

## Toolchain (as found)
| Tool | Version | Notes |
|---|---|---|
| Node (Homebrew) | v23.9.0 | not the pinned version |
| Node (active shell) | v22.13.1 | via mise/shell; will pin 24 LTS in Task 2 |
| Node 24 LTS (mise) | 24.19.0 available | to be pinned in `.mise.toml` |
| pnpm (mise) | 11.23.0 | invoked directly; corepack shim on PATH prompts to download, so use the mise binary or pin via `.mise.toml` |
| git | system git (CLT) | present |
| gh | authenticated as `AmirmLotfy` (keyring), scopes: gist, read:org, repo, workflow | enough to create the repo and run Actions |
| gitleaks | present | used by pre-commit hook and CI |
| ffmpeg / ffprobe | present | video pipeline (Phase 15) |
| whisper-cli | present | captions (Phase 15) |
| ImageMagick | present | image processing |
| **Rust / cargo** | **NOT installed** | install in Task 2 (needs approval) |

## Git identity
- Global git identity: a personal name and personal email address (not recorded here because this repo is public).
- Decision needed: whether commits should use the GitHub `@users.noreply.github.com` address instead of the personal Gmail, since this repo is public. See the question raised in Task 1.

## Notes / quirks
- The Kiro IDE terminal integration garbles inline stdout for some compound commands (character echo doubling, occasional dropped output with exit -1). Mitigation adopted: route command output to files under `.scratch/` and read them back. This does not affect the commands themselves.
