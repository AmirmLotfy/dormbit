# ADR-010: Ship the macOS app with an ad-hoc signature, without notarization

- **Status:** Accepted, Nov 4, 2026
- **Task:** P01-T02
- **Decision owner:** the user, recorded at plan approval (plan "Decisions recorded at approval")
- **Evidence:** [docs/evidence/P00-T03-spikes.md](../evidence/P00-T03-spikes.md) (spike a2)
- **Related:** Tasks 10, 49, 50, 52, 59; risk register "Gatekeeper friction"

## Context

Apple Silicon Macs require downloaded apps to carry a code signature. Distributing without Gatekeeper warnings requires a Developer ID certificate and notarization, which need Apple Developer credentials. The user chose not to use them for this project.

The hackathon requires a working app URL and a video showing the app on its target device. Judges must be able to test without friction.

## Decision

- Sign with an ad-hoc signature: `signingIdentity: "-"` in the Tauri config. No notarization. This is the user's choice.
- **The web demo is the main path for judges.** It runs the same Rust core in the browser with no install ([ADR-009](ADR-009-github-pages.md)).
- The DMG and `SHA256SUMS` are published on GitHub Releases. Apple Silicon only unless the universal binary (P1) ships, and the release notes say which.
- The README and the Devpost testing notes give the Gatekeeper steps below and build-from-source instructions.

**Spike evidence:** the Tauri 2.12.5 spike bundle built with Command Line Tools only. `codesign -dv` reported `Signature=adhoc` and `flags=0x10002(adhoc,runtime)`, and `codesign --verify --deep --strict` passed.

### Judge-facing Gatekeeper steps

These go into the README and the Devpost testing notes. Task 50 checks them on a browser-downloaded DMG (so the quarantine flag is set) before they are published.

1. Download `DORMBIT_<version>.dmg` and `SHA256SUMS` from the GitHub release.
2. Optional: in Terminal, in the download folder, run `shasum -a 256 -c SHA256SUMS`. It should print `OK` for the DMG.
3. Open the DMG and drag DORMBIT to Applications.
4. Open DORMBIT from Applications. macOS says it can't verify the app and doesn't open it. Click **Done**.
5. Open **System Settings → Privacy & Security**. Scroll to the **Security** section. Next to the message about DORMBIT, click **Open Anyway**, then confirm with your password or Touch ID.
6. Open DORMBIT again and click **Open** in the dialog. macOS remembers this choice.

If you'd rather not change this setting, use the web demo, or build from source with the README's instructions.

## Alternatives considered

- **Developer ID signing plus notarization.** Removes the warning. Rejected by the user: it needs paid Apple Developer credentials. Listed as P2 (notarized builds).
- **Unsigned app.** Rejected. Apple Silicon won't run unsigned arm64 code.
- **Asking judges to remove the quarantine attribute in Terminal.** Rejected. It teaches people to bypass a security control, and the System Settings route is Apple's documented path.
- **Web demo only.** Rejected. The rules ask for the app on its target device, and the native app is the product.

## Consequences

- Gatekeeper friction is likely (risk register: High likelihood, Medium impact). The web demo and clear steps reduce it.
- An ad-hoc signature carries no publisher identity. `SHA256SUMS` shows the DMG matches the release page; it doesn't prove who built it.
- The hardened runtime flag is present (spike `flags=...runtime`).
- Task 49 checks `codesign -dv` shows the ad-hoc signature. Task 50 runs the full workflow from a browser-downloaded DMG with fresh app data. Testing on a separate macOS user account is optional and done by the user.
- The exact Gatekeeper wording and button names can change between macOS versions. Task 50 re-checks them on macOS 26.6.2, and the README states the macOS version they were checked on.
