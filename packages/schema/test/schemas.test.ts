import { describe, expect, it } from "vitest";
import {
  AppInfoEventSchema,
  AppInfoSchema,
  DiscoveryResultSchema,
  DormbitError,
  DormbitErrorSchema,
  ErrorCodeSchema,
  JobEventSchema,
  ProjectRecordSchema,
  SafeU64Schema,
  parseError,
} from "../src/index.js";

const info = {
  name: "DORMBIT",
  version: "0.1.0",
  core_version: "0.1.0",
  profile: "debug",
  min_macos: "13.0",
};

describe("schemas", () => {
  it("lists 18 wire codes plus the client-only code", () => {
    expect(ErrorCodeSchema.options).toHaveLength(19);
    expect(ErrorCodeSchema.safeParse("IPC_SCHEMA_MISMATCH").success).toBe(true);
    expect(ErrorCodeSchema.safeParse("NOPE").success).toBe(false);
  });

  it("rejects unknown fields (Requirement 12.4)", () => {
    expect(AppInfoSchema.safeParse({ ...info, extra: 1 }).success).toBe(false);
    expect(
      DormbitErrorSchema.safeParse({ code: "CANCELLED", message: "x", retryable: false, extra: 1 })
        .success,
    ).toBe(false);
  });

  it("rejects a wire error carrying the client-only code", () => {
    const e = { code: "IPC_SCHEMA_MISMATCH", message: "x", retryable: false };
    expect(DormbitErrorSchema.safeParse(e).success).toBe(false);
  });

  it("accepts info and done events and rejects other kinds", () => {
    expect(AppInfoEventSchema.safeParse({ kind: "info", info }).success).toBe(true);
    expect(AppInfoEventSchema.safeParse({ kind: "done" }).success).toBe(true);
    expect(AppInfoEventSchema.safeParse({ kind: "other" }).success).toBe(false);
    expect(AppInfoEventSchema.safeParse({ kind: "done", info }).success).toBe(false);
  });

  it("keeps SafeU64 within 0 to 2^53 - 1", () => {
    expect(SafeU64Schema.safeParse(Number.MAX_SAFE_INTEGER).success).toBe(true);
    expect(SafeU64Schema.safeParse(Number.MAX_SAFE_INTEGER + 1).success).toBe(false);
    expect(SafeU64Schema.safeParse(-1).success).toBe(false);
    expect(SafeU64Schema.safeParse(1.5).success).toBe(false);
  });

  it("parses job events against the result schema", () => {
    const S = JobEventSchema(DiscoveryResultSchema);
    const done = { kind: "done", result: { projects_found: 3, entries_scanned: 120 } };
    expect(S.safeParse(done).success).toBe(true);
    expect(S.safeParse({ kind: "done", result: { projects_found: 3 } }).success).toBe(false);
    expect(S.safeParse({ kind: "cancelled" }).success).toBe(true);
  });

  it("requires null, not a missing key, for an empty optional field (Requirement 11.4)", () => {
    const project = {
      id: "p",
      scope_id: "s",
      rel_root: "dockline",
      name: { value: "dockline", source: "folder" },
      stacks: [],
      git: null,
      activity: { latest_mtime: null, scanned: 1, partial: false },
      size: { total: 1, rebuildable: 0, partial: false },
      docs: [],
      manifests: [],
      agent_artifacts: [],
      marks: { dormant: false },
      preservation: { last_capsule: null, last_verified_at: null, last_verified_restore: null },
    };
    expect(ProjectRecordSchema.safeParse(project).success).toBe(true);
    const withoutGit: Record<string, unknown> = { ...project };
    delete withoutGit.git;
    expect(ProjectRecordSchema.safeParse(withoutGit).success).toBe(false);
  });
});

describe("parseError (Requirement 14.5)", () => {
  it("keeps a valid wire error", () => {
    const e = parseError({ code: "RESTORE_DEST_EXISTS", message: "exists", retryable: false });
    expect(e).toBeInstanceOf(DormbitError);
    expect(e.code).toBe("RESTORE_DEST_EXISTS");
  });

  it("maps an unknown code or a non-error to IPC_SCHEMA_MISMATCH", () => {
    expect(parseError({ code: "NEW_CODE", message: "x", retryable: true }).code).toBe(
      "IPC_SCHEMA_MISMATCH",
    );
    expect(parseError("boom").code).toBe("IPC_SCHEMA_MISMATCH");
    expect(parseError(new Error("boom")).retryable).toBe(false);
  });
});
