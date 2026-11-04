import { describe, expect, expectTypeOf, it } from "vitest";
import type {
  AppInfo as GenAppInfo,
  AppInfoEvent as GenAppInfoEvent,
  DormbitError as GenDormbitError,
  ErrorCode as GenErrorCode,
} from "../generated/index.js";
import {
  AppInfoEventSchema,
  AppInfoSchema,
  DormbitError,
  DormbitErrorSchema,
  ERROR_CODES,
  ErrorCodeSchema,
  type AppInfo,
  type AppInfoEvent,
  type DormbitErrorShape,
  type ErrorCode,
} from "../src/index.js";

describe("Zod types equal the Rust-generated types", () => {
  it("match at compile time", () => {
    expectTypeOf<ErrorCode>().toEqualTypeOf<GenErrorCode>();
    expectTypeOf<AppInfo>().toEqualTypeOf<GenAppInfo>();
    expectTypeOf<DormbitErrorShape>().toMatchTypeOf<GenDormbitError>();
    expectTypeOf<AppInfoEvent>().toMatchTypeOf<GenAppInfoEvent>();
  });
});

describe("schemas", () => {
  const info = {
    name: "DORMBIT",
    version: "0.1.0",
    core_version: "0.1.0",
    profile: "debug",
    min_macos: "13.0",
  };

  it("lists 13 distinct codes", () => {
    expect(new Set(ERROR_CODES).size).toBe(13);
    expect(ErrorCodeSchema.safeParse("NOPE").success).toBe(false);
  });

  it("rejects unknown fields", () => {
    expect(AppInfoSchema.safeParse({ ...info, extra: 1 }).success).toBe(false);
    expect(
      DormbitErrorSchema.safeParse({ code: "CANCELLED", message: "x", retryable: false, extra: 1 })
        .success,
    ).toBe(false);
  });

  it("accepts info and done events and rejects other kinds", () => {
    expect(AppInfoEventSchema.safeParse({ kind: "info", info }).success).toBe(true);
    expect(AppInfoEventSchema.safeParse({ kind: "done" }).success).toBe(true);
    expect(AppInfoEventSchema.safeParse({ kind: "other" }).success).toBe(false);
    expect(AppInfoEventSchema.safeParse({ kind: "done", info }).success).toBe(false);
  });

  it("builds a schema-mismatch error that is not retryable", () => {
    const e = DormbitError.schemaMismatch("message");
    expect(e.code).toBe("IPC_SCHEMA_MISMATCH");
    expect(e.retryable).toBe(false);
    expect(e).toBeInstanceOf(Error);
  });
});
