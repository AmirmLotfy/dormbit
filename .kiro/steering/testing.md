---
inclusion: fileMatch
fileMatchPattern: ["**/*.test.ts", "**/*.test.tsx", "**/*.test-d.ts", "**/*.spec.ts", "**/tests/**", "e2e/**"]
---

# Testing

- **Layers:** Rust unit and property tests (proptest), insta snapshots via `golden!` into `fixtures/golden/<area>/`, Vitest (node project for logic, jsdom for UI), compile-time type tests (`*.test-d.ts`, run by `pnpm typecheck`), PROP-11 schema parity, Playwright + axe E2E in Chromium, WebKit, Firefox, and iPhone 13.
- **Case counts:** `PROPTEST_CASES` and `FC_NUM_RUNS` default 256 locally, 512 in CI, 10,000 in Phase 10. Never lower them to make a test pass.
- **Snapshots:** CI runs with `INSTA_UPDATE=no`, so a changed snapshot fails without writing. Accept new values locally with `INSTA_UPDATE=always` and review the diff.
- **Fixtures:** use `fixtures/dockline/` (fictional) and its Sidecar answers; never real user folders or real agent stores.
- **Evidence:** a result counts only when run through `scripts/evidence/run.sh` with `EVIDENCE_TASK` set, from a clean tree. Record the property-test counterexamples you find in `docs/kiro/usage-log.md`.
- Test names describe behavior ("refuses a restore into an existing folder"), and each test checks one rule.
