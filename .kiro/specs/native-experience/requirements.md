# Requirements Document

## Introduction
This spec covers the complete DORMBIT user interface and its refinement: master plan Phase 08 (P08, Tasks 34-37) and Phase 11 (P11, Tasks 46-48). It formalizes NAT-1, NAT-2, and UX-1, and the UX/UI architecture in master plan §9: layout, the 13 screens, color and text tokens, type, spacing, motion, status markers, the capsule seal, per-screen states, and the dev-only `/gallery` route. Phase 08 runs in parallel from the foundation spec against `MockClient`; the owning specs (scope-and-discovery, capsule-and-verification, transactional-restore, intelligence-and-brief) connect screens to real commands. Phase 11 audits visuals, accessibility (WCAG AA), and copy, and runs the P1 accuracy validation on 5 public repos. Screen 13 (Judge Demo) is built in the judge-demo spec from the same `packages/ui` components; this spec's accessibility and copy audits cover it.

## Glossary
- **DORMBIT_App**: the macOS desktop app in `apps/desktop` (React UI plus `src-tauri`).
- **WebView**: the Tauri 2 WebView that renders the React UI. It renders untrusted project text.
- **Native_Shell**: the Tauri configuration and Rust setup code in `apps/desktop/src-tauri` (window, CSP, capabilities, command allowlist, menus).
- **Design_System**: the tokens, fonts, and components in `packages/ui`.
- **App_Shell**: the window layout: sidebar, title-bar overlay, router, and onboarding flow.
- **Screen**: one of the 13 screens in master plan §9: Welcome, Privacy & Permissions, Folder Selection, Shelf, Evidence View, Preservation Plan, Capsule Creation, Integrity Verification, Restore Wizard, Restoration Result, Resurrection Brief, Settings, Judge Demo. Screens 1-12 are native; screen 13 runs in `apps/demo`.
- **Screen state**: one of loading, empty, error, partial, cancelled, or ready.
- **State_Matrix**: the table, per Screen, of each Screen state and how to reach it; shown on the Gallery.
- **Gallery**: the dev-only `/gallery` route that documents every component and Screen state and backs screenshot tests.
- **DormbitClient**: the TypeScript interface in `packages/client` that every Screen uses to call the core. Defined by the foundation spec.
- **MockClient**: the in-memory `DormbitClient` in `packages/client` used for UI development and tests.
- **TauriClient**: the `DormbitClient` that calls allowlisted Tauri commands and validates every response with Zod.
- **Command**: one of the allowlisted Tauri commands in master plan §6 (for example `scope_pick`, `capture_start`, `restore_recover`).
- **Job**: a long-running command (discovery, capture, verify, restore) that streams progress events over a Tauri Channel.
- **DormbitError**: the error value `{code, message, retryable}` with the stable codes from master plan §8.
- **Error_Presenter**: the `packages/ui` module that maps a DormbitError code to user-facing copy and actions.
- **Safe_Markdown**: the `packages/ui` Markdown renderer built on react-markdown with raw HTML disabled.
- **Untrusted text**: any text that comes from project content: READMEs, commit messages, file names, branch names, manifest values, TODO comments, agent files.
- **Status marker**: a glyph paired with a text label that shows a status. The five plan markers are Verified ✓, Needs decision ◆, Failed ✕, Unknown ?, Unsupported ⊘.
- **Finding status**: Confirmed, Inferred, Unknown, or Unsupported (INT-1).
- **Capsule**: a `.dormbit` file produced by capture (capsule format v1, master plan §8).
- **Capsule_Seal**: the component that draws a fingerprint pattern from a capsule ID. "Pressed" is its verified appearance.
- **VerificationReport**: the verifier's result `{capsule_id, file_sha256, manifest_sha256, checks[], failed_files[], registry_match, status}`.
- **Four outcomes**: the separate results from BRF-2: "Files restored and verified", "Context reconstructed (inferred from evidence)", "AI session resumed: not performed", "Builds/runs: unknown (never executed)".
- **Copy_Audit**: the review of all UI and demo wording against the four outcomes and the truthfulness rules.
- **Accuracy_Validation**: the P1 scoring of DORMBIT's findings on 5 public open-source repos.
- **Display path**: a path shown to the user with the home folder written as `~`.
- **Large text**: WCAG large text: at least 24 px regular or 18.66 px bold.

