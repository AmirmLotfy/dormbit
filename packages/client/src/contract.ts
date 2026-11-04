// Client_Contract: the suite every DormbitClient implementation runs (MockClient now; the
// Tauri and WebAssembly clients later). Each method must resolve to schema-valid data or reject
// with a DormbitError, and every job must end with exactly one done, failed, or cancelled.
import { describe, expect, it } from "vitest";
import type { z } from "zod";
import {
  AdapterInfoSchema,
  AppInfoEventSchema,
  AppInfoSchema,
  ApprovedPlanSchema,
  BriefSchema,
  CapsuleRefSchema,
  CapturePlanSchema,
  CaptureResultSchema,
  ConsentRecordSchema,
  DiscoveryResultSchema,
  DormbitError,
  ExportResultSchema,
  FindingSchema,
  JobEventSchema,
  PrecheckReportSchema,
  ProjectRecordSchema,
  RecoverResultSchema,
  RestoreReportSchema,
  ScopeGrantSchema,
  SettingsSchema,
  VerificationReportSchema,
  type JobEvent,
} from "@dormbit/schema";
import type { DormbitClient, JobListener } from "./DormbitClient.js";

const TERMINAL = new Set(["done", "failed", "cancelled"]);

/** Starts a job and collects its events until the terminal one (or a 10 s timeout). */
export async function collectJob<R>(
  resultSchema: z.ZodType<R>,
  start: (listener: JobListener<R>) => Promise<string>,
  afterStart?: (jobId: string) => Promise<void>,
): Promise<JobEvent<R>[]> {
  const schema = JobEventSchema(resultSchema);
  const events: JobEvent<R>[] = [];
  let finish: () => void = () => undefined;
  const ended = new Promise<void>((resolve) => {
    finish = resolve;
  });
  const jobId = await start((e) => {
    events.push(schema.parse(e));
    if (TERMINAL.has(e.kind)) finish();
  });
  if (afterStart) await afterStart(jobId);
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<void>((_, reject) => {
    timer = setTimeout(() => {
      reject(new Error("job did not finish within 10 s"));
    }, 10_000);
  });
  await Promise.race([ended, timeout]).finally(() => {
    clearTimeout(timer);
  });
  return events;
}

function expectOneTerminal<R>(events: JobEvent<R>[]): JobEvent<R> {
  expect(events.filter((e) => TERMINAL.has(e.kind))).toHaveLength(1);
  const last = events.at(-1);
  if (!last || !TERMINAL.has(last.kind)) throw new Error("the job's last event is not terminal");
  return last;
}

async function rejection(p: Promise<unknown>): Promise<DormbitError> {
  try {
    await p;
  } catch (e) {
    expect(e).toBeInstanceOf(DormbitError);
    return e as DormbitError;
  }
  throw new Error("expected the call to reject with a DormbitError");
}

