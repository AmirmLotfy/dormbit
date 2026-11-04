# ADR-008: Agent adapters read metadata only

- **Status:** Accepted, Nov 4, 2026
- **Task:** P01-T02
- **Related:** AGT-1, AGT-2, INT-2, DSC-3; Phase 07 (P1, cut first); [research.md §2](../product/research.md#2-local-ai-agent-storage-for-p1-adapters); [threat model](../security/threat-model.md) T-13, T-16

## Context

Agent context is part of what goes missing when a project pauses. But external agent stores are the most sensitive data DORMBIT could touch:

- Claude Code keeps plaintext transcripts at `~/.claude/projects/<project>/<session>.jsonl`. They contain file contents, command output, and request data. They are deleted after `cleanupPeriodDays` (default 30 days). Credentials (`~/.claude/.credentials.json`) sit in the same tree.
- Codex keeps state under `CODEX_HOME` (default `~/.codex`), with `history.jsonl` when history persistence is on and `auth.json` when file-based credential storage is used. The per-session layout isn't documented.
- Kiro documents its Export Chat zip (`session.json`, `messages.jsonl`), but not its local session store.
- No documented local format was found for Cursor history.

Reading message content would put secrets and other people's code into findings, briefs, logs, or capsules. Formats also change without notice.

## Decision

- **Consent and a separate grant.** Each adapter needs explicit consent and its own folder grant, separate from project grants (AGT-1).
- **Metadata only.** Extract session ID, working directory, timestamps, branch, and tool version. Discard message content as it is parsed; never store it.
- **Never open credential files.** A deny-list (`.credentials.json`, `auth.json`, and similar) is checked before any open.
- **Per-format status.** Each file format is reported Supported or Unsupported. Unknown formats are Unsupported, not guessed.
- **Honest labels.** Session references say "not resumed by DORMBIT" (AGT-2).
- **Per tool:**
  - Claude Code: the documented path; sessions matched by working directory; a "context at risk" note citing the documented 30-day default. The user's own setting shows as Unknown.
  - Codex: confirm the layout from official sources first; otherwise `history.jsonl` metadata or Unsupported.
  - Kiro: `session.json` from user-exported zips only. The local store is not read in v1.
  - Cursor: Unsupported.
- **In-repo agent files are P0 project evidence**, not adapters: `.kiro/specs`, `.kiro/steering`, `AGENTS.md`, `CLAUDE.md`, `.cursor/rules`, and `.specstory/` (presence, counts, and dates only).
- **Never captured.** External transcripts never go into a capsule. Manifests carry only `agent_refs` metadata.

## Alternatives considered

- **Capture full transcripts into the capsule.** Rejected. Moves plaintext secrets and code into an unencrypted file ([ADR-007](ADR-007-no-encryption-v1.md)).
- **Summarize transcripts (cloud or local AI).** Rejected for v1. Cloud breaks local-only. Opt-in local AI summaries are P2.
- **No adapters.** Possible, and it is the first P1 cut. Without them, DORMBIT can't warn that Claude Code context is about to expire.
- **Read undocumented stores (Kiro local, Cursor).** Rejected. No documented format to hold parsers to, so findings would be guesses.

## Consequences

- The brief can cite that a session exists and when, not what was said. That matches "Context reconstructed (inferred from evidence)" and "AI session resumed: not performed".
- Parsers must be tolerant: a changed format yields Unsupported, not a crash or a wrong finding.
- Tests use synthetic stores only. The development agent is denied `~/.claude/**` and `~/.codex/**` by workspace permissions. If field names can't be confirmed from docs, the user runs a keys-only schema probe themselves.
- If schedule slips, this whole phase is cut first, and DSC-3's "agent context available" covers in-repo files only.
