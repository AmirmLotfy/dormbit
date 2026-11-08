// The desktop journey's client. Each method invokes one allowlisted Tauri command and parses
// the result with its strict schema from @dormbit/schema: a parse failure is
// IPC_SCHEMA_MISMATCH, and a rejection from Rust keeps its DORMBIT error code. Folders are
// referred to by grant ID; no path crosses this boundary.
import { invoke } from "@tauri-apps/api/core";
import {
  ApprovedPlanSchema,
  BriefResultSchema,
  CaptureOutputSchema,
  CapturePlanSchema,
  DiscoverResultSchema,
  DormbitError,
  EvidenceResultSchema,
  RestoreOutcomeSchema,
  RestorePrecheckSchema,
  VerifyReportSchema,
  parseError,
} from "@dormbit/schema";
import { z } from "zod";

export const ReadGrantSchema = z
  .strictObject({ id: z.string(), kind: z.literal("Read"), root_display: z.string() })
  .nullable();

export const TargetGrantSchema = z
  .strictObject({
    id: z.string(),
    kind: z.literal("RestoreTarget"),
    root_display: z.string(),
    entries: z.array(z.strictObject({ name: z.string(), kind: z.string() })),
  })
  .nullable();

export const ExportResultSchema = z
  .strictObject({ file_name: z.string(), bytes: z.number().int().nonnegative() })
  .nullable();

export type ReadGrant = NonNullable<z.infer<typeof ReadGrantSchema>>;
export type TargetGrant = NonNullable<z.infer<typeof TargetGrantSchema>>;

/** One Tauri call; rejections become DormbitError, bad shapes IPC_SCHEMA_MISMATCH. */
export type Invoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

export class NativeApi {
  constructor(private readonly host: Invoke = invoke) {}

  private async call<S extends z.ZodType>(
    schema: S,
    command: string,
    args: Record<string, unknown> = {},
  ): Promise<z.infer<S>> {
    let raw: unknown;
    try {
      raw = await this.host(command, args);
    } catch (e) {
      throw parseError(e);
    }
    const r = schema.safeParse(raw);
    if (!r.success) throw DormbitError.schemaMismatch(command);
    return r.data;
  }

  scopePick() {
    return this.call(ReadGrantSchema, "scope_pick", { kind: "Read" });
  }
  discover(scopeId: string) {
    return this.call(DiscoverResultSchema, "discovery_start", { scopeId });
  }
  evidence(projectId: string) {
    return this.call(EvidenceResultSchema, "evidence_get", { projectId });
  }
  planCreate(projectId: string) {
    return this.call(CapturePlanSchema, "plan_create", { projectId });
  }
  planSetDecision(planId: string, itemPath: string, decision: "include" | "exclude") {
    return this.call(CapturePlanSchema, "plan_set_decision", { planId, itemPath, decision });
  }
  planApprove(planId: string, digest: string, acks: string[]) {
    return this.call(ApprovedPlanSchema, "plan_approve", { planId, digest, acks });
  }
  capture(planId: string) {
    return this.call(CaptureOutputSchema, "capture_start", { planId });
  }
  verify(capsuleId: string) {
    return this.call(VerifyReportSchema, "verify_start", { capsuleId });
  }
  restoreTargetPick() {
    return this.call(TargetGrantSchema, "restore_target_pick");
  }
  restorePrecheck(destName: string) {
    return this.call(RestorePrecheckSchema, "restore_precheck", { destName });
  }
  restore(destName: string) {
    return this.call(RestoreOutcomeSchema, "restore_start", { destName });
  }
  brief() {
    return this.call(BriefResultSchema, "brief_get");
  }
  briefExport() {
    return this.call(ExportResultSchema, "brief_export");
  }
}
