# Technology choices confirmed during implementation

Recorded as `foundation` design.md Open Question 1 requires.

## TypeScript version: 6.0.3, not 7.0.2

The Task 3 spike used TypeScript 7.0.2 (the native, Go-based compiler line).
At Task 9, `typescript-eslint@8.72.1` declares `peerDependencies.typescript: ">=4.8.4 <6.1.0"`
and refuses to run on 7.0.2 ("typescript-eslint does not support TS 7.0"). `tsc --noEmit` and
Vite builds work fine under 7.0.2; only the typed ESLint rules (`strictTypeChecked`) fail to load.

Decision: pin **TypeScript 6.0.3**, the newest release `typescript-eslint` 8.72.1 supports. All
three strict flags (`strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`) and the
typed lint rules (`strictTypeChecked`, `jsx-a11y`, `react-hooks`, the `dangerouslySetInnerHTML`
ban) work under 6.0.3, confirmed by `pnpm lint`, `pnpm typecheck`, `pnpm test`, and `pnpm -r build`
all passing (Task 9 checkpoint).

Revisit when `typescript-eslint` adds TypeScript 7.x support
(tracked at https://github.com/typescript-eslint/typescript-eslint/issues/10940).
