# ADR-004: Read Git with `git2` and never run the git CLI

- **Status:** Accepted, Nov 4, 2026
- **Task:** P01-T02
- **Related:** GIT-1, CAP-5, RST-4, RST-6; [threat model](../security/threat-model.md) T-11, T-12

## Context
DORMBIT inspects Git repositories it didn't create. A repository's config can make the git CLI run arbitrary commands: `core.fsmonitor`, `core.hooksPath` and hooks, `filter.*` drivers, `core.sshCommand`, and `credential.helper`. Running `git status` on an untrusted repo is therefore code execution. DORMBIT promises never to run code (PRD non-goals).

DORMBIT needs: branch, HEAD, upstream ahead/behind as of the last fetch, working-tree status counts, stash count, submodules, LFS detection, linked worktrees, and risky config (the `GitSnapshot` in plan §8). It never needs to write, fetch, or check out.

## Decision
- Use `git2` 0.21.0 with vendored libgit2 and network features off (no HTTPS, no SSH).
- Never spawn `git` or any other process from DORMBIT.
- Open repositories read-only in intent: no operation that writes to `.git`, no checkout, no fetch, no index refresh written back to disk.
- Flag `core.fsmonitor`, `core.hooksPath`, `filter.*`, `core.sshCommand`, and `credential.helper` by exact key in the snapshot, the restore report, and the brief's pre-run checks.
- Hide credentials in remote URLs (`remotes_redacted`).
- Do not capture or restore `.git/hooks/`, lock files, or fsmonitor state (CAP-5, RST-6).
- Keep a small ref reader in `dormbit-core` for HEAD and refs, so the demo (no `git2` in WebAssembly) and the post-restore check share one implementation ([ADR-003](ADR-003-vfs-core.md)).

## Alternatives considered
- **git CLI with a sanitized environment** (`GIT_CONFIG_NOSYSTEM`, `-c core.fsmonitor=`, empty `core.hooksPath`). Rejected. Every new git version can add a config key that runs a command. The blocklist would be ours to keep current, and one miss is code execution.
- **gitoxide (`gix`).** Pure Rust and a possible wasm path, but it is not the plan's choice and its status support is newer. Revisit only if libgit2 causes build or correctness problems.
- **Parse `.git` by hand.** Rejected beyond HEAD and refs. Packfiles, the index format, and worktrees are too much to reimplement safely in five weeks.

## Consequences
- libgit2 is C code built from source with the Command Line Tools. It is native-only, so the demo uses a Git snapshot precomputed from the Dockline fixture.
- libgit2 may not support every repository format or extension the git CLI does. Unsupported repositories are reported as Unsupported, never guessed at.
- Task 17 tests cover dirty, detached, empty, linked-worktree, submodule, LFS, and risky-config repos. They should include a fixture whose risky config points at a marker-writing script, and assert the marker never appears.
- The native working-tree status check after restore (RST-4) uses `git2`. The HEAD and refs check uses the core ref reader in both builds.
