---
inclusion: always
---

# Product rules (DORMBIT)

DORMBIT finds dormant projects, seals each into a verifiable capsule, restores it into a new folder, and writes an evidence-backed resurrection brief. Tagline: "Pick up where your project stopped."

- **Truthfulness.** Never claim more than the evidence shows. Every brief line cites a finding ID. Unknown stays "unknown". UI copy, the README, and the Devpost text follow the same rule.
- **The four outcomes (BRF-2) are always reported separately, with these exact labels:**
  1. Files restored and verified.
  2. Context reconstructed (inferred from evidence).
  3. AI session resumed: not performed.
  4. Builds/runs: unknown (never executed).
- **No deletion.** DORMBIT never deletes or modifies the user's source folders. Cleanup touches only its own staging folders.
- **No code execution.** DORMBIT never runs project code, scripts, build tools, or the git CLI. Git is read through libgit2.
- **Local-first.** No account, no telemetry, no network calls. Capsules and state stay on the user's disk.
- Agent histories (Claude Code, Codex, Kiro export) are read for metadata only, after explicit per-adapter consent.
- The web demo is synthetic and says so on every screen: "Synthetic Demo — No Local Files Accessed".