export function runClientContract(
  name: string,
  makeClient: () => DormbitClient | Promise<DormbitClient>,
): void {
  describe(`Client_Contract: ${name}`, () => {
    it("appInfo streams info then done and returns AppInfo", async () => {
      const client = await makeClient();
      const kinds: string[] = [];
      const info = AppInfoSchema.parse(
        await client.appInfo((e) => kinds.push(AppInfoEventSchema.parse(e).kind)),
      );
      expect(info.name).toBe("DORMBIT");
      expect(kinds).toEqual(["info", "done"]);
    });

    it("lists scopes, projects, capsules, and adapters in valid shapes", async () => {
      const client = await makeClient();
      ScopeGrantSchema.array().parse(await client.scopeList());
      expect(ProjectRecordSchema.array().parse(await client.projectsList()).length).toBeGreaterThan(
        0,
      );
      CapsuleRefSchema.array().parse(await client.capsulesList());
      AdapterInfoSchema.array().parse(await client.adaptersList());
    });

    it("returns a project with its evidence and brief", async () => {
      const client = await makeClient();
      const [first] = await client.projectsList();
      if (!first) throw new Error("no projects");
      expect(ProjectRecordSchema.parse(await client.projectGet(first.id)).id).toBe(first.id);
      FindingSchema.array().parse(await client.evidenceGet(first.id));
      BriefSchema.parse(await client.briefGet(first.id));
      ExportResultSchema.parse(await client.briefExport(first.id));
      expect(
        ProjectRecordSchema.parse(await client.projectSetDormant(first.id, true)).marks.dormant,
      ).toBe(true);
    });

    it("rejects an unknown project with a DormbitError", async () => {
      const client = await makeClient();
      const e = await rejection(client.projectGet("00000000-0000-4000-8000-000000000000"));
      expect(e.code).toBe("SCOPE_DENIED");
      expect(e.retryable).toBe(false);
    });

    it("creates, edits, and approves a plan; a stale digest is refused", async () => {
      const client = await makeClient();
      const [first] = await client.projectsList();
      if (!first) throw new Error("no projects");
      const plan = CapturePlanSchema.parse(await client.planCreate(first.id));
      const editable = plan.items.find((i) => i.editable);
      let digest = plan.digest;
      if (editable) {
        const flipped = editable.decision === "include" ? "exclude" : "include";
        const edited = CapturePlanSchema.parse(
          await client.planSetDecision(plan.plan_id, editable.path, flipped),
        );
        expect(edited.digest).not.toBe(plan.digest);
        const stale = await rejection(
          client.planApprove(plan.plan_id, { digest, acks: [], overrides: [] }),
        );
        expect(stale.code).toBe("PLAN_DIGEST_MISMATCH");
        digest = edited.digest;
      }
      ApprovedPlanSchema.parse(
        await client.planApprove(plan.plan_id, { digest, acks: [], overrides: [] }),
      );
    });

    it("runs discovery, capture, verify, and restore to exactly one terminal event", async () => {
      const client = await makeClient();
      const [scope] = (await client.scopeList()).filter((s) => s.kind === "Read");
      if (!scope) throw new Error("no Read scope");
      expectOneTerminal(
        await collectJob(DiscoveryResultSchema, (l) => client.discoveryStart(scope.id, l)),
      );

      const [project] = await client.projectsList();
      if (!project) throw new Error("no projects");
      const plan = await client.planCreate(project.id);
      await client.planApprove(plan.plan_id, { digest: plan.digest, acks: [], overrides: [] });
      const capture = expectOneTerminal(
        await collectJob(CaptureResultSchema, (l) => client.captureStart(plan.plan_id, l)),
      );
      expect(capture.kind).not.toBe("cancelled");

      const [capsule] = await client.capsulesList();
      if (!capsule) throw new Error("no capsules");
      expectOneTerminal(
        await collectJob(VerificationReportSchema, (l) => client.verifyStart(capsule.id, l)),
      );

      const target = ScopeGrantSchema.nullable().parse(await client.restoreTargetPick());
      if (target) {
        const input = {
          capsule_id: capsule.id,
          target_scope_id: target.id,
          dest_name: "contract-restore",
        };
        PrecheckReportSchema.parse(await client.restorePrecheck(input));
        expectOneTerminal(
          await collectJob(RestoreReportSchema, (l) => client.restoreStart(input, l)),
        );
        RecoverResultSchema.parse(
          await client.restoreRecover({ op_id: "unknown-op", action: "discard" }),
        );
      }
    });

    it("ends a cancelled job with exactly one terminal event", async () => {
      const client = await makeClient();
      const [capsule] = await client.capsulesList();
      if (!capsule) throw new Error("no capsules");
      const events = await collectJob(
        VerificationReportSchema,
        (l) => client.verifyStart(capsule.id, l),
        (jobId) => client.jobCancel(jobId),
      );
      expectOneTerminal(events);
    });

    it("pickers resolve to a grant, a capsule, or null", async () => {
      const client = await makeClient();
      ScopeGrantSchema.nullable().parse(await client.scopePick("CapsuleLibrary"));
      CapsuleRefSchema.nullable().parse(await client.capsulePick());
    });

    it("applies a settings patch and leaves null fields unchanged", async () => {
      const client = await makeClient();
      const before = SettingsSchema.parse(await client.settingsGet());
      const after = SettingsSchema.parse(
        await client.settingsSet({ reduce_motion: !before.reduce_motion, capsule_max_mb: null }),
      );
      expect(after.reduce_motion).toBe(!before.reduce_motion);
      expect(after.capsule_max_mb).toBe(before.capsule_max_mb);
    });

    it("records consent for a supported adapter", async () => {
      const client = await makeClient();
      const supported = (await client.adaptersList()).find((a) => a.support === "supported");
      if (!supported) return;
      expect(ConsentRecordSchema.parse(await client.consentSet(supported.id, true)).granted).toBe(
        true,
      );
    });
  });
}
