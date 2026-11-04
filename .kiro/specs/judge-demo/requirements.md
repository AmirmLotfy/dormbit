# Requirements Document

## Introduction
This spec covers the public judge demo: master plan Phase 09 (P09, Tasks 38-41). It formalizes DEM-1, DEM-2, and DEM-3, and user journey 8, the guided 12-step walkthrough. The demo runs the real Rust core (`dormbit-core`) compiled to WebAssembly over an in-memory filesystem, using the synthetic Dockline Invoices fixture, and is hosted as a static site on GitHub Pages. It reuses the `packages/ui` Screens from the native-experience spec through a `WasmClient` that implements the same `DormbitClient` interface. It carries PROP-10 (WebAssembly capsule bytes equal native golden bytes) and two demo-specific properties: reset idempotence and rejection of every single-byte corruption control.

Journey 8, the 12 steps:
1. Discover Dockline.
2. Open its details.
3. Inspect the stack and the unfinished work.
4. Review the plan.
5. Approve it.
6. Run the capture state machine.
7. Verify integrity.
8. Restore to a virtual destination, including a refused collision with an existing folder.
9. Inspect the independent verification.
10. Read the brief.
11. Run a corruption test.
12. Observe the failure, then reset.

## Glossary
- **Judge_Demo**: the static web app in `apps/demo`, served from GitHub Pages under `/dormbit/demo/`.
- **Landing_Page**: the static page at `/dormbit/` (`apps/demo/index.html`) that introduces DORMBIT and links to the Judge_Demo and the macOS download (ADR-011).
- **Visitor**: anyone using the Judge_Demo in a browser, usually a hackathon judge.
- **Demo_Shell**: Screen 13 (Judge Demo): the Synthetic_Banner, Guided_Rail, Whats_Real_Panel, Reset_Control, and the area that shows the current step's `packages/ui` Screen.
- **Synthetic_Banner**: the banner with the exact text "Synthetic Demo — No Local Files Accessed".
- **Guided_Rail**: the ordered list of the 12 journey-8 steps with each step's progress.
- **Whats_Real_Panel**: the panel that states which parts of the demo are real computation and which are precomputed, mocked, or paced.
- **Mock_Picker**: the labeled dialog that stands in for the native folder picker and offers only virtual folders.
- **Wasm_Core**: the `dormbit-wasm` crate: `dormbit-core` plus MemVfs plus step jobs, compiled to `wasm32-unknown-unknown` with wasm-bindgen.
- **MemVfs**: the in-memory filesystem used by the Wasm_Core: the core's `MemVfs` (read side, from scope-and-discovery) and `MemRestoreFs` (write side, from transactional-restore) over one shared tree.
- **WasmClient**: the `DormbitClient` implementation in `apps/demo` that calls the Wasm_Core and validates every result with Zod.
- **Step_Job_Runner**: the TypeScript loop that advances Wasm_Core jobs in small budgets during browser idle time.
- **Work budget**: the maximum bytes and operations one `step` call may process (the core's `Budget { max_bytes, max_ops }`).
- **Independent_Rehash**: the TypeScript module that hashes bytes with WebCrypto `crypto.subtle.digest('SHA-256')`, a different SHA-256 implementation from the Rust `sha2` crate the writer uses.
- **Corruption_Controls**: the three controls that damage virtual bytes: Damage a stored file, Damage capsule bytes, and Damage after verification.
- **Reset_Control**: the control that returns the demo to the Pristine state.
- **Pristine state**: the demo state right after the dataset first loads: MemVfs holds the fixture tree and the pre-seeded virtual folders, with no grants, plans, capsules, journals, or restores, and the Guided_Rail at step 1.
- **State digest**: a SHA-256 over a canonical encoding of the MemVfs tree (paths, bytes, modes), registry, journals, plans, grants, and rail progress.
- **Dockline fixture**: the synthetic Dockline Invoices project tree committed as `fixtures/dockline/dockline-invoices.tar` (DEM-3).
- **Sidecar**: the JSON file written next to the Dockline tar by the foundation generator. It holds the precomputed Git snapshot, synthetic agent metadata, the fixed demo clock, and expected hashes.
- **Virtual path**: a MemVfs path, shown with the prefix `demo:/` so it can't be mistaken for a path on the Visitor's device.
- **Virtual destination**: the virtual parent folder `demo:/Restores`, pre-seeded with an existing folder named `dockline-invoices`.
- **Golden capsule**: the native capsule bytes for the Dockline fixture produced under fixed inputs by the capsule-and-verification spec.
- **Golden inputs**: the fixed clock, fixed ID sequence, and fixed plan decisions used to produce the Golden capsule.
- **Pacing**: a minimum display time added to a step so a human can follow it.
- **Real elapsed time**: the wall time from a job's first `step` call to its final result, measured with `performance.now()`.
- **Screen**: a `packages/ui` screen from the native-experience spec (Shelf, Evidence View, Preservation Plan, Capsule Creation, Integrity Verification, Restore Wizard, Restoration Result, Resurrection Brief) shown inside the Demo_Shell.
- **Capsule**: a `.dormbit` file in capsule format v1 (master plan §8). In the demo it exists only in MemVfs.
- **Capsule_Seal**: the native-experience component that draws a fingerprint from the capsule ID and appears pressed only after verification passes.
- **VerificationReport**: the verifier's result `{capsule_id, file_sha256, manifest_sha256, checks[], failed_files[], registry_match, status}`.
- **Four outcomes**: the separate BRF-2 results: "Files restored and verified", "Context reconstructed (inferred from evidence)", "AI session resumed: not performed", "Builds/runs: unknown (never executed)".

## Requirements

### Requirement 1: Synthetic banner always visible (DEM-1)

**User Story:** As a Visitor, I want a constant notice that the demo is synthetic, so that I never think it is reading my computer.

#### Acceptance Criteria

1. THE Demo_Shell SHALL show the Synthetic_Banner with the exact text "Synthetic Demo — No Local Files Accessed".
2. THE Demo_Shell SHALL keep the Synthetic_Banner visible on every route, at every supported viewport size, and while scrolled.
3. WHILE a Dialog, Sheet, or Toast is open, THE Demo_Shell SHALL keep the Synthetic_Banner visible above it.
4. THE Synthetic_Banner SHALL offer no control that hides or dismisses it.
5. THE Synthetic_Banner SHALL be exposed to assistive technology as a landmark with its full text.

### Requirement 2: No access to the Visitor's computer (DEM-1)

**User Story:** As a Visitor, I want the demo to never touch or claim to inspect my files, so that I can try it without risk.

#### Acceptance Criteria

1. THE Judge_Demo SHALL NOT call the File System Access API, use file inputs, or read dropped files.
2. THE Judge_Demo SHALL show every path as a Virtual path with the `demo:/` prefix.
3. THE Judge_Demo SHALL NOT contain wording that claims to scan, read, or inspect the Visitor's computer, disk, or files.
4. IF the production bundle contains `showDirectoryPicker`, `showOpenFilePicker`, `showSaveFilePicker`, `webkitdirectory`, or `type="file"`, THEN the bundle check SHALL fail.
5. THE Judge_Demo SHALL write to the Visitor's device only through a browser download that the Visitor starts by clicking a download control.

### Requirement 3: Real core in WebAssembly over MemVfs (DEM-1, ADR-001)

**User Story:** As a judge, I want the demo to run the same code as the native app, so that what I test in the browser is the real product logic.

#### Acceptance Criteria

1. THE Wasm_Core SHALL run `dormbit-core`'s planner, capsule writer, verifier, restore planner, evidence rules, and brief generator compiled to `wasm32-unknown-unknown`.
2. THE Wasm_Core SHALL perform every filesystem operation through MemVfs, using the core's read and restore-write traits.
3. THE MemVfs SHALL implement exclusive rename that fails when the target name exists.
4. THE MemVfs SHALL report its volume as case-insensitive, supporting exclusive rename, and with a fixed capacity larger than the dataset.
5. THE Wasm_Core SHALL take its clock from the Sidecar's fixed demo clock and its IDs from the host.
6. THE Wasm_Core SHALL use `flate2` with the `rust_backend` feature only and SHALL depend on `wasm-bindgen` pinned to `=0.2.130`.
7. IF a new Wasm_Core dependency fails `cargo build --target wasm32-unknown-unknown`, THEN CI SHALL fail.

### Requirement 4: Dockline dataset (DEM-3)

**User Story:** As a judge, I want a realistic synthetic project to work with, so that every DORMBIT feature has something meaningful to show.

#### Acceptance Criteria

1. WHEN the Judge_Demo starts, THE Wasm_Core SHALL load the Dockline fixture from the static copy of `fixtures/dockline/dockline-invoices.tar` and the Sidecar into MemVfs under `demo:/Projects/dockline-invoices`.
2. THE loaded tree SHALL contain a React + Vite + TypeScript frontend manifest, a Fastify API manifest, and SQLite via Drizzle manifests, with no installed dependencies.
3. THE loaded tree SHALL contain a Git repository with about 24 commits by fixed authors with fictional spring 2025 dates, branches `main` and `feature/pdf-export`, and `feature/pdf-export` checked out.
4. THE loaded tree SHALL contain 2 modified files, 1 untracked note, and 1 stash.
5. THE loaded tree SHALL contain `README`, `docs/ROADMAP.md`, and `CHANGELOG`.
6. THE loaded tree SHALL contain TODO and FIXME comments in `src/pdf/renderInvoice.ts`, a skipped test, and `.kiro/specs/pdf-export/tasks.md` with 3 of 7 tasks done.
7. THE loaded tree SHALL contain a `.env` with obviously fake values and a `.env.example`.
8. THE loaded tree SHALL contain `data/dev.sqlite` and placeholder `dist/` and `node_modules/` folders.
9. THE Sidecar SHALL supply the precomputed Git snapshot, synthetic Claude Code store metadata, synthetic Kiro export metadata, the fixed demo clock, and expected hashes.
10. WHEN the dataset loads, THE Wasm_Core SHALL compare each file's SHA-256 with the Sidecar's expected hash.
11. IF the tar or Sidecar fails to load or parse, or any expected hash differs, THEN THE Demo_Shell SHALL show an error state naming the failure and SHALL NOT start step 1.

### Requirement 5: Labeled mock picker (DEM-2)

**User Story:** As a Visitor, I want to see clearly that the folder picker is a stand-in, so that I'm not confused about what the demo can reach.

#### Acceptance Criteria

1. WHEN a step needs a folder, THE Mock_Picker SHALL open as a Dialog titled as a mock folder picker that states it shows virtual folders only.
2. THE Mock_Picker SHALL list only Virtual paths in MemVfs.
3. WHEN the Visitor confirms a folder in the Mock_Picker, THE WasmClient SHALL return a scope grant for that virtual folder through the same `DormbitClient` method the native app uses.
4. IF the Visitor cancels the Mock_Picker, THEN THE WasmClient SHALL return no grant and the current step SHALL stay unchanged.

### Requirement 6: Real hashes and independent re-hash (DEM-2)

**User Story:** As a judge, I want the hashes I see to be computed for real and checked by separate code, so that I can trust the verification claim.

#### Acceptance Criteria

1. THE Judge_Demo SHALL show only hashes computed by the Wasm_Core or the Independent_Rehash from bytes in MemVfs, and SHALL NOT show a hard-coded hash as a computed result.
2. WHEN a restore completes, THE Independent_Rehash SHALL hash every restored file with WebCrypto and compare it with the manifest SHA-256.
3. WHEN a capsule is verified, THE Independent_Rehash SHALL hash the whole capsule with WebCrypto and compare it with the verifier's `file_sha256`.
4. THE Judge_Demo SHALL show each WebCrypto hash next to the Rust hash, with a label naming which implementation produced each.
5. IF any WebCrypto hash differs from the Rust hash, THEN THE Judge_Demo SHALL show Failed for that item and name it.

### Requirement 7: Pacing labeled with real elapsed time (DEM-2)

**User Story:** As a judge, I want to know when the demo slows down for readability, so that I see how fast the real work was.

#### Acceptance Criteria

1. WHERE a step adds Pacing, THE Demo_Shell SHALL label the step "paced for readability" and show its Real elapsed time in milliseconds.
2. THE Step_Job_Runner SHALL measure Real elapsed time from the first `step` call to the final result, excluding Pacing.
3. THE Demo_Shell SHALL show progress counts only from real file and byte counts reported by the Wasm_Core.

### Requirement 8: Guided rail and steps 1-5 (journey 8)

**User Story:** As a first-time Visitor, I want a guided path through the workflow, so that I can complete all 12 steps without help.

#### Acceptance Criteria

1. THE Guided_Rail SHALL list the 12 journey-8 steps in order with number and title.
2. THE Guided_Rail SHALL mark each step as done, current, or locked with a glyph and a text label.
3. IF the Visitor opens a step whose earlier steps are not done, THEN THE Demo_Shell SHALL open the first step that is not done and state which step is required.
4. WHEN step 1 runs, THE Demo_Shell SHALL open the Mock_Picker, run discovery in the Wasm_Core over the chosen virtual folder, and show Dockline Invoices on the Shelf.
5. WHEN step 2 runs, THE Demo_Shell SHALL show the Evidence View overview for Dockline Invoices, including the checked-out branch `feature/pdf-export`.
6. WHEN step 3 runs, THE Demo_Shell SHALL show the stack findings with cited manifests, the TODO and FIXME findings in `src/pdf/renderInvoice.ts` with line ranges, the skipped test, and "3 of 7 tasks complete".
7. WHEN step 4 runs, THE Demo_Shell SHALL show the plan with `.env` in Excluded: sensitive, `data/dev.sqlite` in Needs decision, and `dist/` and `node_modules/` in Excluded: rebuildable.
8. WHILE `data/dev.sqlite` has no decision, THE Demo_Shell SHALL keep Approve disabled in step 5 and state the reason.
9. WHEN the Visitor approves in step 5, THE Wasm_Core SHALL record the plan digest and acknowledgments, and THE Demo_Shell SHALL show the digest.

### Requirement 9: Steps 6-10: capture, verify, restore, independent check, brief (journey 8)

**User Story:** As a judge, I want to watch the real capture, verification, and restore, so that I can judge the safety claims myself.

#### Acceptance Criteria

1. WHEN step 6 runs, THE Wasm_Core SHALL run the capture state machine as a step job and THE Demo_Shell SHALL show each capture state.
2. WHEN step 7 runs, THE Wasm_Core SHALL run the separate verifier on the capsule bytes in MemVfs, and THE Demo_Shell SHALL show the VerificationReport and the Capsule_Seal pressed only if the status is Verified.
3. WHEN step 8 runs with a new folder name in the Virtual destination, THE Wasm_Core SHALL run pre-checks, stage, verify, and commit the restore with exclusive rename in MemVfs.
4. WHEN step 9 runs, THE Demo_Shell SHALL show the Independent_Rehash result for every restored file and the HEAD and refs check against the manifest.
5. THE step 9 view SHALL show the working-tree status check as not run in the browser, because it runs natively only.
6. WHEN step 10 runs, THE Demo_Shell SHALL show the Resurrection Brief with the Four outcomes as separate items.

### Requirement 10: Refused restore into an existing folder (step 8, RST-1)

**User Story:** As a judge, I want to see DORMBIT refuse to overwrite an existing folder, so that I can trust it never clobbers work.

#### Acceptance Criteria

1. THE Pristine state SHALL include an existing folder `demo:/Restores/dockline-invoices` with content.
2. WHEN the Visitor requests a restore into `demo:/Restores` named `dockline-invoices`, THE Wasm_Core SHALL refuse it with `RESTORE_DEST_EXISTS`.
3. WHEN the Visitor requests a restore named with a case variant of the existing folder, THE Wasm_Core SHALL refuse it with `RESTORE_DEST_EXISTS`.
4. WHEN a restore is refused, THE existing folder's paths, bytes, and modes SHALL be unchanged.
5. WHEN a restore is refused, THE Demo_Shell SHALL show the error code and the reason, and let the Visitor enter a new name.

### Requirement 11: Corruption controls (steps 11-12, journey 6)

**User Story:** As a judge, I want to damage a capsule myself and watch DORMBIT catch it, so that I can test the integrity claims directly.

#### Acceptance Criteria

1. THE Corruption_Controls SHALL offer three controls: Damage a stored file, Damage capsule bytes, and Damage after verification.
2. WHEN the Visitor uses any Corruption_Control, THE Demo_Shell SHALL show the affected file or region, the byte offset, and the old and new byte values.
3. WHEN the Visitor uses Damage a stored file, THE Wasm_Core SHALL change one byte inside one stored object in the capsule in MemVfs.
4. WHEN verification runs after Damage a stored file, THE verifier SHALL report Failed and name the damaged file.
5. WHEN the Visitor uses Damage capsule bytes, THE Wasm_Core SHALL change one byte of the capsule outside object data.
6. WHEN verification runs after Damage capsule bytes, THE verifier SHALL report Failed with a DormbitError code of `CAPSULE_MALFORMED`, `CAPSULE_TRUNCATED`, or `CAPSULE_HASH_MISMATCH`.
7. WHILE the capsule's latest verification is Failed, THE Wasm_Core SHALL refuse to start a restore from it.
8. WHEN the Visitor uses Damage after verification on a Verified capsule, THE Wasm_Core SHALL change one byte of a stored object after restore pre-checks pass and before the restore reads that object.
9. WHEN the restore reads the damaged object, THE Wasm_Core SHALL fail the restore with `CAPSULE_HASH_MISMATCH`, roll back, and remove its staging folder.
10. WHEN a restore fails after Damage after verification, THE destination folder SHALL NOT exist in MemVfs, and THE restore report SHALL show 0 writes outside the destination.
11. THE Corruption_Controls SHALL change only bytes in MemVfs and SHALL state that they simulate damage to virtual bytes.

### Requirement 12: Reset (DEM-1)

**User Story:** As a Visitor, I want to start over cleanly at any time, so that I can repeat a test or try another corruption control.

#### Acceptance Criteria

1. THE Demo_Shell SHALL make the Reset_Control reachable from every step by pointer and keyboard.
2. WHEN the Visitor activates the Reset_Control, THE Judge_Demo SHALL cancel any running job and return to the Pristine state.
3. WHEN reset completes, THE State digest SHALL equal the State digest recorded right after the first dataset load.
4. WHEN reset runs twice in a row, THE State digest after the second reset SHALL equal the State digest after the first.
5. WHEN reset completes, THE Guided_Rail SHALL show step 1 as current and every other step as not done.
6. THE Reset_Control SHALL rebuild the Pristine state from the dataset bytes already in memory, with no network request.

### Requirement 13: Capsule and brief downloads (DEM-1)

**User Story:** As a judge, I want to download the capsule and the brief, so that I can inspect them outside the browser.

#### Acceptance Criteria

1. WHEN the Visitor downloads the capsule, THE Judge_Demo SHALL save a `.dormbit` file whose bytes equal the capsule bytes in MemVfs.
2. THE downloaded capsule's SHA-256 SHALL equal the `file_sha256` the Demo_Shell shows for it.
3. IF a Corruption_Control has changed the capsule, THEN THE download control SHALL label the file as a damaged copy.
4. WHEN the Visitor downloads the brief, THE Judge_Demo SHALL save a Markdown file equal to the brief rendered by the Wasm_Core.

### Requirement 14: What's real panel (§9 screen 13)

**User Story:** As a judge, I want a plain statement of what is real and what is simulated, so that I can weigh what I'm seeing.

#### Acceptance Criteria

1. THE Whats_Real_Panel SHALL state that the planner, capsule writer, verifier, restore, evidence rules, brief, and hashes are real Rust code compiled to WebAssembly.
2. THE Whats_Real_Panel SHALL state that the files are synthetic and live only in browser memory.
3. THE Whats_Real_Panel SHALL state that the folder picker is a mock, the Git snapshot and agent metadata are precomputed from the fixture, the clock is fixed, and the working-tree status check runs natively only.
4. THE Whats_Real_Panel SHALL state which steps use Pacing.
5. THE Whats_Real_Panel SHALL state that the independent re-hash uses WebCrypto, a separate SHA-256 implementation.

### Requirement 15: No auth, no analytics, no outside requests, no storage

**User Story:** As a Visitor, I want the demo to collect nothing about me, so that I can use it freely.

#### Acceptance Criteria

1. THE Judge_Demo SHALL require no sign-in and contain no analytics or tracking code.
2. THE Judge_Demo SHALL request only its own static assets from its own origin under `/dormbit/`.
3. THE Judge_Demo SHALL NOT write cookies, `localStorage`, `sessionStorage`, or IndexedDB.
4. WHEN the Visitor reloads the page, THE Judge_Demo SHALL start in the Pristine state.
5. WHEN the E2E suite runs, THE suite SHALL fail on any request outside the demo's origin and base path.

### Requirement 16: Browsers and responsive layout

**User Story:** As a judge on any modern browser or screen size, I want the demo to work, so that I can evaluate it on the device I have.

#### Acceptance Criteria

1. THE Judge_Demo SHALL complete all 12 steps on current Chromium, WebKit, and Firefox.
2. THE Judge_Demo SHALL complete all 12 steps at a phone, a tablet, and a desktop viewport size.
3. WHILE the viewport is too narrow for the Guided_Rail beside the content, THE Demo_Shell SHALL collapse the Guided_Rail into a step menu that shows the current step number and title.
4. IF the browser lacks WebAssembly or WebCrypto `subtle`, THEN THE Demo_Shell SHALL show a message naming the missing feature and SHALL NOT start step 1.

### Requirement 17: Accessibility (UX-1)

**User Story:** As a Visitor using a keyboard or a screen reader, I want the demo to be fully usable, so that I can evaluate it like anyone else.

#### Acceptance Criteria

1. WHEN axe runs on every step in each browser, THE E2E suite SHALL report zero critical violations.
2. THE Demo_Shell SHALL let the Visitor complete all 12 steps, the Corruption_Controls, Reset, and downloads by keyboard alone.
3. THE Demo_Shell SHALL announce step changes and job completion through a polite live region.
4. WHILE `prefers-reduced-motion: reduce` is set, THE Demo_Shell SHALL run no animation or transition.
5. THE Demo_Shell SHALL pair every status with a glyph and a text label, never color alone.

### Requirement 18: Content Security Policy

**User Story:** As a judge, I want the demo locked down by a strict CSP, so that it can't load or run anything beyond its own code.

#### Acceptance Criteria

1. THE Judge_Demo SHALL set its CSP in a `<meta http-equiv="Content-Security-Policy">` tag.
2. THE CSP SHALL allow WebAssembly compilation through `'wasm-unsafe-eval'` in `script-src` and SHALL NOT include `'unsafe-eval'` or `'unsafe-inline'` in `script-src`.
3. THE CSP SHALL limit `connect-src`, `script-src`, `font-src`, and `style-src` to `'self'`, and set `object-src 'none'` and `base-uri 'self'`.
4. WHEN the E2E suite runs in Chromium, WebKit, and Firefox, THE suite SHALL fail on any `securitypolicyviolation` event.
5. WHEN the E2E suite runs in WebKit, THE Wasm_Core SHALL load and complete step 1 under the CSP.

### Requirement 19: Step jobs on idle callbacks

**User Story:** As a Visitor, I want the page to stay responsive while the core works, so that the demo never freezes.

#### Acceptance Criteria

1. THE Step_Job_Runner SHALL advance capture, verify, restore, and discovery jobs by calling the Wasm_Core `step` function with a Work budget.
2. WHERE `requestIdleCallback` is available, THE Step_Job_Runner SHALL schedule `step` calls in idle callbacks.
3. WHERE `requestIdleCallback` is not available, THE Step_Job_Runner SHALL schedule `step` calls with a macrotask fallback using the same Work budget.
4. WHEN the Visitor cancels a job or resets, THE Step_Job_Runner SHALL stop scheduling `step` calls for that job and THE Wasm_Core SHALL report `CANCELLED`.

### Requirement 20: GitHub Pages hosting

**User Story:** As a judge, I want a stable public URL, so that I can open the demo with no install.

#### Acceptance Criteria

1. THE Judge_Demo SHALL build with Vite `base: '/dormbit/'`.
2. THE Judge_Demo SHALL use hash-based routes inside `/dormbit/demo/`, so that every step URL loads without server-side rewrites.
3. WHEN `pages.yml` runs, THE workflow SHALL build the Wasm_Core, build `apps/demo`, and deploy the static output to GitHub Pages.
4. WHEN the preview is deployed, THE smoke test SHALL load the live URL, show the Synthetic_Banner, and complete step 1.
5. THE build SHALL produce two pages: the Landing_Page at `/dormbit/` and the Judge_Demo at `/dormbit/demo/`.

### Requirement 21: Native parity (PROP-10)

**User Story:** As a judge, I want proof that the browser builds the same capsule as the native app, so that the demo is evidence about the product.

#### Acceptance Criteria

1. WHEN the Wasm_Core captures the Dockline fixture with the Golden inputs, THE resulting capsule bytes SHALL equal the Golden capsule bytes.
2. WHEN the differential test runs, THE Wasm_Core SHALL produce, for every generated input in the corpus, capsule bytes equal to the native build's bytes for the same input.
3. WHEN the E2E suite runs in each browser, THE in-browser capsule built with the Golden inputs SHALL have the Golden capsule's SHA-256 and length.
4. IF any parity check differs, THEN CI SHALL fail and report the first differing byte offset.

### Requirement 22: Landing page (ADR-011)

**User Story:** As a judge arriving from Devpost, I want a page that explains DORMBIT in a minute and takes me to the demo or the download, so that I know what I'm evaluating before I start.

#### Acceptance Criteria

1. THE Landing_Page SHALL link to the Judge_Demo at `/dormbit/demo/` and to the GitHub release that holds the macOS build.
2. THE Landing_Page SHALL meet Requirement 15 (no sign-in, no analytics, no outside requests, no storage) and Requirement 18 (Content Security Policy).
3. WHERE WebGL is available, THE Landing_Page SHALL show the hero capsule as a 3D object that turns when the Visitor drags it.
4. IF WebGL is unavailable or its context is lost, THEN THE Landing_Page SHALL show the static capsule drawing in the same place.
5. THE Landing_Page SHALL load the 3D code only after the hero text has rendered.
6. WHILE `prefers-reduced-motion: reduce` is set, THE Landing_Page SHALL run no automatic motion (no sway, inertia, scroll-linked movement, gradient drift, or reveal effects), and dragging SHALL turn the capsule without easing.
7. WHERE the browser supports scroll-driven animations, THE Landing_Page SHALL reveal sections with them, and WHERE it does not, THE Landing_Page SHALL show every section without animation.
8. THE Landing_Page SHALL render without horizontal scrolling at 360, 390, 768, 1024, and 1440 px wide.
9. THE Landing_Page SHALL label the sample verification report and sample capsule as samples.

## Open questions
1. **Sidecar file name and corrupted-capsule path.** DEM-3 and Task 12 describe a JSON sidecar and a prepared corrupted capsule but give no file names. This spec reads the sidecar from the path the foundation generator writes and uses the prepared corrupted capsule as a test oracle only. The foundation spec fixes both paths.
2. **Host-generated IDs.** ADR-001 says IDs come from the host (`crypto.randomUUID()` in the browser). The live demo follows that, so its capsule hash differs from the Golden capsule on every run. PROP-10 checks use the Golden inputs, including the fixed ID sequence. The Whats_Real_Panel does not claim the live capsule equals the Golden capsule.
3. **Error code for damage after verification.** Requirement 11.9 expects `CAPSULE_HASH_MISMATCH` when the restore's re-hash catches a changed object. transactional-restore defines the exact code; if it differs, this spec follows it.
4. **Preview vs production on Pages.** Task 41 deploys a "Pages preview" and Task 51 deploys "Pages production", but one repo has one Pages site. This spec deploys the preview to the same site with a visible build label and leaves promotion to the release-and-submission spec.
5. **Repo name.** The base path `/dormbit/` assumes the repo is named `dormbit`. If Task 5 used the fallback `dormbit-app`, the base path changes to `/dormbit-app/`.
6. **Downloads and the banner.** The banner says no local files are accessed. A download writes one file through the browser, only when the Visitor clicks. Requirement 2.5 limits writes to that case, and the download control says so.