## Requirements

### Requirement 1: Hardened WebView (NAT-1)

**User Story:** As a developer trusting DORMBIT with my projects, I want the UI layer to have no direct filesystem or shell access, so that a bug or injected text in the WebView cannot touch my files.

#### Acceptance Criteria

1. THE Native_Shell SHALL register no Tauri `fs` plugin and no `shell` plugin.
2. THE Native_Shell SHALL expose to the WebView only the Commands listed in master plan §6, through `AppManifest::commands` and a capability file.
3. IF a build adds a plugin permission or a Command not on the allowlist, THEN the NAT-1 configuration test SHALL fail.
4. THE Native_Shell SHALL set a CSP whose `script-src` contains no `'unsafe-inline'` and no `'unsafe-eval'`.
5. WHERE the build is a release build, THE Native_Shell SHALL have devtools disabled.
6. WHEN the App_Shell adds native menus or window settings, THE Native_Shell SHALL implement them in Rust without adding a plugin or Command beyond the allowlist.

### Requirement 2: Untrusted text without raw HTML (NAT-2)

**User Story:** As a user scanning projects I didn't write, I want project text shown as text, so that a hostile README or commit message can't run code or load content.

#### Acceptance Criteria

1. THE Safe_Markdown SHALL render Markdown with raw HTML skipped, so that no HTML element in the source appears in the DOM.
2. THE Safe_Markdown SHALL render links as non-navigating text that shows the link text and its URL.
3. THE Safe_Markdown SHALL render images as a text placeholder containing the alt text, and SHALL NOT issue a request for the image URL.
4. THE Design_System SHALL render Untrusted text outside Safe_Markdown only through React text nodes.
5. IF source code in `apps/` or `packages/` uses `dangerouslySetInnerHTML`, THEN the lint step SHALL fail.

### Requirement 3: Redacted logs (NAT-2)

**User Story:** As a user, I want DORMBIT's logs to leave out sensitive details, so that sharing a log doesn't leak my data.

#### Acceptance Criteria

1. THE DORMBIT_App SHALL write paths to logs only as Display paths.
2. THE DORMBIT_App SHALL NOT write file contents, `.env` values, or remote URL credentials to logs.
3. WHERE the build is a release build, THE TauriClient SHALL NOT log Command arguments or response payloads to the WebView console.
4. IF a Zod validation of a Command response fails, THEN THE TauriClient SHALL log the Command name and the failing schema path only, not the payload.

### Requirement 4: No telemetry and no outside requests (NAT-2, §9 Type)

**User Story:** As a privacy-minded developer, I want DORMBIT to make no network requests, so that nothing about my projects leaves my Mac.

#### Acceptance Criteria

1. THE DORMBIT_App SHALL contain no telemetry, analytics, or crash-reporting code.
2. THE WebView CSP SHALL limit `connect-src`, `font-src`, and `img-src` to the app bundle, Tauri IPC, and `data:` or `blob:` URLs the app creates.
3. THE Design_System SHALL load every font from files bundled in the app, with no web-font request.
4. WHEN any Screen is exercised in the Playwright UI suite, THE test SHALL observe zero requests to origins other than the local test server.

### Requirement 5: Window layout and navigation (§9 Layout)

**User Story:** As a Mac user, I want a single window with a familiar sidebar, so that I can move between my projects, capsules, restores, and settings.

#### Acceptance Criteria

1. THE App_Shell SHALL show one window with a sidebar containing Shelf, Capsules, Restores, and Settings.
2. THE Native_Shell SHALL set a minimum window size of 1100×720.
3. THE Native_Shell SHALL overlay the title bar on the content with inset traffic lights, and THE App_Shell SHALL provide a drag region in the title-bar area.
4. WHILE onboarding is not complete, THE App_Shell SHALL show the onboarding Screens as a full-window flow without the sidebar.
5. THE App_Shell SHALL route each of the 12 native Screens to its own route, and the Gallery to `/gallery` in development builds only.

