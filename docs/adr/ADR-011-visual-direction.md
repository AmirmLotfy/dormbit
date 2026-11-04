# ADR-011: Visual direction "Flight Recorder"

- **Status:** Accepted, Nov 4, 2026 (the user reviewed the prototype and asked for an interactive 3D hero and a cleaner gradient background)
- **Supersedes:** the color tokens, type, and status-marker colors in master plan §9
- **Related:** native-experience Requirements 16, 17, 19; judge-demo Requirements 20, 22; Task 34

## Context

The palette in master plan §9 was a cream background (`#F4F1E9`), forest green, copper, and sage, with a serif for headings. Current catalogs of AI-generated design tells list exactly this combination as the 2026 default: cream, an editorial serif, and a sage or forest-green accent. They also list generic fade-up-on-scroll reveals. The user judged the result to look like an AI template.

## Decision

Derive the look from the product instead of from trends. Flight recorders are painted international orange so investigators can find them and read what happened. A DORMBIT capsule plays the same role for a project that stopped. Verification is the readout, the brief is the report, and the security guarantees are an "operating limits" placard.

**Color tokens.** Ratios use the WCAG 2 formula and are rechecked by the Task 34 contrast test.

| Token                | Hex                   | Allowed use                                         | Ratio                                                         |
| -------------------- | --------------------- | --------------------------------------------------- | ------------------------------------------------------------- |
| Plate                | `#F6F7F5`             | light surface                                       | —                                                             |
| Panel                | `#ECEEEA`             | secondary light surface                             | —                                                             |
| Rule                 | `#D3D7D3`             | dividers only                                       | —                                                             |
| Edge                 | `#737B76`             | control borders, icons                              | 4.05 on Plate, 3.73 on Panel                                  |
| Ink                  | `#1A1E1C`             | body text                                           | 15.68 on Plate                                                |
| Steel                | `#59625D`             | secondary text                                      | 5.87 on Plate, 5.40 on Panel                                  |
| Graphite             | `#0F1211`             | dark surface                                        | —                                                             |
| Graphite-2           | `#181C1A`             | secondary dark surface                              | —                                                             |
| On-dark              | `#E8EBE7`             | text on Graphite                                    | 15.67                                                         |
| On-dark-muted        | `#9AA29D`             | secondary text on Graphite                          | 7.20                                                          |
| Signal               | `#FF4F00`             | fills, focus rings, Large text on Plate or Graphite | 3.07 on Plate, 5.71 on Graphite; Graphite text on Signal 5.71 |
| `--signal-text`      | `#B83A00`             | orange text on Plate or Panel                       | 5.36 on Plate, 4.94 on Panel                                  |
| `--signal-text-dark` | `#FF5F1A`             | orange text on Graphite                             | 6.19                                                          |
| Ok / Ok-dark         | `#1B6E45` / `#3DBE7E` | Verified                                            | 5.81 on Plate / 7.95 on Graphite; white on Ok 6.25            |
| Warn                 | `#F2B200`             | Needs decision fill (Ink text on it)                | Ink on Warn 8.94; 9.99 on Graphite                            |
| `--warn-text`        | `#8A5A00`             | Needs decision label on Plate                       | 5.52                                                          |
| Fail / Fail-dark     | `#C21F1A` / `#FF6B5E` | Failed                                              | 5.58 on Plate / 6.74 on Graphite                              |

Signal is never used on Panel for text or controls, where it measures 2.82. Status colors follow cockpit annunciators: green is normal, amber is caution, red is warning. Orange is the brand and hardware color and never a status. Every status still pairs a glyph with a word (UX-1).

**Type (all SIL Open Font License 1.1, self-hosted, no web-font requests).** Archivo variable (width 62–125%) for headings, Atkinson Hyperlegible Next for interface and brief text, and Atkinson Hyperlegible Mono for paths and hashes. The mono face was drawn so 0/O and 1/l/I can't be confused, which matters when people compare SHA-256 values.

**Landing page and web motion.** `/dormbit/` serves a landing page and `/dormbit/demo/` serves the judge demo. The hero capsule is a three.js model built from primitives (no model files), loaded after the text renders. When WebGL is missing or the context is lost, the SVG drawing stays. The background is a pointer-reactive gradient made of three light pools moved with transforms only. Sections reveal with CSS scroll-driven animations inside `@supports (animation-timeline: view())`. Content is visible by default, scrolling stays native, and nothing animates under `prefers-reduced-motion: reduce`. Dragging still turns the capsule, without easing.

**Native app.** It uses the same tokens and fonts, keeps the 120–200 ms ease-out motion rules, and has no WebGL.

## Alternatives considered

- **Keep §9:** rejected for the reason above.
- **Cryo archive (teal and frost):** too close to WGSN's 2026 color of the year and to science-fiction clichés.
- **Museum conservation (archival grey with red tags):** drifts back toward the editorial template.
- **Retro terminal (phosphor green):** an overused developer-tool trope.
- **GSAP for scroll motion:** its license bars use in tools that compete with Webflow, and Webflow can revoke it. CSS scroll timelines now run in Chromium 115+, Safari 26, and Firefox 159.
- **Full-page WebGL background:** costs battery on phones and adds nothing the CSS gradient doesn't already do.

## Consequences

- Task 34's tokens, contrast test, and font bundle change to the values above. Fonts already live in `packages/ui/src/fonts/`.
- three.js 0.187.0 (MIT) is a dependency of `apps/demo` only. Its chunk is about 138 kB gzipped and loads after first paint.
- The app icon must move off the placeholder indigo crescent. That happens in the native-experience icon task.
- Judges using browsers without WebGL see the SVG capsule. Browsers without scroll timelines see every section without animation.
