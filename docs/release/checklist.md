# Release checklist

Mirrors the Release_Gate in the `release-and-submission` spec. Every box needs a link to a recorded result before it is checked.

## Gate (must all be true before any release task)
- [ ] Latest evidence record is green (`scripts/evidence/run.sh` output linked)
- [ ] gitleaks finds nothing on the full history
- [ ] `LICENSE` exists and GitHub shows MIT in the About section
- [ ] `.kiro/` is tracked and not ignored (`git check-ignore -v .kiro` prints nothing)
- [ ] `cargo deny check` and `pnpm audit --audit-level high` are clean
- [ ] Security review verdict is `APPROVED` (`docs/security/review.md`)

## Build
- [ ] Versions agree across Cargo.toml, tauri.conf.json, package.json
- [ ] `codesign -dv` shows `Signature=adhoc`; `codesign --verify --deep --strict` passes
- [ ] Release binary contains no test features
- [ ] DMG and `SHA256SUMS` built; architecture stated (Apple Silicon only unless the universal build ships)

## Verify
- [ ] Browser-downloaded DMG (quarantine set) opens via the documented Gatekeeper steps
- [ ] Full native workflow passes on the Dockline fixture with fresh app data
- [ ] Production demo: 12-step E2E green; no third-party requests
- [ ] Daily smoke test scheduled through Jan 29, 2027
- [ ] Fresh clone builds by following the README

## Publish (user approval for each)
- [ ] v1.0.0 tag and release
- [ ] Repo topics, website, About text
