---
inclusion: manual
---

# Release steps

Owned by the `release` agent and the `release-and-submission` spec. Full checklist: `docs/release/checklist.md`.

1. **Gate first.** Run the `release-gate` skill: the newest Evidence record is green and no code changed since it, gitleaks finds nothing, `LICENSE` exists, and `git check-ignore .kiro` prints nothing.
2. **Build.** `pnpm --filter @dormbit/desktop tauri build` (ad-hoc signature `-`, ADR-010). Check with `codesign -dv --verbose=4` and `spctl -a -vv` (expected "rejected": not notarized).
3. **Package.** `.dmg` via `hdiutil`; `shasum -a 256` for every artifact; record the hashes in the release notes.
4. **Web demo.** `pages.yml` builds the WebAssembly core and `apps/demo`, deploys to GitHub Pages, and the smoke test runs against the live URL.
5. **Publish** the GitHub release only after the user approves (approval gate).
6. **Notes** state what is real and what isn't: ad-hoc signed (Gatekeeper steps: System Settings → Privacy & Security → Open Anyway), the demo is synthetic, and the four outcomes.
7. Update `docs/status/STATUS.md` to Released and write the phase gate note.