### Requirement 6: Onboarding screens (screens 1-3, journey 1, SCP-1)

**User Story:** As a first-time user, I want a short onboarding that tells me what DORMBIT does and doesn't do before I grant a folder, so that I can trust it with my projects.

#### Acceptance Criteria

1. WHEN the DORMBIT_App starts with no Read scope grant, THE App_Shell SHALL open the Welcome Screen.
2. THE Privacy & Permissions Screen SHALL state that DORMBIT works locally, never deletes project files, never runs project code, and reads agent histories only with consent.
3. WHEN the user chooses a folder on the Folder Selection Screen, THE Screen SHALL call `scope_pick` so that Rust opens the native folder picker.
4. THE Folder Selection Screen SHALL refer to granted folders by grant ID and show only the Display path returned by Rust.
5. WHEN `scope_pick` returns a grant, THE Folder Selection Screen SHALL start discovery and show live progress with real file counts until the Shelf opens.
6. IF the user cancels the native picker, THEN THE Folder Selection Screen SHALL stay open with no grant created and no error shown.

### Requirement 7: Shelf (screen 4, DSC-3, DSC-4, DSC-5)

**User Story:** As a developer with many projects, I want a dense, searchable list of them, so that I can find the one I want to shelve or restore.

#### Acceptance Criteria

1. THE Shelf Screen SHALL show one row per project with name, middle-truncated Display path, stack chips, branch and dirty marks, latest activity, size, and Capsule_Seal.
2. THE Shelf Screen SHALL support search, sort, filter, and multi-select.
3. THE Shelf Screen SHALL virtualize rows so that only visible rows are in the DOM.
4. THE Shelf Screen SHALL show dormancy only as the manual mark set through `project_set_dormant`, and SHALL NOT label a project inactive or abandoned from timestamps.
5. THE Shelf Screen SHALL label upstream ahead and behind counts "as of last fetch".
6. WHEN a project's size or activity is partial, THE Shelf Screen SHALL mark that value as partial.

### Requirement 8: Evidence View (screen 5, journey 2, INT-1)

**User Story:** As a developer returning to a project, I want to see what DORMBIT found and where it found it, so that I can trust each statement.

#### Acceptance Criteria

1. THE Evidence View Screen SHALL provide tabs for Overview, Evidence, Files, Git, Agent context, and History.
2. THE Evidence View Screen SHALL show each finding with its Finding status as a Status marker, its rule ID, and its evidence references.
3. WHEN the user filters by Finding status, THE Evidence View Screen SHALL show only findings with the chosen statuses and the count of hidden findings.
4. WHEN the user opens an evidence reference, THE Evidence View Screen SHALL show the cited file and line range, Git object, manifest key, or file stat.
5. THE Evidence View Screen SHALL NOT display the contents of files in the Excluded: sensitive group, and SHALL show only key names from `.env.example`.
6. WHILE no agent adapter has consent, THE Agent context tab SHALL show in-repo agent files only and state that external session histories are not read.

### Requirement 9: Preservation Plan (screen 6, CAP-1 to CAP-7)

**User Story:** As a developer shelving a project, I want to review and decide what goes into the capsule, so that nothing is captured without my approval.

#### Acceptance Criteria

1. THE Preservation Plan Screen SHALL show the plan items in the groups Included, Excluded: rebuildable, Excluded: sensitive, Needs decision, Unreadable, and Unsupported, each item with its reason code.
2. THE Preservation Plan Screen SHALL show a summary rail with group totals, sizes, the plan digest, and outstanding decisions and acknowledgments.
3. THE Preservation Plan Screen SHALL offer no control that includes an item in Excluded: sensitive.
4. WHILE any item is in Needs decision or any warning is unacknowledged, THE Preservation Plan Screen SHALL keep Approve disabled and state the reason next to it.
5. WHEN the user approves, THE Preservation Plan Screen SHALL call `plan_approve` with the displayed digest and the acknowledgments, after the user has chosen a capsule library folder.
6. IF `plan_approve` returns `PLAN_DIGEST_MISMATCH`, THEN THE Preservation Plan Screen SHALL reload the plan and ask the user to review it again.

