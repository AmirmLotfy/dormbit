# Research record

Collected Nov 3–4, 2026 during Plan mode and Task 4 (P00-T04). Sources are linked inline. Product names appear here only to describe the landscape; DORMBIT's video and UI use no third-party logos.

## 1. Competitors and adjacent tools

| Tool                                                                                                       | What it does (from its own docs)                                                                                         | Overlap with DORMBIT                      | What it doesn't do                                                                                                     |
| ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| [Agent Sessions](https://github.com/jazzyalex/agent-sessions) (macOS, open source)                         | Searches local conversations from 16 coding agents, resumes supported CLI sessions, tracks quota burn; index stays local | Reads agent histories locally             | No project preservation, capsule, or restore proof                                                                     |
| [Claude Code History Viewer](https://github.com/jhlee0409/claude-code-history-viewer) (Tauri, open source) | Browses and analyzes conversations from 31 assistants, offline, desktop or headless server                               | Same stack (Tauri), agent history reading | Same gap                                                                                                               |
| [SpecStory](https://specstory.com/)                                                                        | Saves the conversations behind your code as you work and reuses them in later tasks                                      | Preserves agent context                   | Forward-only; doesn't shelve a dormant project or verify a restore                                                     |
| Git clients, `git bundle`                                                                                  | Repository state and history                                                                                             | Git inspection                            | Gitignored-but-needed files (local notes, data) are lost; no handoff; no restore verification                          |
| Time Machine, Arq, restic, Borg                                                                            | Verified backups                                                                                                         | Integrity checking                        | Not project-aware; no capture plan, no development context                                                             |
| [kondo](https://github.com/tbillington/kondo), npkill, disk cleaners                                       | Remove `node_modules`, `target`, build output; kondo's README describes itself as "essentially `rm -rf` with a prompt"   | Rebuildable-folder classification         | Destructive by design; DORMBIT only reads and uses similar classification to exclude rebuildable output from a capsule |

**Conclusion.** Every individual capability exists somewhere. No tool found combines (1) an evidence-graded reconstruction of a project's state, (2) an approved capture into a verifiable capsule, (3) a transactional, independently verified restore, and (4) a handoff that keeps "files restored" separate from "context inferred", "session resumed", and "builds". DORMBIT claims the combination, not any single piece.

## 2. Local AI-agent storage (for P1 adapters)

| Tool                | Documented storage                                                                                                                                                                                                                                      | Retention                                                  | Credentials nearby                                     | Adapter plan                                                                                                                                          |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- | ------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| Claude Code         | Plaintext transcripts at `~/.claude/projects/<project>/<session>.jsonl`; subagent transcripts under `<session>/subagents/`. They contain file contents, command output, and request data. ([docs](https://code.claude.com/docs/en/claude-directory.md)) | Deleted after `cleanupPeriodDays`, **default 30 days**     | `~/.claude/.credentials.json`                          | Metadata only, with consent and a separate folder grant; never open the credentials file; show a "context at risk" note citing the documented default |
| Codex               | State under `CODEX_HOME` (default `~/.codex`); `history.jsonl` when history persistence is on ([docs](https://learn.chatgpt.com/docs/config-file/config-advanced))                                                                                      | Configurable (`history.max_bytes`, `persistence = "none"`) | `auth.json` when file-based credential storage is used | Confirm per-session layout from official sources first; otherwise metadata from `history.jsonl` or report Unsupported                                 |
| Kiro                | Export Chat zip contains `session.json`, `messages.jsonl`, `sub-executions/*.jsonl` ([docs](https://kiro.dev/docs/ide/chat/chat-export.md))                                                                                                             | User-controlled                                            | —                                                      | Read `session.json` metadata from user-exported zips only; the local session store is undocumented and not read in v1                                 |
| Cursor              | Not researched as a documented local format                                                                                                                                                                                                             | —                                                          | —                                                      | Report Unsupported; read in-repo `.cursor/rules` as ordinary project files                                                                            |
| In-repo agent files | `.kiro/specs`, `.kiro/steering`, `AGENTS.md`, `CLAUDE.md`, `.cursor/rules`, `.specstory/`                                                                                                                                                               | Part of the project                                        | —                                                      | P0: parsed as normal project evidence                                                                                                                 |

## 3. Platform facts that shaped the design

- **Tauri capabilities are not the security boundary.** Tauri's docs state capabilities don't protect against "incorrect scope checks in the command implementation" ([capabilities](https://tauri.app/security/capabilities/)). DORMBIT's Rust scope layer (cap-std handles) is the real boundary.
- **macOS WebDriver testing** for Tauri works through `@wdio/tauri-service` with an embedded driver plugin; `tauri-driver` alone doesn't support macOS ([webdriver](https://tauri.app/develop/tests/webdriver/)).
- **Ad-hoc signing** (`signingIdentity: "-"`) is supported and required on Apple Silicon for downloaded apps ([macOS signing](https://tauri.app/distribute/sign/macos/)). Notarization needs Apple Developer credentials (not used; decision in the master plan).
- **Command Line Tools are enough** for Tauri desktop builds ([prerequisites](https://tauri.app/start/prerequisites/)); confirmed by the Task 3 spike.
- **Playwright MCP** supports `--isolated` profiles and `--extension` to drive the user's own Chrome; its origin allow/block lists are explicitly _not_ a security boundary ([README](https://github.com/microsoft/playwright-mcp)).

## 4. Kiroween 2026 rules checklist

From [kiroween.devpost.com](https://kiroween.devpost.com/), [/rules](https://kiroween.devpost.com/rules), [/details/faqs](https://kiroween.devpost.com/details/faqs), read Nov 3, 2026.

- [ ] Built during the Submission Period (Oct 30, 2026 09:00 PT – Dec 4, 2026 14:00 PT). Planning date Nov 3 recorded in `docs/status/environment.md`.
- [ ] Working app URL (web demo) accessible free and without restriction through the Judging Period (Dec 16, 2026 – Jan 29, 2027).
- [ ] Public **GitHub** repo (not GitLab/Bitbucket) with all source, assets, and instructions.
- [ ] Open-source license detectable in the repo's About section.
- [ ] `/.kiro` at the repo root, not gitignored.
- [ ] English text description of features and functionality.
- [ ] Demo video under 3:00, public on YouTube, showing the app on its target device, no third-party trademarks or copyrighted music without permission.
- [ ] Kiro usage write-up (specs, hooks, steering, custom agents, MCP, PBT, powers — only what was actually used).
- [ ] Exactly one category: **Frankenstein**.
- [ ] Disclose any pre-existing code or work (none planned beyond standard libraries and templates).
- [ ] Third-party SDKs/data used within their licenses.
- [ ] Optional bonus prizes need extra fields (blog post, social post, startup/student details). Ask the user in Task 14.
- [ ] Credits: request by Dec 1, 12:00 PT; redeem by Dec 4. (User action.)
- [ ] No edits after the deadline; submit early (target Dec 2, 12:00 PT).

## 5. Kiro Powers review (Task 4)

Catalog reviewed at [kiro.dev/powers](https://kiro.dev/powers) on Nov 4, 2026. Candidates considered:

- Browser automation (Amazon Nova Act Browser Automation, ScoutQA): Playwright MCP already covers web testing and screenshots without a cloud account.
- Security scanning (Snyk, Aikido, SonarQube, Checkmarx, Sonatype): each needs a third-party account; gitleaks, cargo-deny, and `pnpm audit` cover the plan's needs locally.
- Context7 (library docs): useful but optional; official docs are fetched directly.
- Netlify deploy: hosting is GitHub Pages.

**Decision:** no Power installed now. Revisit only if one replaces planned work. The Kiro write-up will say none were used unless that changes.
