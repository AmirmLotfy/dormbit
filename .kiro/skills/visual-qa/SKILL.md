---
name: visual-qa
description: Screenshot every screen state of the DORMBIT UI at 1x and 2x, compare against the approved baselines and ADR-011 rules, and log each defect. Use for UI changes, before phase gates, and when asked for visual QA.
---

# Visual QA

1. Build and serve the surface under test:
   - landing and demo: `pnpm --filter @dormbit/demo build`, then `pnpm --filter @dormbit/demo exec vite preview --host 127.0.0.1 --port 4173 --strictPort`
   - desktop UI without Rust: `VITE_DORMBIT_CLIENT=mock pnpm --filter @dormbit/desktop dev`
2. With the `playwright` MCP server (isolated, output in `.scratch/playwright`), open every route and every state on the dev-only `/gallery` (loading, empty, error, partial, cancelled, ready).
3. Capture each at 1440×900 (1x), 1440×900 (2x), and 390×844 (2x). Repeat with `prefers-reduced-motion: reduce`.
4. Check each shot against:
   - ADR-011 tokens only; Signal orange never used as a status; every status has a glyph and a word
   - no horizontal scroll, no clipped text, no overlapping elements at any width
   - focus rings visible on every control; contrast per the ADR-011 table
   - nothing animates under reduced motion
5. Compare with the baselines from the screenshot suite. For every defect, record: route, state, viewport, what is wrong, and the screenshot path, in `docs/kiro/usage-log.md` under the task ID.
6. Never point the browser at real user folders or outside `http://localhost:*` and `https://amirmlotfy.github.io`.