### Requirement 10: Capsule Creation (screen 7, CPS-1, CAP-7)

**User Story:** As a developer, I want to watch the capture happen step by step, so that I know what was written and whether it finished.

#### Acceptance Criteria

1. THE Capsule Creation Screen SHALL show the capture steps as a step state machine with the current step marked.
2. WHILE a capture Job runs, THE Capsule Creation Screen SHALL show progress from the Job's real file and byte counts.
3. WHEN the user cancels a capture, THE Capsule Creation Screen SHALL call `job_cancel` and, on `CANCELLED`, state that no capsule was saved.
4. IF the capture fails with `FILE_CHANGED_DURING_CAPTURE`, THEN THE Capsule Creation Screen SHALL name the changed file by Display path and state that no capsule was saved.
5. WHEN capture completes, THE Capsule Creation Screen SHALL list files that appeared after approval as not captured.
6. WHEN capture completes, THE Capsule Creation Screen SHALL start the separate verification pass and hand off to the Integrity Verification Screen.

### Requirement 11: Integrity Verification and capsule seal (screen 8, VER-1, VER-2, journey 4)

**User Story:** As a developer, I want a clear verification result and a seal that only appears pressed when it's earned, so that I never mistake an unverified capsule for a good one.

#### Acceptance Criteria

1. THE Integrity Verification Screen SHALL list every verifier check with a Status marker and detail text.
2. WHEN the VerificationReport status is Failed, THE Integrity Verification Screen SHALL name each damaged file by its manifest path.
3. THE Integrity Verification Screen SHALL show the registry result, and SHALL show a capsule not in the registry as Unknown, not Failed.
4. THE Integrity Verification Screen SHALL state that SHA-256 shows integrity, not authorship, and that the OS cache may serve the bytes read.
5. THE Capsule_Seal SHALL draw the same pattern for the same capsule ID and SHALL derive the pattern only from the capsule ID.
6. WHILE the latest VerificationReport for a capsule ID has status Verified, THE Capsule_Seal SHALL appear pressed.
7. IF a capsule has no VerificationReport, or its latest VerificationReport has any status other than Verified, THEN THE Capsule_Seal SHALL appear unpressed.
8. THE Capsule_Seal SHALL have a text label stating the verification state, so that the state does not depend on its appearance.
9. WHEN the user opens a capsule with `capsule_pick`, THE Integrity Verification Screen SHALL offer Verify and show the result as for any other capsule.

### Requirement 12: Restore Wizard (screen 9, RST-1, RST-2)

**User Story:** As a developer, I want a guided restore that checks everything before writing, so that a restore can never overwrite my work.

#### Acceptance Criteria

1. THE Restore Wizard Screen SHALL ask for a capsule, a destination parent folder through `restore_target_pick`, and a new folder name.
2. THE Restore Wizard Screen SHALL show each pre-check from `restore_precheck` with a Status marker before Start is enabled.
3. IF any pre-check fails, THEN THE Restore Wizard Screen SHALL keep Start disabled and show the DormbitError code and message for that check.
4. IF `restore_precheck` returns `RESTORE_DEST_EXISTS`, THEN THE Restore Wizard Screen SHALL state that DORMBIT never overwrites and ask for a different name.
5. WHILE a restore Job runs, THE Restore Wizard Screen SHALL show the staging, verifying, and commit steps with real file and byte counts.

### Requirement 13: Restoration Result and crash recovery (screen 10, RST-5, RST-7, journey 10)

**User Story:** As a developer, I want a complete report of what was restored, and a recovery offer after a crash, so that I always know the state of my disk.

#### Acceptance Criteria

1. THE Restoration Result Screen SHALL list files written and verified, mismatches, items not restored with reasons, risky Git config keys, executable files, the source-unchanged check, and the count of writes outside the destination.
2. THE Restoration Result Screen SHALL show the Git HEAD and refs check result, and SHALL show the working-tree status check result.
3. IF a restore failed and rolled back, THEN THE Restoration Result Screen SHALL state that the destination folder was not created.
4. WHEN the DORMBIT_App starts and `restore_recover` reports an unfinished restore, THE App_Shell SHALL show a dialog that offers to discard that restore's staging folder.
5. WHEN the user confirms the discard, THE App_Shell SHALL call `restore_recover` and show the result, and SHALL NOT delete anything without that confirmation.

