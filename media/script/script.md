# DORMBIT demo video: voice script

Status: draft for the user's approval. Nothing is recorded before it is approved (release-and-submission Requirement 14.7).
Voice: recorded by the user (Requirement 14.5). Timings: `node scripts/media/script-timing.mjs`.

Each block below is one audio file, named by its ID (`V01.wav` to `V10.wav`). Read the text exactly; the captions are built from it. If you change a word, tell me so the captions match.

## How to read it

- About 150 words per minute: calm, clear, a little wry. Explain it to a smart friend; don't sell.
- Leave about half a second of silence before and after each file.
- Pronounce DORMBIT as "DORM-bit". Read `.env` as "dot env" and "TODOs" as "to-dos".
- In V08, give each of the four outcomes its own short beat. Read "Builds/runs" as "builds, runs", and the parentheses as a softer aside.
- Don't rush V10.

## Lines

### V01: The problem

Every developer has a folder like this. A project you'll get back to someday. The branch is still checked out. The TODOs are still there. But the context is gone.

### V02: Find

This is DORMBIT. It finds projects like that, preserves them, and brings them back. You grant it one folder. It never follows symlinks, never runs your code, and never runs the git command.

### V03: Evidence

It reads the project the way you would: the branch you were on, the uncommitted changes, the unchecked tasks in your Kiro spec, the TODO you left on line fourteen. Every finding points to its evidence. What it can't know stays unknown.

### V04: Plan

Before anything is written, you approve a plan. Secret files like `.env` are locked out: never read, never captured. Your approval is tied to a digest of that exact plan. Change one decision, and you approve again.

### V05: Capture and verify

The capture hashes every file, checks it again, and writes a capsule: a standard tar archive with a manifest you can read without DORMBIT. Then a separate verifier re-reads the whole capsule and checks it, rule by rule.

### V06: Restore

Restore never overwrites anything. It stages every file in a hidden folder, verifies them, and commits with one exclusive rename, which macOS refuses if the name is taken.

### V07: Break it

Now break it. Flip one byte inside a stored file, and verification fails and names the file. Damage the capsule after it passed its checks, and the restore catches it before committing. The destination is never created.

### V08: The brief

Then the resurrection brief: where you stopped, what's unfinished, and what to do next, with every line citing its evidence. And it reports four outcomes, separately, every time. Files restored and verified. Context reconstructed (inferred from evidence). AI session resumed: not performed. Builds/runs: unknown (never executed).

### V09: How it was built

One Rust core, two bodies: a native Mac app, and a WebAssembly demo in your browser. Built spec-first in Kiro. A reviewer agent caught real bugs, like a forged plan that could have slipped a `.env` file into a capsule. Every test number on screen comes from a recorded run.

### V10: End

DORMBIT. Pick up where your project stopped.

## Why each claim is allowed (Requirement 14.3)

| Line | Claim                                                                            | Backed by                                                                                                                                   |
| ---- | -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| V02  | Never follows symlinks, never runs code or the git command                       | Security rules 1-2; `FsVfs` refuses symlinks at any depth (D-25)                                                                            |
| V03  | Branch, uncommitted changes, unchecked Kiro tasks, the TODO on line 14, unknowns | Evidence rules `git.branch`, `git.uncommitted`, `kiro.task_unchecked`, `work.todo` (`src/pdf/renderInvoice.ts:14` in Dockline), `unknown.*` |
| V04  | `.env` never read or captured; approval bound to the plan digest                 | Plan reason `sensitive_env` (locked); `plan_approve` checks the digest                                                                      |
| V05  | Standard tar with a readable manifest; separate verifier                         | A demo capsule lists with plain `tar -tf` (`DORMBIT`, `manifest.json`, 182 gzip objects, `END`); verifier V01-V14                           |
| V06  | Staged, verified, one exclusive rename                                           | `rename_excl` (`renameatx_np` with `RENAME_EXCL`) in `crates/dormbit-fs/src/rename.rs`                                                      |
| V07  | Damage is caught and named; no destination after late damage                     | `e2e/demo/journey.spec.ts` (record `20261108T004329Z-pnpm-filter.json`); P04-T03 flip and truncation sweeps                                 |
| V08  | The four outcomes, exact labels                                                  | BRF-2                                                                                                                                       |
| V09  | The forged-plan bug the reviewer caught                                          | `docs/kiro/usage-log.md` (P04-T01), STATUS D-19/D-20                                                                                        |
| V09  | Test numbers come from recorded runs                                             | The video reads every test number from `docs/evidence/runs/*.json`                                                                          |

Never said or implied (Requirement 14.4): resuming an AI session, running or building a project, encryption, notarization.

## Text for one full take

The same ten lines, written the way a voice generator should say them, with one paragraph per line. The captions keep the written forms above: DORMBIT, `.env`, TODOs, and the exact outcome labels.

```text
Every developer has a folder like this. A project you'll get back to someday. The branch is still checked out. The to-dos are still there. But the context is gone.

This is Dormbit. It finds projects like that, preserves them, and brings them back. You grant it one folder. It never follows symlinks, never runs your code, and never runs the git command.

It reads the project the way you would: the branch you were on, the uncommitted changes, the unchecked tasks in your Kiro spec, the to-do you left on line fourteen. Every finding points to its evidence. What it can't know stays unknown.

Before anything is written, you approve a plan. Secret files like dot env are locked out: never read, never captured. Your approval is tied to a digest of that exact plan. Change one decision, and you approve again.

The capture hashes every file, checks it again, and writes a capsule: a standard tar archive with a manifest you can read without Dormbit. Then a separate verifier re-reads the whole capsule and checks it, rule by rule.

Restore never overwrites anything. It stages every file in a hidden folder, verifies them, and commits with one exclusive rename, which macOS refuses if the name is taken.

Now break it. Flip one byte inside a stored file, and verification fails and names the file. Damage the capsule after it passed its checks, and the restore catches it before committing. The destination is never created.

Then the resurrection brief: where you stopped, what's unfinished, and what to do next, with every line citing its evidence. And it reports four outcomes, separately, every time. Files restored and verified. Context reconstructed, inferred from evidence. AI session resumed: not performed. Builds and runs: unknown, never executed.

One Rust core, two bodies: a native Mac app, and a WebAssembly demo in your browser. Built spec-first in Kiro. A reviewer agent caught real bugs, like a forged plan that could have slipped a dot env file into a capsule. Every test number on screen comes from a recorded run.

Dormbit. Pick up where your project stopped.
```
