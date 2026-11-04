# Security policy

## Supported versions

Only the latest release of DORMBIT is supported. There are no backports to earlier builds.

## Reporting a vulnerability

Report privately through GitHub's private vulnerability reporting on
[`AmirmLotfy/dormbit`](https://github.com/AmirmLotfy/dormbit/security/advisories/new). Please do
not open a public issue for a vulnerability.

We aim to acknowledge a report within a week. If a report is accepted, we fix it in the latest
release and credit you unless you ask otherwise.

## What DORMBIT protects

- Your project files, and the accuracy of what DORMBIT reports about them.
- Agent transcripts and any secrets inside your projects (sensitive files are excluded from
  capsules, and content is scanned before it is written).
- The integrity of capsules and of restored output.

DORMBIT is local-first: no account, no telemetry, no network calls (NAT-2). It never runs your
code or the `git` CLI, and it never deletes or modifies your source folders.

## Stated limits (v1)

These are real limits, stated plainly rather than hidden:

- **No OS sandbox.** Scope enforcement happens inside the app through `cap-std` directory
  handles. v1 does not add a separate operating-system sandbox, so a bug in the app's own
  enforcement is not contained by the OS.
- **No encryption.** Capsules and local state are not encrypted in v1. Protect them with your
  own disk encryption (for example FileVault). Because of this, sensitive files are excluded
  from capsules rather than encrypted inside them.
- **Access times.** Reading your files to discover and capture them can update their access
  times (atime). DORMBIT never changes file contents or modification times of your sources.

## Integrity, not authorship

A capsule's SHA-256 hashes show that its contents have not changed since capture
(**integrity**). They do not prove **who** created the capsule (VER-2). DORMBIT's verification
tells you a capsule is intact, not that it came from a particular person, and the app never
calls the capsule seal a signature.

## Scope

This policy covers the DORMBIT application, its web demo, and this repository. It does not cover
third-party dependencies, which are tracked with pinned versions, `cargo deny`, and
`pnpm audit` in CI.
