/// <reference types="node" />
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import * as S from "../src/index.js";

const generated = readFileSync(new URL("../generated/index.ts", import.meta.url), "utf8");
const typeNames = [...generated.matchAll(/^export type (\w+)/gm)].map((m) => m[1] ?? "");

describe("schema coverage (Requirement 12.3)", () => {
  it("reads the generated types", () => {
    expect(typeNames.length).toBeGreaterThan(50);
  });

  it("has a <Type>Schema export for every generated type", () => {
    const exports = S as Record<string, unknown>;
    const missing = typeNames.filter((n) => exports[`${n}Schema`] === undefined);
    expect(missing, `types without a Zod schema: ${missing.join(", ")}`).toEqual([]);
  });
});

describe("error code parity (Requirement 14.6)", () => {
  const union = /^export type ErrorCode = (.+);$/m.exec(generated)?.[1] ?? "";
  const rust = [...union.matchAll(/"([A-Z_]+)"/g)].map((m) => m[1] ?? "");
  const ts = S.ErrorCodeSchema.options.filter((c) => c !== S.CLIENT_ONLY_CODE);

  it("matches the Rust set exactly, minus the client-only code", () => {
    const onlyRust = rust.filter((c) => !ts.includes(c as (typeof ts)[number]));
    const onlyTs = ts.filter((c) => !rust.includes(c));
    expect({ onlyRust, onlyTs }).toEqual({ onlyRust: [], onlyTs: [] });
    expect(rust).toHaveLength(18);
  });

  it("has exactly one client-only code, and it is not retryable", () => {
    expect(S.ErrorCodeSchema.options.length - S.WireErrorCodeSchema.options.length).toBe(1);
    expect(S.DormbitError.schemaMismatch("x").retryable).toBe(false);
  });
});
