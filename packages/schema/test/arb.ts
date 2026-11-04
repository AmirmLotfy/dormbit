// fast-check arbitraries derived from the Zod schemas' shapes (PROP-11, TypeScript side).
// Covers exactly the schema kinds the IPC types use; anything else throws so a new kind can't
// slip through untested.
import fc from "fast-check";
import { z } from "zod";

const TRICKY = [
  "",
  "~",
  "~/dev/dockline",
  '"quoted"',
  "back\\slash",
  "caf\u00e9",
  "cafe\u0301",
  "\u00c5",
  "A\u030a",
  "\u65e5\u672c\u8a9e",
  "\u{1f642}",
  "a\u200db",
  "tab\tnew\nline",
  "</script>",
  "nul\u0000byte",
];

const text = fc.oneof(
  { weight: 1, arbitrary: fc.constantFrom(...TRICKY) },
  { weight: 2, arbitrary: fc.string({ unit: "grapheme", maxLength: 12 }) },
);

/** 0 to 2^53 - 1, weighted toward the edges (SafeU64 is the only number type over IPC). */
const safeU64 = fc.oneof(
  fc.constantFrom(0, Number.MAX_SAFE_INTEGER),
  fc.nat(1000),
  fc.maxSafeNat(),
);

export function arbitraryFor(schema: z.core.$ZodType): fc.Arbitrary<unknown> {
  if (schema instanceof z.ZodString) return text;
  if (schema instanceof z.ZodNumber) return safeU64;
  if (schema instanceof z.ZodBoolean) return fc.boolean();
  if (schema instanceof z.ZodEnum) return fc.constantFrom(...schema.options);
  if (schema instanceof z.ZodLiteral) return fc.constantFrom(...schema.values);
  if (schema instanceof z.ZodNullable)
    return fc.option(arbitraryFor(schema.unwrap()), { freq: 4, nil: null });
  if (schema instanceof z.ZodArray) return fc.array(arbitraryFor(schema.element), { maxLength: 3 });
  if (schema instanceof z.ZodObject) {
    const shape = schema.shape as Record<string, z.core.$ZodType>;
    return fc.record(
      Object.fromEntries(Object.entries(shape).map(([k, v]) => [k, arbitraryFor(v)])),
    );
  }
  if (schema instanceof z.ZodUnion) return fc.oneof(...schema.options.map((o) => arbitraryFor(o)));
  throw new Error(`no arbitrary for schema type "${schema._zod.def.type}"`);
}