### Requirement 14: Resurrection Brief (screen 11, BRF-1, BRF-2)

**User Story:** As a developer resuming a project, I want a readable brief that cites its sources and is honest about what it can't know, so that I can start work quickly without false confidence.

#### Acceptance Criteria

1. THE Resurrection Brief Screen SHALL render the brief sections through Safe_Markdown in the Atkinson Hyperlegible Next typeface.
2. THE Resurrection Brief Screen SHALL show the Four outcomes as four separate items, each with a Status marker and its exact BRF-2 wording.
3. THE Resurrection Brief Screen SHALL show the finding ID cited by each line, and WHEN the user activates a citation, THE Screen SHALL open that finding in the Evidence View.
4. WHEN the user exports the brief, THE Resurrection Brief Screen SHALL call `brief_export` so that Rust opens the save dialog.

### Requirement 15: Settings (screen 12, journey 9)

**User Story:** As a user, I want to see and revoke what DORMBIT can access, so that I stay in control of my folders and data.

#### Acceptance Criteria

1. THE Settings Screen SHALL provide sections for scopes, consent, defaults, data and privacy, and about and licenses.
2. WHEN the user revokes a folder grant, THE Settings Screen SHALL call `scope_revoke` and THE App_Shell SHALL remove that grant's projects from every cached query.
3. WHEN the user withdraws an adapter consent, THE Settings Screen SHALL call `consent_set` and THE App_Shell SHALL remove that adapter's cached session data.
4. THE data and privacy section SHALL state what DORMBIT stores locally and that it sends no telemetry.
5. THE about and licenses section SHALL show the app version from `app_info` and the license text of each bundled font.

### Requirement 16: Color tokens and contrast (§9 Color tokens)

**User Story:** As a user with low vision, I want text that meets contrast standards, so that I can read every screen.

#### Acceptance Criteria

1. THE Design_System SHALL define these color tokens with these values (ADR-011): Plate `#F6F7F5`, Panel `#ECEEEA`, Rule `#D3D7D3`, Edge `#737B76`, Ink `#1A1E1C`, Steel `#59625D`, Graphite `#0F1211`, Graphite-2 `#181C1A`, On-dark `#E8EBE7`, On-dark-muted `#9AA29D`, Signal `#FF4F00`, Ok `#1B6E45`, Ok-dark `#3DBE7E`, Warn `#F2B200`, Fail `#C21F1A`, Fail-dark `#FF6B5E`.
2. THE Design_System SHALL define the text tokens `--signal-text #B83A00`, `--signal-text-dark #FF5F1A`, and `--warn-text #8A5A00`.
3. WHEN the contrast test runs, THE test SHALL compute each WCAG contrast ratio and match the ADR-011 table rounded to two decimals: on Plate, Ink 15.68, Steel 5.87, Edge 4.05, Signal 3.07, `--signal-text` 5.36, Ok 5.81, `--warn-text` 5.52, Fail 5.58; on Panel, Steel 5.40, `--signal-text` 4.94, Edge 3.73; on Graphite, On-dark 15.67, On-dark-muted 7.20, Signal 5.71, `--signal-text-dark` 6.19, Ok-dark 7.95, Warn 9.99, Fail-dark 6.74; and Graphite on Signal 5.71, Ink on Warn 8.94, white on Ok 6.25.
4. THE Design_System SHALL use Signal only for fills, focus rings, and Large text on Plate or Graphite (never on Panel), Edge only for control borders and icons, Rule only for dividers, and Warn only as a fill with Ink text or as text on Graphite.
5. THE Design_System SHALL use Ink, Steel, `--signal-text`, `--warn-text`, Ok, or Fail for all text below Large text size on Plate and Panel, and On-dark, On-dark-muted, `--signal-text-dark`, Ok-dark, Warn, or Fail-dark on Graphite.
6. IF any text and background pair used by a component has a contrast ratio below 4.5:1 for normal text or 3:1 for Large text, THEN the contrast test SHALL fail.

