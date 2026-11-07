# DORMBIT

**Pick up where your project stopped.**

[![CI](https://github.com/AmirmLotfy/dormbit/actions/workflows/ci.yml/badge.svg)](https://github.com/AmirmLotfy/dormbit/actions/workflows/ci.yml)
&nbsp;[![License: MIT](https://img.shields.io/badge/license-MIT-black.svg)](LICENSE)

A local-first macOS app that finds the projects you stopped touching, seals each one into a
capsule you can verify byte for byte, restores it into a new folder, and hands you an
evidence-backed brief of where you left off.

Built for the [Kiroween 2026](https://kiroween.devpost.com/) hackathon, Frankenstein category.

- **Web demo:** https://amirmlotfy.github.io/dormbit/ _(deployed in the release phase)_
- **Plan of record:** [`docs/MASTER_PLAN.md`](docs/MASTER_PLAN.md)
- **Progress ledger:** [`docs/status/STATUS.md`](docs/status/STATUS.md)

> Projects don't end. They stop. Months later the code is still on disk, but the thing you were
> in the middle of — the unpushed branch, the stash, the note you never committed, what your AI
> agent last tried — is scattered where you'd never look. DORMBIT treats a stopped project the
> way a flight recorder treats a flight: seal it, make it findable, and read it back with proof.

## What it does

1. **Find** — point it at a folder; it lists the projects inside and when each was last touched,
   reading Git through libgit2. It never runs your code.
2. **Plan** — every file gets a decision and a reason. Secrets and rebuildable folders stay out.
   Nothing is captured until you approve the plan.
3. **Capture** — a capsule is one file: a manifest, one compressed object per file, and an END
   record that catches truncation. Identical files share one object.
4. **Verify** — every object is re-hashed with SHA-256 and compared with the manifest. A damaged
   capsule fails loudly and names the file.
5. **Restore** — files are staged and verified, then moved into a new folder in one step. An
   interrupted restore never leaves a half-written project, and it refuses to overwrite an
   existing folder.
6. **Brief** — a resurrection brief reports four outcomes separately, each citing its evidence:
   files restored and verified; context reconstructed (inferred from evidence); AI session
   resumed: not performed; builds/runs: unknown (never executed).

## What it never does

- Never deletes or modifies your source folders. Cleanup touches only its own staging folders.
- Never runs your code, build tools, or the `git` CLI (Git is read through libgit2).
- Never uploads anything: no account, no telemetry, no network calls.
- Never reads your real AI-agent histories or credential stores without explicit consent, and
  even then only metadata.

## The web demo is synthetic

The browser demo runs on a fictional project (Dockline Invoices) and says so on every screen:
"Synthetic Demo — No Local Files Accessed". It is the real Rust core compiled to WebAssembly, so
the demo is evidence about the actual product — but it never reads your files.

## Install the macOS app

The app is **ad-hoc signed and not notarized** (this is a hackathon entry without a paid Apple
Developer account). macOS Gatekeeper will ask you to approve it once:

1. Download `DORMBIT.app` (or the `.dmg`) from the [latest release](https://github.com/AmirmLotfy/dormbit/releases).
2. Move `DORMBIT.app` to Applications and open it. macOS blocks it the first time.
3. Open **System Settings → Privacy & Security**, scroll to **Security**, and click **Open Anyway**
   next to the message about DORMBIT. Confirm with your password or Touch ID.

Requires macOS 13 or later on Apple Silicon or Intel.

## Build from source

Prerequisites: [rustup](https://rustup.rs/) (the toolchain is pinned in `rust-toolchain.toml`)
and [mise](https://mise.jdx.dev/) for Node and pnpm (pinned in `.mise.toml`).

```sh
mise install                 # Node 24 + pnpm, from .mise.toml
pnpm install --frozen-lockfile
pnpm --filter @dormbit/desktop tauri build   # builds and ad-hoc signs DORMBIT.app
```

Run the web demo locally:

```sh
pnpm --filter @dormbit/demo dev   # then open the printed localhost URL at /dormbit/
```

## How it was built with Kiro

The whole build runs on Kiro:

- **Specs** (`.kiro/specs/`): eight specs with requirements, design, and tasks, executed as
  parallel task waves.
- **Steering** (`.kiro/steering/`): 12 rule files that keep every turn on the truthfulness,
  security, and evidence rules.
- **Custom agents** (`.kiro/agents/`): an architect who only writes specs, a single Rust/security
  writer, a frontend agent, a QA agent that only writes tests, a release agent, and a read-only
  reviewer — each able to write only its own area.
- **Hooks** (`.kiro/hooks/`): format on save, type-check when a turn ends, block secrets before a
  write, run the fast security tests when a security module changes, and prompt a review after
  each task.
- **Evidence:** every test result in this repo is backed by a recorded run (command, exit code,
  counts, commit) under `docs/evidence/runs/`.

See [`docs/kiro/usage-log.md`](docs/kiro/usage-log.md) for notable usage.

## Architecture

- A pure core crate (`dormbit-core`) with no I/O and no clock, so it builds for WebAssembly and
  stays deterministic. The native and browser builds produce byte-identical capsules.
- Exactly one `unsafe` call in the project: the exclusive-rename primitive that makes restore
  atomic.
- An allowlist of 28 IPC commands, checked by a test that reads the Rust handlers, the build
  manifest, and the capabilities file together.

Details: [`docs/architecture/overview.md`](docs/architecture/overview.md) and the ADRs in
[`docs/adr/`](docs/adr/).

## Security

See [`SECURITY.md`](SECURITY.md) for how to report a vulnerability and the stated limits (no OS
sandbox in v1, no encryption in v1, integrity ≠ authorship, no telemetry).

## License

[MIT](LICENSE).
