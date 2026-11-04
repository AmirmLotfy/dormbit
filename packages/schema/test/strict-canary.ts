// Strict canary: this file must fail to compile if any strict flag is turned off.
// Each @ts-expect-error line becomes an error ("unused directive") when its flag is disabled.

// strict (strictNullChecks): null is not assignable to string
// @ts-expect-error canary: strictNullChecks must reject null here
const a: string = null;

// noUncheckedIndexedAccess: xs[0] is number | undefined
const xs: number[] = [];
// @ts-expect-error canary: noUncheckedIndexedAccess must reject this index read
const b: number = xs[0];

// exactOptionalPropertyTypes: undefined is not assignable to an optional property
// @ts-expect-error canary: exactOptionalPropertyTypes must reject explicit undefined
const c: { p?: string } = { p: undefined };

export { a, b, c };