### Requirement 17: Typography and bundled fonts (§9 Type)

**User Story:** As a user, I want consistent, readable type that loads offline, so that the app looks right without any network access.

#### Acceptance Criteria

1. THE Design_System SHALL use Atkinson Hyperlegible Next for interface and brief text, Archivo (variable width and weight) for headings, and Atkinson Hyperlegible Mono for paths and hashes.
2. THE Design_System SHALL bundle each font file with its OFL license file.
3. IF the built CSS references a font URL with an `http:` or `https:` scheme, THEN the font test SHALL fail.

### Requirement 18: Spacing, elevation, and motion (§9 Spacing and motion)

**User Story:** As a user, I want a calm, consistent layout and short, quiet motion, so that the interface is easy to scan.

#### Acceptance Criteria

1. THE Design_System SHALL define spacing tokens as multiples of 4 px.
2. THE Design_System SHALL define corner radius tokens of 6, 10, and 14 px and exactly three elevation levels.
3. THE Design_System SHALL define motion durations between 120 and 200 ms with ease-out timing.
4. THE Design_System components SHALL use only the spacing, radius, elevation, and motion tokens for those properties.

### Requirement 19: Status markers (§9 Status markers, UX-1)

**User Story:** As a color-blind user, I want every status to have a symbol and a word, so that I never need color to understand a result.

#### Acceptance Criteria

1. THE Design_System SHALL render Verified with ✓ in Ok, Needs decision with ◆ on a Warn fill, Failed with ✕ in Fail, Unknown with ? in Steel, and Unsupported with ⊘ in Steel (Ok-dark and Fail-dark on Graphite).
2. THE Design_System SHALL render every Status marker as a glyph plus a visible text label.
3. THE Design_System SHALL render Status marker labels in a text token from Requirement 16.5.
4. THE Design_System SHALL render each Finding status (Confirmed, Inferred, Unknown, Unsupported) as a Status marker with a distinct glyph and label.
5. THE Design_System SHALL NOT use color as the only signal for any status, selection, error, or focus state.

### Requirement 20: Screen states and Gallery (§9 States and gallery)

**User Story:** As a user, I want every screen to explain what's happening when data is loading, missing, failed, partial, or cancelled, so that I'm never left with a blank window.

#### Acceptance Criteria

1. THE Design_System SHALL provide a loading, empty, error, partial, and cancelled state for every Screen.
2. WHERE a Screen state cannot occur on a Screen, THE State_Matrix SHALL record it as not reachable with the reason.
3. THE Gallery SHALL show every Design_System component and every reachable Screen state from the State_Matrix, rendered with MockClient data.
4. THE Gallery SHALL be present only in development builds, and IF a release bundle contains the `/gallery` route, THEN the build test SHALL fail.
5. WHEN the screenshot suite runs, THE suite SHALL capture every Gallery entry and compare it with the approved baseline.

### Requirement 21: Keyboard access (UX-1)

**User Story:** As a keyboard user, I want to do everything without a mouse, so that I can use DORMBIT efficiently and accessibly.

#### Acceptance Criteria

1. THE Design_System SHALL make every interactive element reachable and operable by keyboard alone.
2. THE Design_System SHALL show a visible focus indicator on the focused element with at least 3:1 contrast against its background.
3. THE Design_System SHALL support arrow-key navigation in Tabs, List, Table, and Tree, and Escape to close Dialog, Sheet, and Toast.
4. THE Design_System SHALL trap focus inside an open Dialog or Sheet and return focus to the triggering element on close.
5. THE Native_Shell SHALL provide native macOS menus with a keyboard shortcut for each top-level destination, including ⌘, for Settings.
6. THE Settings Screen SHALL list every keyboard shortcut.

### Requirement 22: VoiceOver labels (UX-1)

**User Story:** As a VoiceOver user, I want every control and status announced clearly, so that I can follow each workflow by ear.

#### Acceptance Criteria

