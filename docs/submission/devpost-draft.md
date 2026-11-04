# Devpost submission draft

Working draft for the Kiroween 2026 submission. Every unmeasured number or claim is marked
`TBD (Task N)`, naming the task that will produce it. The user saves this on Devpost; the agent
does not sign in to Devpost in this phase (Requirement 26.4).

- **Project name:** DORMBIT
- **Tagline:** Pick up where your project stopped.
- **Category:** Frankenstein (exactly one category)
- **Demo URL:** `https://amirmlotfy.github.io/dormbit/` — TBD go-live (Task 56, release-and-submission)
- **Video URL:** TBD (Task 55, demo video; public per the approval gate)
- **Repository:** https://github.com/AmirmLotfy/dormbit (public, MIT)
- **Kiro write-up:** see "How we built it" below, backed by `docs/kiro/usage-log.md`

## Elevator pitch

Projects don't end. They stop. Months later the code is still on disk, but the thing you were
in the middle of — the unpushed branch, the stash, the note you never committed, what your AI
agent last tried — is scattered where you'd never look. DORMBIT finds those stopped projects,
seals each into a capsule you can verify byte for byte, restores it into a fresh folder, and
hands you an evidence-backed brief of where you left off.

## The problem

A developer's machine fills with half-finished projects. Backups copy bytes but lose the
context: which branch was active, what was stashed, what the last session was about. And a
backup that says it worked rarely proves it. DORMBIT treats a stopped project the way a flight
recorder treats a flight: seal it, make it findable, and read it back with proof.

## What it does

- **Find.** Point it at a folder; it lists the projects inside and when each was last touched,
  reading Git through libgit2. It never runs your code.
- **Plan.** Every file gets a decision and a reason. Secrets and rebuildable folders stay out.
  Nothing is captured until you approve the plan.
- **Capture.** A capsule is one file: a manifest, one compressed object per file, and an END
  record that catches truncation. Identical files share one object.
- **Verify.** Every object is re-hashed with SHA-256 and compared with the manifest. A damaged
  capsule fails loudly and names the file.
- **Restore.** Files are staged and verified, then moved into a new folder in one step. An
  interrupted restore never leaves a half-written project, and it refuses to overwrite an
  existing folder.
- **Brief.** A resurrection brief reports four outcomes separately and cites the evidence for
  each line: files restored and verified; context reconstructed (inferred from evidence); AI
  session resumed: not performed; builds/runs: unknown (never executed).

## How we built it

- **Kiro specs** drove the whole build: eight specs under `.kiro/specs/` with requirements,
  design, and tasks, run as parallel task waves.
- **Steering** (12 files) keeps every turn on the truthfulness, security, and evidence rules.
- **Custom agents** split the work: an architect who only writes specs, a single Rust/security
  writer, a frontend agent, a QA agent that only writes tests, a release agent, and a read-only
  reviewer. Each can write only its own area.
- **Hooks** format on save, type-check when a turn ends, block secrets before a write, run the
  fast security tests when a security module changes, and remind us to review each task.
- **Evidence runner:** every test result in this submission is backed by a recorded run
  (command, exit code, counts, commit) under `docs/evidence/runs/`.
- **Stack:** Tauri 2 + Rust for the app; the same Rust core compiled to WebAssembly powers the
  browser demo, so the demo is evidence about the real product. React 19 + Zod for the UI.

## Architecture highlights

- A pure core crate with no I/O and no clock, so it builds for WebAssembly and stays
  deterministic; the native and browser builds produce byte-identical capsules (PROP-10).
- One `unsafe` call in the whole project: the exclusive-rename primitive that makes restore
  atomic.
- An allowlist of exactly 28 IPC commands, checked by a test that reads the Rust handlers, the
  build manifest, and the capabilities file together.

## What's real in the demo

The web demo runs on a synthetic, fictional project (Dockline Invoices) and says so on every
screen: "Synthetic Demo — No Local Files Accessed". It uses the real Rust core in WebAssembly.
It never reads your files.

## Numbers (filled in as they're measured)

- Test count across Rust, TypeScript, and E2E: TBD (Task 47, native-experience checkpoint)
- WCAG AA conformance: TBD (Task 47)
- Native and WebAssembly capsule parity cases: TBD (Task 44, judge-demo)
- Demo video length (under 3:00): TBD (Task 55)

## Built with

Kiro, Rust, Tauri, React, TypeScript, WebAssembly, Zod, Vite, Playwright, three.js, libgit2,
SHA-256.

## Try it out

- Web demo: `https://amirmlotfy.github.io/dormbit/` — TBD (Task 56)
- macOS app: download from the GitHub release. It is ad-hoc signed and not notarized, so macOS
  asks you to approve it once in System Settings → Privacy & Security → Open Anyway. Build
  from source instructions are in the README.

## Open questions for the user (Requirement 26.3)

- **Teammates:** solo entry, or are there teammates to credit? (answer: TBD)
- **Bonus prizes:** which, if any, does this entry qualify for? (answer: TBD)
