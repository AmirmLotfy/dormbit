# ADR-007: No encryption in v1

- **Status:** Accepted, Nov 4, 2026
- **Task:** P01-T02
- **Related:** CAP-2, CAP-3, AGT-1; [capsule format §4.8](../architecture/capsule-format.md#48-limitations); [threat model](../security/threat-model.md) §6–7

## Context

The original brief asked to "encrypt sensitive data". The plan's assessment (§1) decided against it for v1:

- Encryption needs key management. With age plus a Keychain-held key, a lost key or a new Mac means a lost project. That is worse than the problem DORMBIT solves.
- It adds format complexity (key wrapping, per-object encryption) to the most security-critical code, in a five-week schedule.
- An encrypted capsule can't be listed with `tar -tvf` or recovered by hand, which ADR-002 relies on.
- The safer v1 choice is to keep sensitive data out of capsules in the first place.

## Decision

- Capsules are not encrypted in v1.
- Sensitive files (`.env*` except example files, private keys, `.pem`, `.p12`, credential files) are always excluded, with no override in v1 (CAP-2).
- File contents that look like secrets trigger a warning that must be acknowledged before capture (CAP-3).
- External agent transcripts are never captured. Adapters read metadata only (AGT-1, [ADR-008](ADR-008-metadata-only-adapters.md)).
- The app says so: every manifest's `limitations` includes "The capsule is not encrypted.", and the Privacy screen, plan, and verification report state it.
- Encrypted capture (age plus Keychain) is P2.

## Alternatives considered

- **age with a passphrase.** Rejected for v1. A forgotten passphrase loses the project, and passphrase UX plus strength checks add scope.
- **age with a key in the Keychain.** Rejected for v1. Ties capsules to one Mac's Keychain unless keys are exported, and the export flow is its own security feature. This is the P2 plan.
- **Encrypted disk images (`hdiutil`).** Rejected. Requires running an external process, which DORMBIT avoids, and breaks native/WebAssembly parity.
- **Zip with AES.** Rejected. ADR-002 already rejected zip, and zip encryption leaves names and sizes in the clear.

## Consequences

- Anyone who gets a capsule file can read everything in it, including approved gitignored notes and data files.
- Users who need confidentiality should store capsules on an encrypted volume (FileVault or an encrypted external drive). The app can say this; it doesn't enforce it.
- Secrets in captured `.git` history are a residual risk ([threat model §7](../security/threat-model.md#7-residual-risks)).
- Adding encryption later means a new capsule format version, not an optional v1 field ([capsule format §10](../architecture/capsule-format.md#10-versioning-and-compatibility)).