1. THE Design_System SHALL give every interactive element an accessible name, including icon-only buttons.
2. THE Design_System SHALL expose each Status marker's text label to assistive technology and hide its glyph from it.
3. WHILE a Job runs, THE Design_System SHALL announce step changes, completion, failure, and cancellation through a polite live region, and SHALL NOT announce every progress count update.
4. THE Design_System SHALL give the Capsule_Seal an accessible name that includes the verification state.
5. THE Design_System SHALL mark up List and Table with roles, row and column headers, and total row counts that remain correct when rows are virtualized.

### Requirement 23: Reduced motion (UX-1)

**User Story:** As a user sensitive to motion, I want animation off when I ask for reduced motion, so that the app is comfortable to use.

#### Acceptance Criteria

1. WHILE the system setting "reduce motion" is on, THE Design_System SHALL set every transition and animation duration to 0 ms.
2. WHILE the system setting "reduce motion" is on, THE Design_System SHALL still show every state change through a static visual change and text.
3. WHEN the Playwright suite emulates `prefers-reduced-motion: reduce`, THE test SHALL find no running CSS animation or transition on any Screen.

### Requirement 24: WCAG AA conformance (P11, Task 47)

**User Story:** As a user with a disability, I want the app and the demo to meet WCAG AA, so that I can use all of DORMBIT.

#### Acceptance Criteria

1. WHEN axe runs on every reachable state of every Screen, including the Judge Demo, THE accessibility suite SHALL report zero critical violations.
2. WHEN Phase 11 closes, THE accessibility report SHALL list zero open violations for the WCAG 2.1 A and AA rule tags, with each excluded rule recorded as a false positive with its reason.
3. WHEN the keyboard-only walkthrough runs on journeys 1, 2, 3, 5, 6, and 9, THE walkthrough SHALL complete every step without a pointer.
4. WHEN the VoiceOver spot check runs on each native Screen, THE accessibility report SHALL record the result for each Screen.
5. THE accessibility report SHALL be written to `docs/evidence/P11-T02-accessibility.md`.

### Requirement 25: Error display (§8 DormbitError)

**User Story:** As a user, I want errors explained in plain words with the code shown, so that I know what happened and what to do next.

#### Acceptance Criteria

1. THE Error_Presenter SHALL map every stable DormbitError code in master plan §8 to a title, a plain-language explanation, and an optional action.
2. THE Error_Presenter SHALL show the DormbitError code next to the message in Atkinson Hyperlegible Mono.
3. WHERE a DormbitError has `retryable: true`, THE Error_Presenter SHALL offer a Retry action, and WHERE it has `retryable: false`, THE Error_Presenter SHALL NOT offer one.
4. IF the Error_Presenter receives a code it does not know, THEN it SHALL show a generic message that includes the code.
5. WHEN a Job ends with `CANCELLED`, THE Error_Presenter SHALL show the cancelled Screen state, not an error state.

### Requirement 26: Client layer (TauriClient, Zod, TanStack Query)

**User Story:** As a developer of DORMBIT, I want one typed client interface with validated responses, so that the screens work the same against the mock, Tauri, and WebAssembly backends.

#### Acceptance Criteria

1. THE TauriClient SHALL implement the DormbitClient interface and call only allowlisted Commands.
2. THE TauriClient SHALL validate every Command response and every Job event with the Zod schemas from `packages/schema` before returning it.
3. IF a response or event fails Zod validation, THEN THE TauriClient SHALL reject with a DormbitError with `retryable: false`.
4. IF a Command rejects with a value that is not a valid DormbitError, THEN THE TauriClient SHALL reject with a DormbitError with `retryable: false`.
5. THE TauriClient SHALL send folder and capsule references as IDs and SHALL send no absolute path to any Command.
6. THE Screens SHALL read data through TanStack Query hooks, and SHALL NOT call DormbitClient methods from render code.
7. THE App_Shell SHALL retry a failed query only when its DormbitError has `retryable: true`, and SHALL NOT retry mutations automatically.
8. WHEN the shared client contract suite runs against MockClient and TauriClient (with a mocked `invoke`), THE suite SHALL pass for both.

### Requirement 27: Visual QA (P11, Task 46)

**User Story:** As the DORMBIT team, I want every screen and state checked visually at 1x and 2x, natively and on the web, so that defects are found before release.

#### Acceptance Criteria

