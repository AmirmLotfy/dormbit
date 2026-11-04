---
inclusion: fileMatch
fileMatchPattern: ["apps/**", "packages/**"]
---

# Frontend conventions

- React 19 function components, TypeScript strict (`noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`). ESLint `strictTypeChecked` with no inline disables.
- **Zod at the boundary.** Every value from Rust or the WebAssembly core is parsed with its schema from `@dormbit/schema`; a parse failure becomes `DormbitError` code `IPC_SCHEMA_MISMATCH` (never retryable). Use `parseError` for rejections.
- Screens talk only to a `DormbitClient` (`@dormbit/client`). Build and test against `MockClient`; run `runClientContract` for any new client.
- No raw HTML, no `dangerouslySetInnerHTML`, Markdown without raw HTML. No outside requests: fonts and assets are bundled; the CSP is `script-src 'self'`.
- Never edit `packages/schema/generated/`; regenerate it with `sh scripts/gen-schema.sh`.
- Every status shows a glyph and a word, never color alone. Respect `prefers-reduced-motion`: no animation at all when it is set.
- Desktop dev without Rust: `VITE_DORMBIT_CLIENT=mock pnpm --filter @dormbit/desktop dev`.
