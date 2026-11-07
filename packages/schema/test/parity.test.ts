/// <reference types="node" />
// PROP-11 schema round trip, TypeScript side (foundation Requirement 15). Runs only inside
// scripts/schema-parity.sh, which sets DORMBIT_PARITY=1 after the Rust side writes its samples.
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import fc from "fast-check";
import { describe, expect, it } from "vitest";
import type { z } from "zod";
import * as S from "../src/index.js";
import { arbitraryFor } from "./arb.js";

const dir =
  process.env.SCHEMA_PARITY_DIR ??
  fileURLToPath(new URL("../../../target/schema-parity", import.meta.url));
const runs = Number(process.env.FC_NUM_RUNS ?? "256");
const generated = readFileSync(new URL("../generated/index.ts", import.meta.url), "utf8");
const names = [...generated.matchAll(/^export type (\w+)/gm)].map((m) => m[1] ?? "");

function schemaFor(name: string): z.ZodType {
  if (name === "JobEvent") return S.JobEventSchema(S.DiscoveryResultSchema);
  // The wire type has 18 codes; ErrorCodeSchema adds the client-only IPC_SCHEMA_MISMATCH,
  // which Rust must (and does) reject. Parity is about what crosses IPC.
  if (name === "ErrorCode") return S.WireErrorCodeSchema;
  const schema = (S as Record<string, unknown>)[`${name}Schema`];
  if (schema === undefined) throw new Error(`no schema for ${name}`);
  return schema as z.ZodType;
}

function seedOf(name: string): number {
  let h = 7;
  for (let i = 0; i < name.length; i++) h = (Math.imul(h, 31) + name.charCodeAt(i)) | 0;
  return h;
}

describe.skipIf(process.env.DORMBIT_PARITY !== "1")(
  "PROP-11 schema round trip (TypeScript side)",
  () => {
    it("has Rust samples for every generated type, and no extras", () => {
      const files = readdirSync(join(dir, "rust"))
        .map((f) => f.replace(/\.jsonl$/, ""))
        .sort();
      expect(files).toEqual([...names].sort());
    });

    it("parses every Rust sample and re-serializes it unchanged", () => {
      const failures: string[] = [];
      let total = 0;
      for (const name of names) {
        const schema = schemaFor(name);
        const lines = readFileSync(join(dir, "rust", `${name}.jsonl`), "utf8")
          .split("\n")
          .filter(Boolean);
        for (const line of lines) {
          total += 1;
          const original: unknown = JSON.parse(line);
          const parsed = schema.safeParse(original);
          if (!parsed.success) {
            failures.push(
              `${name}: Zod rejected a Rust sample\n  sample: ${line}\n  ${parsed.error.message}`,
            );
            continue;
          }
          const again: unknown = JSON.parse(JSON.stringify(parsed.data));
          if (!isDeepStrictEqual(again, original))
            failures.push(`${name}: round trip changed the value\n  sample: ${line}`);
        }
      }
      expect(total).toBeGreaterThanOrEqual(names.length);
      expect(failures.slice(0, 10).join("\n\n")).toBe("");
    });

    it("writes fast-check samples of every generated type, each valid under its schema", () => {
      mkdirSync(join(dir, "ts"), { recursive: true });
      for (const name of names) {
        const schema = schemaFor(name);
        const values = fc.sample(arbitraryFor(schema), { numRuns: runs, seed: seedOf(name) });
        for (const v of values) {
          expect(schema.safeParse(v).success, `${name}: ${JSON.stringify(v)}`).toBe(true);
        }
        writeFileSync(
          join(dir, "ts", `${name}.jsonl`),
          `${values.map((v) => JSON.stringify(v)).join("\n")}\n`,
        );
      }
    });
  },
);
