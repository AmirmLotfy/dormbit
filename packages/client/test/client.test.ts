/// <reference types="node" />
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  CaptureResultSchema,
  DiscoveryResultSchema,
  DormbitError,
  VerificationReportSchema,
} from "@dormbit/schema";
import { collectJob } from "../src/contract.js";
import { CLIENT_METHODS, MOCK_IDS, MockClient } from "../src/index.js";

const read = (rel: string): string => readFileSync(new URL(rel, import.meta.url), "utf8");
const camel = (s: string): string => s.replace(/_([a-z])/g, (_, c: string) => c.toUpperCase());

describe("DormbitClient shape (Requirement 13.1, 13.2)", () => {
  it("has one method per plan command, in plan order", () => {
    const rust = read("../../../apps/desktop/src-tauri/tests/allowlist.rs");
    const block = /const PLAN_COMMANDS[^=]*=\s*\[([^\]]+)\]/.exec(rust)?.[1] ?? "";
    const commands = [...block.matchAll(/"([a-z_]+)"/g)].map((m) => m[1] ?? "");
    expect(commands).toHaveLength(28);
    expect([...CLIENT_METHODS]).toEqual(commands.map(camel));
  });

  it("has no parameter that names a filesystem path, except the plan-relative itemPath", () => {
    const src = read("../src/DormbitClient.ts");
    const params = [...src.matchAll(/^\s+\w+\(([^)]*)\)/gm)].flatMap((m) =>
      (m[1] ?? "").split(",").map((p) => p.split(":")[0]?.trim() ?? ""),
    );
    const suspicious = params.filter((p) => /path|dir|folder|file/i.test(p) && p !== "itemPath");
    expect(params.length).toBeGreaterThan(20);
    expect(suspicious).toEqual([]);
  });
});

describe("MockClient (Requirement 13.3 to 13.7)", () => {
  it("streams state, real progress counts, then done", async () => {
    const client = new MockClient();
    const events = await collectJob(CaptureResultSchema, async (l) => {
      const plan = await client.planCreate(MOCK_IDS.dockline);
      await client.planApprove(plan.plan_id, { digest: plan.digest, acks: [], overrides: [] });
      return client.captureStart(plan.plan_id, l);
    });
    const kinds = events.map((e) => e.kind);
    expect(kinds[0]).toBe("state");
    expect(kinds.at(-1)).toBe("done");
    const progress = events.filter((e) => e.kind === "progress");
    const last = progress.at(-1);
    const plan = await client.planCreate(MOCK_IDS.dockline);
    expect(last).toMatchObject({
      files_done: plan.totals.included_files,
      files_total: plan.totals.included_files,
      bytes_done: plan.totals.included_bytes,
    });
  });

  it("streams one cancelled event and no done when a running job is cancelled", async () => {
    const client = new MockClient();
    const events = await collectJob(
      VerificationReportSchema,
      (l) => client.verifyStart(MOCK_IDS.capsule, l),
      (jobId) => client.jobCancel(jobId),
    );
    expect(events.filter((e) => e.kind === "cancelled")).toHaveLength(1);
    expect(events.some((e) => e.kind === "done")).toBe(false);
  });

  it("discoveryCancel cancels only discovery jobs", async () => {
    const client = new MockClient();
    const events = await collectJob(
      DiscoveryResultSchema,
      (l) => client.discoveryStart(MOCK_IDS.scope, l),
      (jobId) => client.discoveryCancel(jobId),
    );
    expect(events.at(-1)?.kind).toBe("cancelled");
  });

  it("returns an override value or rejects with an override error", async () => {
    const err = new DormbitError("RESTORE_INSUFFICIENT_SPACE", "No room.", true);
    const client = new MockClient({ overrides: { projectsList: [], projectGet: err } });
    expect(await client.projectsList()).toEqual([]);
    await expect(client.projectGet(MOCK_IDS.dockline)).rejects.toBe(err);
  });

  it("fails a job with the configured error instead of done", async () => {
    const client = new MockClient({
      failJobs: {
        verify: { code: "CAPSULE_HASH_MISMATCH", message: "src/tax.rs changed.", retryable: false },
      },
    });
    const events = await collectJob(VerificationReportSchema, (l) =>
      client.verifyStart(MOCK_IDS.capsule, l),
    );
    expect(events.at(-1)).toMatchObject({
      kind: "failed",
      error: { code: "CAPSULE_HASH_MISMATCH" },
    });
  });

  it("returns copies, so callers can't change its data", async () => {
    const client = new MockClient();
    const first = await client.projectGet(MOCK_IDS.dockline);
    first.marks.dormant = false;
    first.name.value = "changed";
    const again = await client.projectGet(MOCK_IDS.dockline);
    expect(again.marks.dormant).toBe(true);
    expect(again.name.value).toBe("dockline-invoices");
  });

  it("refuses capture until the current plan is approved", async () => {
    const client = new MockClient();
    const plan = await client.planCreate(MOCK_IDS.dockline);
    await expect(client.captureStart(plan.plan_id, () => undefined)).rejects.toMatchObject({
      code: "PLAN_DIGEST_MISMATCH",
    });
  });

  it("refuses a sensitive item and an unsafe restore name", async () => {
    const client = new MockClient();
    const plan = await client.planCreate(MOCK_IDS.dockline);
    await expect(client.planSetDecision(plan.plan_id, ".env", "include")).rejects.toMatchObject({
      code: "PATH_UNSAFE",
    });
    const target = await client.restoreTargetPick();
    if (!target) throw new Error("no target");
    const input = {
      capsule_id: MOCK_IDS.capsule,
      target_scope_id: target.id,
      dest_name: "../escape",
    };
    expect((await client.restorePrecheck(input)).ok).toBe(false);
    await expect(client.restoreStart(input, () => undefined)).rejects.toMatchObject({
      code: "PATH_UNSAFE",
    });
  });
});
