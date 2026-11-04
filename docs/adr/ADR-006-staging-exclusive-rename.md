# ADR-006: Restore writes into a staging folder and commits with `renameatx_np(RENAME_EXCL)`

- **Status:** Accepted, Nov 4, 2026
- **Task:** P01-T02
- **Evidence:** [docs/evidence/P00-T03-spikes.md](../evidence/P00-T03-spikes.md) (spike a1, `spikes/fs-spike`)
- **Related:** RST-1 to RST-6; PROP-4, PROP-5, PROP-8; [threat model](../security/threat-model.md) T-03, T-06, T-18, T-20

## Context

Restore must never overwrite anything (RST-6, PROP-4). A failure at any step must leave no final folder (PROP-5). After a crash, the next launch must be able to recover (RST-5). The destination must never be visible half-written.

Checking "does the target exist?" and then writing is a race: another process can create the target in between. Plain `rename(2)` makes it worse, because renaming a directory onto an existing **empty** directory replaces it.

## Decision

1. Pre-checks (RST-2) run first, including that the destination volume supports exclusive rename. If not, restore fails with `RESTORE_VOLUME_UNSUPPORTED`.
2. Create a fresh staging folder **in the same parent** as the destination, so the commit is a same-volume rename. Write an owner token file into it and record the op in the restore journal.
3. Write files with create-new semantics through `cap-std`. Strip setuid, setgid, and sticky bits; keep executable bits. Re-hash while writing. `sync_all` each file (`F_FULLFSYNC` on macOS).
4. Verify the staged tree against the manifest.
5. Commit with one call: `renameatx_np(dirfd, staging, dirfd, final, RENAME_EXCL)` on the parent's directory handle. If anything exists at `final`, the kernel refuses with `EEXIST` and nothing changes.
6. After commit, re-hash the restored files independently and check Git (RST-4).
7. The journal moves through `Staged → Written → Verified → Committed → Cleaned`. On the next launch, an unfinished op is offered for recovery. DORMBIT deletes a staging folder only after checking its owner token. That is the only deletion DORMBIT ever performs.

**Spike evidence** (Nov 4, 2026, macOS 26.6.2, APFS, `cargo test`: 7 passed, 0 failed):

| Test                                                               | Shows                                                                     |
| ------------------------------------------------------------------ | ------------------------------------------------------------------------- |
| `exclusive_rename_commits_when_target_absent`                      | A staged folder moves into place; staging no longer exists                |
| `exclusive_rename_refuses_existing_target_and_leaves_it_untouched` | `EEXIST`; the existing folder's contents are unchanged; staging is intact |
| `exclusive_rename_refuses_existing_file_target`                    | Also refuses when the target is a file                                    |
| `cap_std_rejects_parent_traversal`                                 | `../` reads and writes fail; nothing written outside                      |
| `cap_std_rejects_absolute_paths`                                   | Absolute paths fail                                                       |
| `cap_std_rejects_symlink_escape`                                   | Absolute and relative symlinks pointing outside fail for read and write   |
| `cap_std_allows_symlink_inside_root`                               | A symlink that stays inside the root still works                          |

## Alternatives considered

- **Write directly into the destination.** Rejected. A crash leaves a half-restored project that looks real.
- **Existence check, then `rename(2)`.** Rejected. Race between check and rename, and `rename(2)` replaces an existing empty directory.
- **`renamex_np` (path-based).** Rejected. It resolves paths from the process's working directory instead of the granted directory handle, so it steps outside the `cap-std` scope.
- **Stage in a temp folder elsewhere (for example `/tmp`).** Rejected. A cross-volume move is a copy, not an atomic rename.
- **Restore into an existing folder with per-file create-new.** Rejected. Leaves partial state on failure and mixes restored files with existing ones.

## Consequences

- This is the only `unsafe` code in the project: one `renameatx_np` call in `dormbit-fs`, with a `SAFETY` comment, a dedicated review, and the tests above.
- Volumes without exclusive rename (for example network shares or FAT/exFAT drives) are refused with `RESTORE_VOLUME_UNSUPPORTED`. The plan names APFS and HFS+ as supported.
- Restore needs free space for the full restored size on the destination volume, checked up front (`RESTORE_INSUFFICIENT_SPACE`).
- On a case-insensitive volume, `RENAME_EXCL` also refuses a target that differs only in case. This backs up the case-fold pre-check (PROP-8).
- A crash can leave a hidden staging folder next to the destination until the next launch offers recovery.
- `MemVfs` and the fault-injecting `Vfs` must implement `rename_exclusive` with the same refuse-if-exists behavior, so PROP-4 and PROP-5 hold in the demo and in tests ([ADR-003](ADR-003-vfs-core.md)).

## Open questions

- **How to detect volume support without more `unsafe`.** `statfs` or `getattrlist` (`VOL_CAP_INT_RENAME_EXCL`) would add `unsafe` calls. One option keeps the single call site: inside the destination parent, create two empty probe folders and exclusive-rename one onto the other. `EEXIST` means supported; `ENOTSUP` or `EINVAL` means unsupported. Both probes are DORMBIT-owned and removed afterward. To be settled in the `transactional-restore` spec.
- **Staging folder name.** The plan doesn't fix it. It should be hidden and derived from the op ID (for example `.dormbit-staging-<op_id>`), set in the `transactional-restore` spec.
