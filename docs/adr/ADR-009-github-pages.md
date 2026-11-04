# ADR-009: Host the judge demo on GitHub Pages

- **Status:** Accepted, Nov 4, 2026
- **Task:** P01-T02
- **Related:** DEM-1, DEM-2; plan §16–17; Tasks 41 and 51; [research.md §4–5](../product/research.md#4-kiroween-2026-rules-checklist)

## Context
The hackathon requires a working app URL that stays available and free, without restriction, from submission through the end of judging (Jan 29, 2027). The judge demo is a static site: HTML, JS, CSS, a WebAssembly module (about 330 KB gzipped in the spike), and the Dockline fixture. It needs no server, accounts, or analytics.

The repo is already public on GitHub, and CI runs on GitHub Actions. The Netlify CLI is installed on this machine, and the Kiro Powers review (research.md §5) considered a Netlify deploy Power and rejected it because hosting is GitHub Pages.

## Decision
- Deploy the demo to GitHub Pages at `https://<owner>.github.io/dormbit/` from a `pages.yml` workflow.
- Build with Vite `base: '/dormbit/'`.
- Set the CSP in a meta tag, including `'wasm-unsafe-eval'` so the WebAssembly core can load. Verify WebKit with it (Task 38).
- No analytics, no cookies, no third-party requests. Fonts are bundled.
- A preview goes live in Task 41. Production deploys in Task 51, with a daily scheduled smoke test through Jan 29, 2027 that opens an issue on failure.

## Alternatives considered
- **Netlify.** Rejected. A second account and deploy pipeline for no gain over Pages; the CLI being installed isn't a reason on its own.
- **Vercel or Cloudflare Pages.** Rejected for the same reason: another account, more moving parts.
- **Self-hosting.** Rejected. Uptime through Jan 29, 2027 would depend on a machine we maintain.

## Consequences
- Pages doesn't allow custom response headers, so the CSP can only be a meta tag. A meta-tag CSP can't set `frame-ancestors`, so the page can be framed by other sites. The demo has no privileged actions and touches no local files, so this is acceptable. It is noted in the security review.
- The demo depends on GitHub Pages availability. The daily smoke test detects outages; it can't prevent them.
- Deploys are tied to the public repo, which is already required by the rules.
- The demo URL goes into the README, the Devpost submission, and the repo's website field.