1. WHEN visual QA runs, THE QA run SHALL capture every reachable Screen state natively with `screencapture` and a Swift window-ID helper, at 1x and 2x.
2. WHEN visual QA runs, THE QA run SHALL capture every reachable Screen state of the web build through Playwright MCP at 1x and 2x.
3. THE QA run SHALL use only synthetic fixture data, so that no screenshot shows a real user project or a path outside `~`-redacted Display paths.
4. THE QA run SHALL record each defect in `docs/evidence/P11-T01-visual-qa.md` with Screen, state, scale, and before and after images.
5. WHEN Phase 11 closes, THE defect log SHALL have no open defects.

### Requirement 28: Copy audit against the four outcomes (P11, Task 48)

**User Story:** As a judge or user, I want DORMBIT's wording to claim only what it does, so that I can trust its reports.

#### Acceptance Criteria

1. THE Copy_Audit SHALL review every user-facing string in `packages/ui`, `apps/desktop`, and `apps/demo`.
2. THE Copy_Audit SHALL fail any string that claims a restored project builds, runs, or works.
3. THE Copy_Audit SHALL fail any string that claims an AI session was resumed, and SHALL require session references to read "not resumed by DORMBIT".
4. THE Copy_Audit SHALL fail any string that describes context as restored rather than "reconstructed (inferred from evidence)".
5. THE Copy_Audit SHALL fail any string that claims encryption, signing, or proof of authorship.
6. THE Copy_Audit SHALL fail any string that states a project's intent or history without a finding, including "abandoned" or "inactive" labels derived from timestamps.
7. WHEN the copy check test runs, THE test SHALL fail on any denylisted phrase in the string modules.
8. THE Copy_Audit SHALL record its result in `docs/evidence/P11-T03-copy-audit.md`.

### Requirement 29: Accuracy validation on public repos (P1)

**User Story:** As the DORMBIT team, I want to measure finding accuracy on real public code, so that the accuracy claim in the submission is backed by data.

#### Acceptance Criteria

1. WHERE accuracy validation is performed, THE Accuracy_Validation SHALL use 5 public open-source repos with different stacks.
2. THE Accuracy_Validation SHALL use shallow clones with Git LFS smudge skipped, stored under `.scratch/validation/`.
3. THE Accuracy_Validation SHALL NOT run, build, or install anything from the cloned repos.
4. THE Accuracy_Validation SHALL score every Confirmed finding as correct or incorrect against its cited source, with a target of at least 90% correct.
5. THE Accuracy_Validation SHALL record whether every Inferred finding cites evidence.
6. THE Accuracy_Validation SHALL write its scoring sheet and result, including a missed target, to `docs/product/validation.md`.
7. IF the user's own projects are included, THEN THE Accuracy_Validation SHALL do so only after the user gives consent at that time.

## Open questions
1. **Glyphs for Confirmed and Inferred.** Master plan §9 lists five Status markers but no glyph for the Finding statuses Confirmed and Inferred. Requirement 19.4 requires a distinct glyph and label for each; the glyphs are chosen in Task 34 and must not reuse ✓, which means byte-verified. Confirm the choice at the P08 gate.
2. **States that can't occur.** §9 says every Screen has loading, empty, error, partial, and cancelled states. Some can't occur on some Screens (for example, cancelled on Welcome). Requirement 20.2 records those as not reachable with a reason instead of inventing a fake state.
3. **Capsules and Restores sidebar items.** §9 names these sidebar items, but the 13-screen list has no separate capsule list or restore history screen. This spec treats them as the list states of the Integrity Verification and Restore Wizard Screens and adds no screen.
4. **"12 native screens" vs "13 screens".** The P08 exit says 12 native screens; UX-1 says 13. Both hold: screens 1-12 are native, and screen 13 (Judge Demo) is built in the judge-demo spec. This spec's accessibility and copy audits include screen 13.
5. **Unverified-in-registry capsules.** VER-1 reports a capsule missing from the registry as Unknown. This spec presses the seal only when the overall VerificationReport status is Verified, and shows the registry result as its own marker. The capsule-and-verification spec decides whether a not-in-registry capsule can have overall status Verified.
