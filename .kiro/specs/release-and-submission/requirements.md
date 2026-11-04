# Requirements Document

## Introduction
This spec covers everything from the final security campaign to the Kiroween submission: master plan Phase 10 (P10, Tasks 42-45), Phase 12 (P12, Tasks 49-51), Phase 13 (P13, Tasks 52-53), Phase 14 (P14, Task 54), Phase 15 (P15, Tasks 55-57), Phase 16 (P16, Task 58), Phase 17 (P17, Tasks 59-60), and Phase 18 (P18, Task 61). It formalizes the §7 "Proven by" column, the §15 property counts, the §16-17 deployment and GitHub rules, the §18-19 automation rules, the §0 hackathon requirements, the §21 timeline, and every approval gate in §22. It owns no property-based tests (§12); it runs the ones the other specs define at campaign counts.

## Glossary
- **Agent**: the Kiro main agent, or a custom agent (`qa`, `release`, `rust-security`, `reviewer`) acting for it.
- **User**: the repo owner, who gives every approval in §22.
- **Reviewer**: the `reviewer` custom agent. Its final line is exactly `APPROVED` or `NEEDS_CHANGES`.
- **Approval record**: a line in the Approvals section of `docs/status/STATUS.md` with the Pacific Time date and time, the action approved, and the User's words.
- **Pacific Time (PT)**: `America/Los_Angeles` time, which the hackathon deadlines use.
- **Evidence_Runner**: `scripts/evidence/run.sh` from the foundation spec. A result counts only if recorded by it or by CI.
- **Evidence record**: the JSON file the Evidence_Runner writes to `docs/evidence/runs/`.
- **Task evidence note**: `docs/evidence/PXX-TYY-<slug>.md`, which summarizes a task's results and links its Evidence records and CI runs.
- **Gate note**: `docs/status/gates/PXX.md` for a phase: what works, what's missing, what's next.
- **Malicious fixture**: an input built to attack DORMBIT: a capsule, a restore request, a filesystem tree, or an IPC argument.
- **Fixture_Catalog**: `fixtures/malicious/catalog.json`, the list of Malicious fixtures with their expected results.
- **Security_Suite**: the Malicious fixtures, the property tests PROP-1 to PROP-12, the IPC fuzz tests, and the CSP tests.
- **Property_Campaign**: one run of every property test at 10,000 cases.
- **Integration_Suite**: CLI smoke tests on 6 project types, crash and recovery tests, and read-only source checks.
- **Synthetic project**: a small project tree built by a test at run time under `.scratch/`, never installed, built, or run.
- **Native_E2E**: the WebdriverIO suite that drives the native app through `@wdio/tauri-service` and the embedded driver plugin (P1).
- **Test feature**: a Cargo feature that exists only for tests, such as `test-picker`, `test-faults`, or `e2e-webdriver`.
- **Supply_Chain_Checks**: `cargo deny check`, `pnpm audit`, and a full-history gitleaks scan.
- **IPC_Fuzz**: tests that send random JSON arguments to every allowlisted command.
- **Security_Review**: the document `docs/security/review.md`.
- **Release_Workflow**: `.github/workflows/release.yml`.
- **Release_Build**: the signed `DORMBIT.app` and the DMG that contains it.
- **DMG**: the disk image published on GitHub Releases.
- **SHA256SUMS**: the file listing the SHA-256 of each release asset, in `shasum -a 256` format.
- **Clean_Install_Run**: the Task 50 run that installs the DMG as a new user would.
- **Clean-install checklist**: `docs/release/clean-install-checklist.md`.
- **Pages_Workflow**: `.github/workflows/pages.yml`, created by the `judge-demo` spec and extended here.
- **Production demo**: the judge demo deployed at `https://amirmlotfy.github.io/dormbit/` with the production channel label.
- **Smoke_Workflow**: `.github/workflows/smoke.yml`, the daily check of the Production demo.
- **Repo_Audit**: the Task 52 check of the public repo's contents.
- **Fresh_Clone_Run**: a clone of the public repo into an empty folder, followed by the README's build-from-source steps.
- **GitHub_Release**: the `v1.0.0` release on `github.com/AmirmLotfy/dormbit`.
- **Screenshot_Set**: the 6-8 images selected in Task 54, stored in `docs/media/`.
- **Video_Script**: `media/script/script.md`.
- **Shot list**: `media/script/shotlist.md`, the timed list of segments.
- **Media_Pipeline**: the scripts in `scripts/media/` that render Title cards, assemble the video, normalize audio, and make captions.
- **Title card**: an HTML page in `media/cards/` rendered to PNG.
- **Final cut**: `media/final/dormbit-demo.mp4`.
- **Caption file**: `media/final/dormbit-demo.en.srt`.
- **Thumbnail**: `media/final/thumbnail.png`.
- **Video_Verifier**: `scripts/media/verify-video.sh`, which runs the automated video checks.
- **Video report**: `docs/release/video-verification.md`.
- **YouTube_Video**: the uploaded demo video.
- **Devpost_Content**: `docs/submission/devpost.md`, the final submission text.
- **Claims_Check**: `scripts/release/check-claims.mjs`, which checks that the Devpost_Content cites only real paths and recorded numbers.
- **Devpost_Submission**: the project entry on kiroween.devpost.com.
- **Browser_Automation**: the `playwright-chrome` MCP server in `--extension` mode, driving the User's own signed-in Chrome.
- **Manual steps**: `docs/submission/manual-steps.md`, exact click-by-click steps the User follows when Browser_Automation isn't used or fails.
- **Logged-out check**: a Playwright test run in a fresh browser context with no cookies.
- **Release_Gate**: the `release-gate` hook (or skill, if the hook payload can't identify the task) from the foundation spec.
- **Final_Audit**: the Task 61 check of every deliverable, run by `scripts/release/final-audit.sh` plus the manual items in its checklist.
- **Final report**: `docs/status/FINAL_REPORT.md`.
- **CI_Workflow**: `.github/workflows/ci.yml` from the foundation spec.

## Requirements

### Requirement 1: Malicious fixture catalog (P10-T01, Task 42)

**User Story:** As a judge, I want a named list of attacks with the exact error each one must produce, so that "no malicious fixture is accepted" is a checkable claim.

#### Acceptance Criteria

1. THE Fixture_Catalog SHALL list at least 25 Malicious fixtures that each have an `id`, a `category` (`capsule`, `restore`, `scope`, or `ipc`), the file or generator that builds it, and exactly one expected ErrorCode.
2. THE Fixture_Catalog SHALL include at least one fixture for each of: path traversal, absolute path, symlink entry, duplicate entry, truncation, missing trailer, archive bomb, bad marker, unknown version, manifest/object mismatch, corrupted object, NFC collision, case-fold collision, existing target, path component over 255 bytes, path over 1024 bytes, control characters in a path, symlink escape from a grant, a `..` request path, and a malformed or unknown IPC ID.
3. WHEN the Security_Suite runs, THE Security_Suite SHALL feed every catalog fixture to the component it targets and check that the result is its expected ErrorCode.
4. IF a fixture is accepted or produces a different code, THEN THE Security_Suite SHALL fail and print the fixture `id`, the expected code, and the actual result.
5. THE Fixture_Catalog SHALL list fixtures whose expected result isn't an error (for example `.git/hooks/*` not restored, or risky Git config flagged) in a separate `outcomes` list, and those SHALL NOT count toward the 25.
6. WHEN the Security_Suite runs, every committed fixture file SHALL equal its generator's fresh output.
7. THE Security_Suite SHALL create fixtures that need symlinks, FIFOs, or other special files at test time, and SHALL NOT commit them.
8. THE Fixture_Catalog SHALL contain no real secret; any secret-shaped test string SHALL be generated at run time.

### Requirement 2: Property campaign (P10-T01, Task 42)

**User Story:** As a security reviewer, I want every property run at a high case count with recorded results, so that the safety claims rest on more than a quick CI pass.

#### Acceptance Criteria

1. WHEN the Property_Campaign runs locally, THE Agent SHALL run every property test for PROP-1 to PROP-12 with `PROPTEST_CASES=10000` and `FC_NUM_RUNS=10000` through the Evidence_Runner.
2. THE CI_Workflow from the foundation spec SHALL run the same property tests with 512 cases on every push.
3. WHEN a property test finds a counterexample, THE Agent SHALL commit it as a regression case and record the property ID, the minimized counterexample, and the fixing commit in `docs/kiro/usage-log.md`.
4. IF any property fails at 10,000 cases, THEN THE Agent SHALL fix the cause and re-run the whole Property_Campaign before Phase 10 exits.
5. WHEN the Property_Campaign passes, THE Agent SHALL write `docs/evidence/P10-T01-security-suite.md` listing, for each property, the case count, result, duration, and Evidence record path, and for each catalog fixture, the expected and actual result.

### Requirement 3: Integration tests (P10-T02, Task 43)

**User Story:** As a user, I want the whole workflow tested end to end on several kinds of projects, including crashes, so that a restore works outside the one demo fixture.

#### Acceptance Criteria

1. WHEN the Integration_Suite runs, THE Integration_Suite SHALL run `dormbit-cli` scan, plan, capture, verify, restore, and brief on 6 Synthetic projects: the Dockline fixture (TypeScript), a Rust crate, a Python package, a Go module, a pnpm monorepo, and a folder without Git.
2. WHEN a restore in criterion 1 finishes, THE Integration_Suite SHALL check that every restored file is byte-identical to its source and that HEAD matches for the projects that have Git.
3. WHEN the CLI restore process is killed with SIGKILL after each journal state (Staged, Written, Verified, Committed), THE Integration_Suite SHALL run recovery and check that the destination is either absent or complete and verified, and that no DORMBIT staging folder remains.
4. WHEN capture and restore run on a Synthetic project whose files and folders are made read-only with `chmod a-w`, THE Integration_Suite SHALL check that every source file's hash, mode, and modification time is unchanged afterward.
5. THE Integration_Suite SHALL NOT install dependencies for, build, or run any Synthetic project.
6. WHEN the Integration_Suite passes, THE Agent SHALL write `docs/evidence/P10-T02-integration.md` with each suite's Evidence record path.

### Requirement 4: Native end-to-end tests (P1) (P10-T02, Task 43)

**User Story:** As a QA engineer, I want the native app driven automatically, so that native regressions are caught without a manual walkthrough.

#### Acceptance Criteria

1. WHERE Native_E2E ships, THE Native_E2E SHALL drive the native app through preserve, verify, and restore of the Dockline fixture using `@wdio/tauri-service` with the embedded driver plugin.
2. WHERE Native_E2E ships, THE desktop app SHALL compile the embedded driver plugin only with the `e2e-webdriver` Test feature.
3. IF Native_E2E is cut, THEN THE Final report and the README SHALL state that native E2E wasn't shipped and that the manual native QA checklist covers the native journey.

### Requirement 5: Supply chain and IPC checks (P10-T03, Task 44)

**User Story:** As a judge installing DORMBIT, I want dependency, secret, CSP, and IPC checks on the final code, so that I know what was checked before release.

#### Acceptance Criteria

1. WHEN `cargo deny check` runs on the final commit, it SHALL report zero errors.
2. WHEN `pnpm audit --audit-level high` runs on the final commit, it SHALL report zero high or critical advisories, and THE Agent SHALL list any moderate or low advisory with its decision in `docs/evidence/P10-T03-supply-chain.md`.
3. WHEN gitleaks scans the full Git history, it SHALL report zero findings.
4. THE CSP test SHALL check that the desktop `tauri.conf.json` CSP equals the foundation design string and that the built demo `index.html` CSP meta tag equals the judge-demo design string.
5. WHEN a Playwright test injects an inline `<script>` into the built demo, THE CSP test SHALL observe a `securitypolicyviolation` event and the script SHALL NOT run.
6. WHEN the IPC_Fuzz runs, it SHALL send random JSON arguments to every allowlisted command, 10,000 cases per command locally and 512 in CI, and check that each call returns a schema-valid result or a DormbitError with a stable code and never panics.
7. IF the release build of `dormbit-desktop` enables any Test feature or the `tauri` `devtools` feature, THEN the test-feature check SHALL fail and name the feature.
8. IF the release binary contains the marker string that every Test feature compiles in, THEN the test-feature check SHALL fail.

### Requirement 6: Security review (P10-T04, Task 45)

**User Story:** As a judge, I want an independent written review of the security claims, so that I can see what was checked and what wasn't.

#### Acceptance Criteria

1. THE Security_Review SHALL cover every threat row in §7 with its mitigation, the test or fixture that proves it, the Evidence record or CI run, and a result.
2. THE Security_Review SHALL include a dedicated section on the single `unsafe` `renameatx_np` call in `dormbit-fs`: its inputs, the invariants it relies on, its error handling, and its tests.
3. THE Security_Review SHALL list the §7 stated limits and the VER-2 integrity limit.
4. THE Security_Review SHALL list every open finding with a severity and a decision.
5. WHEN the `rust-security` agent finishes a draft, THE Reviewer SHALL review it together with the diff of the security modules and return `APPROVED` or `NEEDS_CHANGES`.
6. WHEN the Reviewer returns `NEEDS_CHANGES`, THE `rust-security` agent SHALL revise the code or the document and send it back, for at most 3 iterations in total.
7. IF the Reviewer hasn't returned `APPROVED` after 3 iterations, THEN THE Agent SHALL stop and show the User the open items, and Phase 10 SHALL NOT exit.
8. WHEN the Reviewer returns `APPROVED`, THE Agent SHALL record the verdict and the iteration count in `docs/status/STATUS.md` and `docs/kiro/usage-log.md`.
9. THE Agent SHALL mark Phase 10 done only when every Security_Suite, Integration_Suite, and Supply_Chain_Checks job is green on the final CI run of the phase and the Gate note links each result.

### Requirement 7: Release build (P12-T01, Task 49)

**User Story:** As a judge, I want a DMG built by a public workflow from a tagged commit, so that I can trust what I download.

#### Acceptance Criteria

1. WHEN a tag matching `v*.*.*` is pushed, THE Release_Workflow SHALL build the Release_Build on `macos-15` from that tag.
2. THE Release_Build SHALL use an app icon made from an original DORMBIT image, with the icon set generated by `tauri icon`.
3. WHEN `codesign -dv` runs on the built app, it SHALL report `Signature=adhoc`.
4. WHEN `codesign --verify --deep --strict` runs on the built app, it SHALL exit 0.
5. THE version SHALL be `1.0.0` in `apps/desktop/src-tauri/Cargo.toml`, `apps/desktop/src-tauri/tauri.conf.json`, and `apps/desktop/package.json`; IF the three differ, THEN THE Release_Workflow SHALL fail.
6. WHEN the build passes its checks, THE Release_Workflow SHALL create a draft GitHub release with the DMG and SHA256SUMS, and SHALL NOT publish it.
7. THE Release_Workflow SHALL run the test-feature check from Requirement 5 on the release binary.
8. THE Release_Workflow SHALL give `contents: write` only to the job that creates the draft release; every other job SHALL have `contents: read`.
9. WHEN `scripts/release/build-local.sh` runs on the User's Mac, it SHALL produce a DMG that passes criteria 3, 4, and 7.
10. IF the universal binary isn't shipped, THEN the DMG file name, the release notes, and the README SHALL state that the build is for Apple Silicon only.

### Requirement 8: Universal binary (P1) (P12-T01, Task 49)

**User Story:** As a judge on an Intel Mac, I want a binary that runs on my machine, so that I can test the native app too.

#### Acceptance Criteria

1. WHERE the universal binary ships, THE Release_Workflow SHALL build with `--target universal-apple-darwin`.
2. WHERE the universal binary ships, `lipo -archs` on the app's main executable SHALL print both `x86_64` and `arm64`.
3. WHERE the universal binary ships, THE release notes SHALL state how the Intel slice was tested, or that it wasn't tested on Intel hardware.

### Requirement 9: Clean-install run (P12-T02, Task 50)

**User Story:** As a judge, I want the install path tested exactly the way I'll experience it, so that the Gatekeeper steps in the README are known to work.

#### Acceptance Criteria

1. WHEN the Clean_Install_Run starts, the DMG SHALL be downloaded through a web browser from the draft release, and `xattr -p com.apple.quarantine` on the downloaded DMG SHALL print a value, which THE Agent records.
2. THE User SHALL copy the app to `/Applications` and move the folder `~/Library/Application Support/io.github.amirmlotfy.dormbit` aside by renaming it, or approve the Agent doing so; nothing SHALL be deleted.
3. WHEN the app is first opened, THE Agent SHALL record that Gatekeeper blocks it, and that the documented steps (System Settings → Privacy & Security → Open Anyway) open it.
4. WHEN the app is open, THE User and THE Agent SHALL run the full native workflow (Welcome, Privacy, folder pick, scan, Evidence View, Plan, approval, capture, verification, restore, result, brief export) on the extracted Dockline fixture and record pass or fail for each step in the Clean-install checklist.
5. IF any step fails, THEN THE Agent SHALL fix the cause, build a new DMG, and repeat the Clean_Install_Run from criterion 1.
6. THE README's Gatekeeper steps SHALL match the steps that worked in the Clean_Install_Run, word for word.
7. WHERE the User chooses to test on a separate macOS user account, THE User SHALL run it and THE Agent SHALL record the result as optional.

### Requirement 10: Production web deploy and daily smoke test (P12-T03, Task 51)

**User Story:** As a judge, I want the demo to stay up through judging, so that I can test it any day until Jan 29, 2027.

#### Acceptance Criteria

1. WHEN the Pages_Workflow is run with `workflow_dispatch` inputs `ref` and `channel=production`, THE Pages_Workflow SHALL build the demo from that ref and deploy it to `https://amirmlotfy.github.io/dormbit/`.
2. THE Production demo SHALL show the app version and short commit SHA, and SHALL NOT show the "Preview" label.
3. WHEN the first production deploy is live, THE Pages_Workflow SHALL deploy only through `workflow_dispatch`, not on push to `main`.
4. WHEN a production deploy finishes, THE Agent SHALL run the 12-step demo E2E on Chromium against the Production demo URL and record the result through the Evidence_Runner.
5. WHEN the Production demo loads in that test, every network request SHALL go to `https://amirmlotfy.github.io`, with no analytics or other third-party request.
6. THE Smoke_Workflow SHALL run once a day on a schedule and on `workflow_dispatch`.
7. WHILE the PT date is on or before 2027-01-29, THE Smoke_Workflow SHALL run the 12-step demo E2E on Chromium against the Production demo URL.
8. WHEN the PT date is after 2027-01-29, THE Smoke_Workflow SHALL exit 0 without testing and print that the judging period is over.
9. IF the smoke test fails and no open issue has the label `smoke-failure`, THEN THE Smoke_Workflow SHALL open an issue with that label and the run URL.
10. IF the smoke test fails and an open `smoke-failure` issue exists, THEN THE Smoke_Workflow SHALL add a comment with the run URL instead of opening another issue.
11. THE Smoke_Workflow SHALL have only `contents: read` and `issues: write` permissions.
12. IF the Production demo is down during judging, THEN THE Agent SHALL redeploy the `v1.0.0` tag through `workflow_dispatch`, with no new code, and tell the User.

### Requirement 11: Repository audit and fresh clone (P13-T01, Task 52)

**User Story:** As a judge, I want the public repo to contain everything the rules require and to build from a clean clone, so that I can verify the project myself.

#### Acceptance Criteria

1. WHEN the Repo_Audit runs, `git ls-files .kiro` SHALL list files and `git check-ignore -v .kiro .kiro/specs/foundation/tasks.md` SHALL print nothing.
2. WHEN the Repo_Audit runs, `gh repo view AmirmLotfy/dormbit --json licenseInfo,visibility` SHALL show MIT and PUBLIC.
3. THE README SHALL contain the tagline, the Production demo URL, the DMG download with checksum instructions and the Gatekeeper steps, build-from-source steps, architecture, security with a link to `SECURITY.md`, Kiro usage, the four BRF-2 outcomes, the limits (ad-hoc signing without notarization, no encryption, no OS sandbox, Apple Silicon only if Requirement 8 is cut), and the license.
4. THE repo SHALL contain `THIRD_PARTY_NOTICES.md` listing every Rust crate and npm package shipped in the Release_Build or the Production demo, with version and license.
5. THE repo SHALL include the OFL license file next to each bundled font.
6. THE repo SHALL contain `docs/submission/disclosures.md` stating that planning took place in Kiro Plan mode on Nov 3, 2026, listing every template or scaffold used, and stating that no code from before the Submission Period was used.
7. IF a tracked file contains the User's home folder path, a personal email address, or anything from `.scratch/`, THEN THE Repo_Audit SHALL fail and name the file.
8. WHEN the Fresh_Clone_Run runs, THE Agent SHALL clone the public repo into `.scratch/fresh-clone/` and run every README build-from-source step exactly as written through the Evidence_Runner, and every step SHALL exit 0.
9. IF a README step fails or a needed step is missing, THEN THE Agent SHALL fix the README and repeat the Fresh_Clone_Run from a new clone.

### Requirement 12: Release v1.0.0 (P13-T02, Task 53)

**User Story:** As a judge, I want a versioned release with checksums, so that I can download a known build and check it.

#### Acceptance Criteria

1. WHEN the User approves the release, THE Agent SHALL create the annotated tag `v1.0.0` on the audited commit and push it.
2. WHEN the Release_Workflow has created the draft, THE Agent SHALL show the User the release notes and asset list and SHALL publish the release only after the User approves.
3. THE release notes SHALL contain what's included, the Gatekeeper steps, the limits from Requirement 11.3, checksum instructions, and the Production demo URL.
4. WHEN the release is published, THE Agent SHALL download its assets with `gh release download v1.0.0` into `.scratch/release-check/`, and `shasum -a 256 -c SHA256SUMS` SHALL pass.
5. WHEN the User approves, THE Agent SHALL set the repo topics, the website to the Production demo URL, and the About description.
6. IF the Production demo was built from a commit other than `v1.0.0`, THEN THE Agent SHALL redeploy it from `v1.0.0` and re-run the production smoke test.

### Requirement 13: Screenshots (P14-T01, Task 54)

**User Story:** As a judge browsing the Devpost gallery, I want screenshots that show the real app, so that what I see is what I'll get.

#### Acceptance Criteria

1. THE Agent SHALL capture native screenshots from the newest release build (a `v1.0.0-rc.N` or `v1.0.0` DMG) built from the same commit as the Production demo, and web screenshots from the Production demo, in real app states, with no edited UI.
2. THE screenshots SHALL show paths with the home folder as `~`, and SHALL NOT show the User's name, real project names other than the Dockline fixture, notifications, other apps, the Dock, or the menu bar.
3. THE Agent SHALL capture native screenshots as window-only captures, after the User grants Screen Recording permission.
4. WHEN a screenshot is selected, THE Agent SHALL strip its metadata with `exiftool -all=` and confirm with `exiftool` that no GPS, author, or host field remains.
5. THE Screenshot_Set SHALL contain 6 to 8 images in `docs/media/`, including a README hero image and a thumbnail base, with `docs/media/README.md` naming the screen and state of each.
6. WHEN the Screenshot_Set is final, THE Agent SHALL check each image against the live app and confirm the screen still looks the same.

### Requirement 14: Script and shot list (P15-T01, Task 55)

**User Story:** As the User, I want to approve a script before anything is recorded, so that the video says only what is true and fits the time limit.

#### Acceptance Criteria

1. THE Shot list SHALL contain, in order: 0:00-0:15 the problem; native discovery and Evidence View; Plan; Capture and Verify; Restore; corruption and rollback; the Brief and the four outcomes; 2:20-2:40 architecture and how Kiro was used; and the end card.
2. THE Shot list SHALL plan a total duration between 2:35 and 2:45.
3. THE Video_Script SHALL make only claims that the shipped app shows or that an Evidence record supports, and SHALL use the four BRF-2 outcome labels exactly.
4. THE Video_Script SHALL NOT say or imply that DORMBIT resumes AI sessions, runs or builds projects, encrypts data, or is notarized.
5. THE Agent SHALL ask the User to choose the voiceover: the User's own voice, Kokoro-82M (Apache-2.0), or captions only.
6. THE Media_Pipeline SHALL NOT use macOS `say` voices.
7. THE Agent SHALL start recording only after the User approves the Video_Script and Shot list.

### Requirement 15: Recording and editing (P15-T02, Task 56)

**User Story:** As a judge, I want a clear, legally clean video, so that it meets the rules and shows the product on its target device.

#### Acceptance Criteria

1. WHEN native takes are recorded, THE User SHALL drive the app while THE Agent runs a window-only recorder and resets the Dockline fixture between takes.
2. WHEN web takes are recorded, THE Agent SHALL record them from a Playwright script at a 1920×1080 viewport against the Production demo.
3. THE Agent SHALL render every Title card from HTML in `media/cards/` to PNG with Playwright, using only bundled fonts and no third-party logo.
4. THE Media_Pipeline SHALL assemble the Final cut with ffmpeg from `media/edit/timeline.json` through `scripts/media/build-video.sh`, with no music track.
5. THE Media_Pipeline SHALL normalize the voice track to -14 LUFS integrated loudness.
6. THE Agent SHALL transcribe the Final cut with whisper-cli and correct the result against the Video_Script into the Caption file.
7. THE Media_Pipeline SHALL produce the Final cut as H.264 video at 1920×1080 with AAC audio, the Caption file, and a 1280×720 Thumbnail under 2 MB.
8. THE Agent SHALL keep raw recordings in `media/raw/`, which is gitignored.
9. THE Final cut SHALL show third-party product names only as plain text, with no third-party logo, and SHALL contain no copyrighted music.

### Requirement 16: Video verification (P15-T03, Task 57)

**User Story:** As the User, I want automated checks before I watch the final cut, so that my review is about quality, not about catching rule violations.

#### Acceptance Criteria

1. WHEN the Video_Verifier runs, `ffprobe` SHALL report a Final cut duration below 180.0 seconds, and the Video report SHALL state whether it is within the 155-165 second target.
2. WHEN the Video_Verifier runs, `ffprobe` SHALL report 1920×1080 H.264 video, and AAC audio unless the User chose captions only.
3. WHEN the Video_Verifier runs, it SHALL extract one frame every 5 seconds to `.scratch/video-frames/`, and THE Agent SHALL inspect each frame for third-party logos, paths not shown as `~`, personal information, and notifications, and list every finding.
4. WHEN the Video_Verifier measures loudness with ffmpeg's `ebur128` filter, the integrated loudness SHALL be between -15 and -13 LUFS.
5. WHERE the User chose captions only, THE Video report SHALL mark the loudness check as not applicable.
6. WHEN the Video_Verifier transcribes the Final cut with whisper-cli, it SHALL compare the transcript with the Video_Script and list every line that differs.
7. THE Caption file SHALL match the Video_Script wording, and its last cue SHALL end before the Final cut ends.
8. IF any automated check fails, THEN THE Agent SHALL fix the cause and re-run the Video_Verifier before asking the User to watch.
9. WHEN the automated checks pass, THE Agent SHALL write the Video report with each measured value, the Final cut's SHA-256, and the Evidence record paths.
10. THE Agent SHALL upload nothing until the User has watched the Final cut and approved it.

### Requirement 17: YouTube upload (P16-T01, Task 58)

**User Story:** As a judge, I want a public YouTube video that plays without signing in, so that it meets the submission rules.

#### Acceptance Criteria

1. THE Agent SHALL upload the Final cut through Browser_Automation after the User approves it, or THE User SHALL upload it by following the Manual steps.
2. THE YouTube_Video SHALL first be saved as Unlisted, with the title, the description (Production demo URL, repo URL, chapters starting at 0:00), the Caption file, the "not made for kids" setting, and the Thumbnail if the account can use custom thumbnails.
3. WHEN the Agent fills in the upload form, THE Agent SHALL show the User every field value before saving.
4. WHEN the YouTube_Video is Unlisted, THE Agent SHALL check that it plays, that its duration matches the Video report, and that captions are available.
5. THE Agent SHALL switch the YouTube_Video to Public only after the User approves.
6. WHEN the YouTube_Video is Public, a Logged-out check SHALL open its URL, see playback time advance, and find no private or sign-in notice.
7. IF the account can't use custom thumbnails, THEN THE Agent SHALL tell the User that the account needs phone verification and leave the default thumbnail until the User acts.
8. THE Agent SHALL record the public URL in `docs/status/STATUS.md` and `docs/submission/devpost.md`.

### Requirement 18: Devpost content (P17-T01, Task 59)

**User Story:** As a judge, I want a submission that claims only what the repo proves, so that I can check every statement.

#### Acceptance Criteria

1. THE Devpost_Content SHALL contain the story sections, a Kiro write-up, judge testing instructions (the web demo first, then the DMG with checksum and Gatekeeper steps), the Frankenstein rationale from §2, built-with tags, the gallery from the Screenshot_Set, and the demo, repo, release, and video URLs.
2. THE Kiro write-up SHALL cite only repo paths that exist at `v1.0.0` and only numbers taken from Evidence records or CI runs, each with a link to its source.
3. IF a cited path doesn't exist at `v1.0.0`, a number in the Kiro write-up has no evidence link, or a banned overclaim phrase from the design appears, THEN THE Claims_Check SHALL exit 1 and name each problem.
4. THE Devpost_Content SHALL name exactly one category: Frankenstein.
5. THE Devpost_Content SHALL include the disclosures from `docs/submission/disclosures.md`.
6. THE Devpost_Content SHALL state that accuracy and the workflow were validated, and SHALL NOT claim that adoption was validated (§1).
7. THE Devpost_Content SHALL use the four BRF-2 outcomes exactly and SHALL pass the copy audit against them.

### Requirement 19: Devpost submission (P17-T02, Task 60)

**User Story:** As the User, I want to see and approve every field before the entry is submitted, so that nothing goes out that I haven't checked.

#### Acceptance Criteria

1. THE Agent SHALL fill in the Devpost form through Browser_Automation after the User approves it, or THE Agent SHALL give the User the Manual steps with every field value.
2. WHEN the form is filled in, THE Agent SHALL show the User every field value and the selected category before submitting.
3. THE Agent SHALL click Submit only after the User approves, targeting Wed Dec 2, 2026, 12:00 PM PT.
4. WHEN the entry is submitted, a Logged-out check SHALL open the public submission page and confirm the title, video, demo link, repo link, and the Frankenstein category.
5. THE Agent SHALL NOT report the Devpost_Submission as submitted until it has seen Devpost's confirmation or the public page.
6. WHEN the public page is confirmed, THE Agent SHALL record the submission URL in `docs/status/STATUS.md`.

### Requirement 20: Browser automation and manual fallback (§18-19)

**User Story:** As the User, I want automation to act only in my own browser, with my approval, and never handle my credentials, so that my accounts stay under my control.

#### Acceptance Criteria

1. THE Agent SHALL use only Browser_Automation for YouTube and Devpost, and SHALL NOT use the YouTube API or any Devpost API.
2. THE Agent SHALL enable the `playwright-chrome` server only after the User approves the Chrome extension.
3. THE User SHALL sign in to YouTube and Devpost themselves; THE Agent SHALL NOT type, read, or store passwords, one-time codes, or cookies.
4. WHEN Browser_Automation is about to save, publish, or submit, THE Agent SHALL stop and wait for the User's approval of that specific action.
5. THE Agent SHALL write the Manual steps for YouTube and Devpost before Browser_Automation is first used.
6. IF Browser_Automation is blocked, fails, or shows an unexpected page, THEN THE Agent SHALL stop automating and give the User the Manual steps from the current point.
7. WHEN Task 60 is done, THE Agent SHALL set the `playwright-chrome` server back to `disabled: true`.
8. THE Agent SHALL run every Logged-out check as a Playwright test in a fresh browser context, never in the User's Chrome profile.

### Requirement 21: Approval gates (§22)

**User Story:** As the User, I want the Agent to stop at every action that is public, hard to reverse, or outside the repo, so that nothing of that kind happens without my yes.

#### Acceptance Criteria

1. WHEN an install outside the repo is needed (for example the whisper-cli model or Kokoro-82M), THE Agent SHALL ask the User and SHALL NOT install until the User approves.
2. WHEN a write outside the workspace is needed (for example copying the app to `/Applications` or renaming the app data folder), THE Agent SHALL ask the User and SHALL NOT write until the User approves.
3. WHEN a release tag (`v1.0.0-rc.N` or `v1.0.0`) is to be pushed or a release published, THE Agent SHALL ask the User and SHALL NOT act until the User approves each one.
4. WHEN the repo topics, website, or About description are to be changed, THE Agent SHALL ask the User and SHALL NOT change them until the User approves.
5. WHEN a screenshot or recording of the screen is needed, THE Agent SHALL ask the User to grant Screen Recording permission and SHALL NOT capture until it is granted.
6. WHEN Browser_Automation is needed, THE Agent SHALL ask the User to install and approve the Chrome extension and SHALL NOT enable `playwright-chrome` until the User approves.
7. WHEN the YouTube_Video is to be made Public, THE Agent SHALL ask the User and SHALL NOT change its visibility until the User approves.
8. WHEN the Devpost_Submission is to be submitted, THE Agent SHALL ask the User and SHALL NOT click Submit until the User approves.
9. THE Agent SHALL also ask the User for the voiceover choice, approval of the Video_Script, and approval of the Final cut before moving past each.
10. WHEN the User approves or declines, THE Agent SHALL write an Approval record.
11. IF the User declines, THEN THE Agent SHALL use the Manual steps or stop, and SHALL NOT retry the same action without a new approval.
12. THE Agent SHALL treat each approval as covering only the action it names.

### Requirement 22: Release gating, evidence, and status

**User Story:** As the User, I want release work blocked unless the basics are green, and every phase closed with a gate note, so that the status ledger is always true.

#### Acceptance Criteria

1. IF a task in this spec for Phase 12 or later (tasks 5 to 17) is about to start and the latest Evidence record isn't green, gitleaks finds anything, `LICENSE` is missing, or `.kiro` is ignored, THEN THE Release_Gate SHALL block it and name each failed condition.
2. WHEN a task in this spec is done, THE Agent SHALL write its Task evidence note linking every Evidence record and CI run it relies on.
3. WHEN each of Phases 10 and 12 to 18 ends, THE Agent SHALL write its Gate note and update `docs/status/STATUS.md` and `docs/status/HANDOFF.md`.
4. THE Agent SHALL mark an item Released only when it is in the `v1.0.0` tag or the Production demo, and Submitted only when it is part of the confirmed Devpost_Submission.
5. THE Agent SHALL name the task ID in every commit message for this spec.
6. THE Agent SHALL record notable Kiro usage for this spec (review verdicts, property counterexamples, hook catches, automation runs) in `docs/kiro/usage-log.md`.

### Requirement 23: Final audit (P18-T01, Task 61)

**User Story:** As the User, I want a last check of every deliverable with a written report, so that I know the submission is complete and honest.

#### Acceptance Criteria

1. WHEN the Final_Audit runs, a Logged-out check SHALL get HTTP 200 and the expected page content from the Production demo, the repo, the GitHub_Release, the YouTube_Video, and the Devpost_Submission.
2. WHEN the Final_Audit runs, it SHALL confirm MIT in the repo's About section and `.kiro` at the repo root.
3. WHEN the Final_Audit runs, it SHALL list every Smoke_Workflow run since Task 51 with its result.
4. WHEN the Final_Audit runs, it SHALL download the DMG from the GitHub_Release and pass `shasum -a 256 -c SHA256SUMS`.
5. WHEN the Final_Audit runs, it SHALL confirm the YouTube_Video is Public and under 3:00, and that the Devpost_Submission shows the Frankenstein category and correct links.
6. WHEN the Final_Audit runs, it SHALL confirm that the CI_Workflow run for the final commit on `main` succeeded.
7. WHEN the Final_Audit runs, it SHALL check each behavior claim in the README against the shipped app and list any mismatch.
8. THE Final report SHALL state what works, its limits, what's next, which P1 items shipped or were cut, and that adoption wasn't validated.
9. THE Agent SHALL run the Final_Audit first on Wed Dec 2, 2026 after submission and again on Thu Dec 3, 2026 before the 6:00 PM PT freeze.
10. IF a check fails before the freeze, THEN THE Agent SHALL fix it and re-run the Final_Audit; IF a check fails after the freeze, THEN THE Agent SHALL fix only critical problems before Fri Dec 4, 2026, 2:00 PM PT and record the rest in the Final report.

### Requirement 24: Timeline (§21)

**User Story:** As the User, I want hard dates for each step, so that the submission lands before the deadline with time to spare.

#### Acceptance Criteria

1. THE Agent SHALL plan Phases 12 to 14 for Fri Nov 27 to Sat Nov 28, 2026, Phase 15 for Sun Nov 29 to Mon Nov 30, and Phase 16 for Tue Dec 1.
2. THE Agent SHALL target submission for Wed Dec 2, 2026, 12:00 PM PT.
3. WHILE it is after Thu Dec 3, 2026, 6:00 PM PT, THE Agent SHALL make only critical fixes.
4. IF the Devpost_Submission isn't confirmed by Thu Dec 3, 2026, 6:00 PM PT, THEN THE Agent SHALL tell the User what is blocking it and the time left before Fri Dec 4, 2026, 2:00 PM PT.
5. THE Agent SHALL NOT plan any Devpost edit after Fri Dec 4, 2026, 2:00 PM PT, because the rules allow none.
6. IF a phase in this spec runs more than one day late, THEN THE Agent SHALL tell the User and propose P1 cuts in the §3 order.

## Open questions
1. **Pushes after the deadline.** The plan says the demo must stay up through Jan 29, 2027, and only work done during the Submission Period counts. This spec allows only a redeploy of the `v1.0.0` tag during judging, with no new code (Requirement 10.12). Whether any commit may be pushed after Dec 4 is left to the User.
2. **Where screenshots live.** Task 54 puts the Screenshot_Set in `docs/media/`, but the `release` agent can write only `media/**` and `docs/release/**`. The `release` agent captures into `media/raw/`; the main Agent copies the selected images into `docs/media/`.
3. **Committing the Final cut.** The plan doesn't say whether the MP4 is committed. A ~100 MB file is close to GitHub's per-file limit, so this spec keeps `media/final/dormbit-demo.mp4` out of Git, records its SHA-256 in the Video report, and commits the Caption file, Thumbnail, script, and Title card sources.
4. **Loudness tolerance.** The plan says -14 LUFS without a tolerance. This spec accepts -15 to -13 LUFS integrated so the check is testable.
5. **Pages preview vs production.** One repo has one Pages site. The `judge-demo` spec deploys a labeled preview on push; this spec switches the label to production and removes the push trigger so later pushes can't replace the production build (Requirement 10.3).
6. **Scheduled workflow lifetime.** GitHub disables scheduled workflows in public repos after 60 days without repo activity. If the last commit is on or after Dec 1, 2026, the 60 days end after Jan 29, 2027. If a run is skipped for that reason, the Final report says so.
7. **Expected codes for some fixtures.** A few attack classes (for example archive bombs and limit violations) may be reported as `Unsupported` by the verifier rather than as an error. The Fixture_Catalog uses the code that the owning spec defines (for example `CAPSULE_LIMIT_EXCEEDED` if it is added to `DormbitError`); until those codes exist, such fixtures count only once their code is in the shared list.
8. **Tag order.** Task 49 builds through a tag-triggered workflow and Task 50 downloads a DMG through a browser, but the `v1.0.0` tag is created in Task 53. This spec uses release-candidate tags `v1.0.0-rc.N` (draft prereleases) for Tasks 49-52 and the final `v1.0.0` tag in Task 53. Each RC tag push needs the same approval as a release (Requirement 21.3). The `v*.*.*` pattern in Requirement 7.1 matches both.
9. **Third-party notices tool.** No license-collection tool is installed. This spec generates `THIRD_PARTY_NOTICES.md` from `cargo deny list` and `pnpm licenses list --prod`, which are already available, rather than adding a tool.
