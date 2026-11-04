---
inclusion: fileMatch
fileMatchPattern: "packages/ui/**"
---

# UI design ("Flight Recorder", ADR-011)

Until `packages/ui/src/tokens/tokens.ts` exists (native-experience Task 34), the source of truth is master plan §9 and `docs/adr/ADR-011-visual-direction.md`.

#[[file:docs/adr/ADR-011-visual-direction.md]]

- Use only the tokens; no new colors. Signal orange is for fills, focus rings, and Large text, and never a status.
- Status markers: Verified ✓ (Ok), Needs decision ◆ (Warn fill, `--warn-text` label), Failed ✕ (Fail), Unknown ? and Unsupported ⊘ (Steel).
- Type: Archivo for headings, Atkinson Hyperlegible Next for interface and brief text, Atkinson Hyperlegible Mono for paths and hashes. Fonts load from `packages/ui/src/fonts/` only.
- App motion: 120–200 ms ease-out, none under reduced motion. Every screen has loading, empty, error, partial, and cancelled states, shown on the dev-only `/gallery`.
- Avoid the catalogued AI-template look: cream backgrounds, editorial serifs, generic fade-up reveals, three-card rows, purple gradients.
