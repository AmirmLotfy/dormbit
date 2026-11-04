---
inclusion: manual
---

# Kiroween 2026 rules checklist and approval gates

Source: master plan §0 and §22 (kiroween.devpost.com rules and FAQ). Category: **Frankenstein**.

## Rules checklist

- [ ] Submission Period: Oct 30, 2026 9:00 AM PT to **Dec 4, 2026 2:00 PM PT**. No late entries, no edits after the deadline. Target: submit Wed Dec 2, 12:00 PM PT.
- [ ] Judging runs Dec 16, 2026 to Jan 29, 2027: the app and demo stay available and free the whole time.
- [ ] Only work from the Submission Period counts; disclose anything older (planning happened Nov 3, 2026).
- [ ] A working app URL (GitHub Pages demo at `https://amirmlotfy.github.io/dormbit/`).
- [ ] An English description.
- [ ] A public YouTube video under 3:00 showing the app on its target device; no third-party trademarks or copyrighted music without permission.
- [ ] A public GitHub repo with an open-source license visible in the About section (MIT), and `/.kiro` at the root, never gitignored (`git check-ignore .kiro` prints nothing).
- [ ] A Kiro usage write-up (specs, steering, hooks, agents, MCP), backed by `docs/kiro/usage-log.md`.
- [ ] Exactly one category.
- [x] Registered on Devpost; 4,000 Kiro credits requested and in use (Nov 4, 2026).

## Approval gates (§22): stop and ask the user before

- installs outside the repo
- writes outside the workspace
- the repo target
- the release (publishing a GitHub release)
- macOS Screen Recording permission
- enabling the Playwright Chrome extension (`playwright-chrome`)
- making the video public
- the final Devpost submit

Everything else is routine and proceeds without asking.
