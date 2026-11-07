// In-memory DormbitClient for UI work and tests (Requirement 13). Every value it returns or
// streams passes through the matching Zod schema first, so a screen built on the mock gets
// exactly the shapes the Rust side will send. Jobs advance one step per `tick` (a macrotask by
// default), so a test can cancel a job before it finishes.
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
  type AppInfoEvent,
  type Brief,
  type CapsuleId,
  type CapsuleRef,
  type CapturePlan,
  type CaptureResult,
  type Decision,
  type DiscoveryResult,
  type DormbitErrorShape,
  type JobEvent,
  type JobId,
  type PlanApproveInput,
  type PlanId,
  type PlanItem,
  type ProjectRecord,
  type RecoverRequest,
  type RestoreInput,
  type RestoreReport,
  type ScopeGrant,
  type ScopeId,
  type ScopeKind,
  type SettingsPatch,
  type VerificationReport,
} from "@dormbit/schema";
import type { DormbitClient, JobListener } from "../DormbitClient.js";
import { IDS, dataset, docklinePlanItems, type MockData } from "./data.js";

type Result<K extends keyof DormbitClient> = Awaited<ReturnType<DormbitClient[K]>>;

/** Replaces one method's result, or makes it reject with the given error. */
export type MockOverrides = { [K in keyof DormbitClient]?: Result<K> | DormbitError };
export type MockJobKind = "discovery" | "capture" | "verify" | "restore";

export interface MockClientOptions {
  overrides?: MockOverrides;
  /** Makes a job stream progress and then `failed` with this error instead of `done`. */
  failJobs?: Partial<Record<MockJobKind, DormbitErrorShape>>;
  /** Waits between job steps. Defaults to one macrotask. */
  tick?: () => Promise<void>;
}

interface Job {
  kind: MockJobKind;
  cancelled: boolean;
  finished: boolean;
}

interface Chunk {
  files: number;
  bytes: number;
}

const macrotask = (): Promise<void> =>
  new Promise((resolve) => {
    setTimeout(resolve, 0);
  });

const denied = (what: string): DormbitError =>
  new DormbitError("SCOPE_DENIED", `${what} is not in an approved folder.`, false);

const PICKED: Record<ScopeKind, string> = {
  Read: "~/dev/archive",
  CapsuleLibrary: "~/Capsules",
  RestoreTarget: "~/Restored",
};

export class MockClient implements DormbitClient {
  private readonly data: MockData = dataset();
  private readonly jobs = new Map<string, Job>();
  private readonly approved = new Map<string, string>();
  private readonly options: MockClientOptions;
  private readonly tick: () => Promise<void>;
  private seq = 0;

  constructor(options: MockClientOptions = {}) {
    this.options = options;
    this.tick = options.tick ?? macrotask;
  }

  // ---- commands, in plan order ----------------------------------------------------------

  appInfo(onEvent?: (e: AppInfoEvent) => void): Promise<Result<"appInfo">> {
    return this.call("appInfo", AppInfoSchema, () => {
      const info = AppInfoSchema.parse({
        name: "DORMBIT",
        version: "0.1.0-mock",
        core_version: "0.1.0",
        profile: "debug",
        min_macos: "13.0",
      });
      onEvent?.(AppInfoEventSchema.parse({ kind: "info", info }));
      onEvent?.(AppInfoEventSchema.parse({ kind: "done" }));
      return info;
    });
  }

  scopePick(kind: ScopeKind): Promise<Result<"scopePick">> {
    return this.call("scopePick", ScopeGrantSchema.nullable(), () => this.grant(kind));
  }

  scopeList(): Promise<Result<"scopeList">> {
    return this.call("scopeList", ScopeGrantSchema.array(), () => this.data.scopes);
  }

  scopeRevoke(scopeId: ScopeId): Promise<void> {
    return this.call("scopeRevoke", null, () => {
      const i = this.data.scopes.findIndex((s) => s.id === scopeId);
      if (i < 0) throw denied("That folder");
      this.data.scopes.splice(i, 1);
    });
  }

  discoveryStart(scopeId: ScopeId, onEvent: JobListener<DiscoveryResult>): Promise<JobId> {
    return this.call("discoveryStart", null, () => {
      const scope = this.data.scopes.find((s) => s.id === scopeId && s.kind === "Read");
      if (!scope) throw denied("That folder");
      const found = this.data.projects.filter((p) => p.scope_id === scopeId);
      const chunks = found.map((p) => ({ files: p.activity.scanned, bytes: p.size.total }));
      const scanned = chunks.reduce((n, c) => n + c.files, 0);
      return this.startJob(
        "discovery",
        DiscoveryResultSchema,
        onEvent,
        ["walking"],
        chunks,
        () => ({
          projects_found: found.length,
          entries_scanned: scanned,
        }),
      );
    });
  }

  discoveryCancel(jobId: JobId): Promise<void> {
    return this.call("discoveryCancel", null, () => {
      const job = this.jobs.get(jobId);
      if (job?.kind === "discovery" && !job.finished) job.cancelled = true;
    });
  }

  projectsList(): Promise<Result<"projectsList">> {
    return this.call("projectsList", ProjectRecordSchema.array(), () => this.data.projects);
  }

  projectGet(projectId: string): Promise<Result<"projectGet">> {
    return this.call("projectGet", ProjectRecordSchema, () => this.project(projectId));
  }

  projectSetDormant(projectId: string, dormant: boolean): Promise<Result<"projectSetDormant">> {
    return this.call("projectSetDormant", ProjectRecordSchema, () => {
      const p = this.project(projectId);
      p.marks.dormant = dormant;
      return p;
    });
  }

  evidenceGet(projectId: string): Promise<Result<"evidenceGet">> {
    return this.call("evidenceGet", FindingSchema.array(), () => {
      const p = this.project(projectId);
      return this.data.findings.get(p.id) ?? p.stacks;
    });
  }

  planCreate(projectId: string): Promise<Result<"planCreate">> {
    return this.call("planCreate", CapturePlanSchema, () => {
      const p = this.project(projectId);
      const items: PlanItem[] =
        p.id === IDS.dockline
          ? docklinePlanItems()
          : p.manifests.concat(p.docs).map((path) => ({
              path,
              kind: "file",
              size: 2048,
              decision: "include",
              reason: "manifest or documentation",
              editable: true,
              warning: null,
            }));
      const plan = withTotals({
        plan_id: p.id === IDS.dockline ? IDS.plan : this.newId(),
        items,
        groups: [],
        git_treatment: "full",
        totals: {
          files: 0,
          bytes: 0,
          included_files: 0,
          included_bytes: 0,
          secret_scan_skipped: 0,
        },
        digest: "",
      });
      this.data.plans.set(plan.plan_id, plan);
      return plan;
    });
  }

  planSetDecision(
    planId: PlanId,
    itemPath: string,
    decision: Decision,
  ): Promise<Result<"planSetDecision">> {
    return this.call("planSetDecision", CapturePlanSchema, () => {
      const plan = this.plan(planId);
      const item = plan.items.find((i) => i.path === itemPath);
      if (!item) throw new DormbitError("PATH_UNSAFE", `${itemPath} is not in this plan.`, false);
      if (!item.editable)
        throw new DormbitError("PATH_UNSAFE", `${itemPath} can't be changed.`, false);
      item.decision = decision;
      const updated = withTotals(plan);
      this.data.plans.set(planId, updated);
      return updated;
    });
  }

  planApprove(planId: PlanId, input: PlanApproveInput): Promise<Result<"planApprove">> {
    return this.call("planApprove", ApprovedPlanSchema, () => {
      const plan = this.plan(planId);
      if (input.digest !== plan.digest) {
        throw new DormbitError(
          "PLAN_DIGEST_MISMATCH",
          "The plan changed after you reviewed it.",
          false,
        );
      }
      this.approved.set(planId, plan.digest);
      return {
        digest: plan.digest,
        approved_at: this.now(),
        acks: input.acks,
        overrides: input.overrides,
      };
    });
  }

  captureStart(planId: PlanId, onEvent: JobListener<CaptureResult>): Promise<JobId> {
    return this.call("captureStart", null, () => {
      const plan = this.plan(planId);
      if (this.approved.get(planId) !== plan.digest) {
        throw new DormbitError(
          "PLAN_DIGEST_MISMATCH",
          "Approve the current plan before capturing.",
          false,
        );
      }
      const included = plan.items.filter((i) => i.decision === "include");
      const chunks = included.map((i) => ({ files: 1, bytes: i.size }));
      const project = this.data.projects.find((p) =>
        plan.plan_id === IDS.plan ? p.id === IDS.dockline : true,
      );
      return this.startJob(
        "capture",
        CaptureResultSchema,
        onEvent,
        ["hashing", "writing"],
        chunks,
        () => {
          const capsule = {
            id: this.newId(),
            project_name: project?.name.value ?? "project",
            created_at: this.now(),
            size: plan.totals.included_bytes,
            file_count: plan.totals.included_files,
          };
          this.data.capsules.push(capsule);
          return { capsule };
        },
      );
    });
  }

  jobCancel(jobId: JobId): Promise<void> {
    return this.call("jobCancel", null, () => {
      const job = this.jobs.get(jobId);
      if (job && !job.finished) job.cancelled = true;
    });
  }

  capsulePick(): Promise<Result<"capsulePick">> {
    return this.call(
      "capsulePick",
      CapsuleRefSchema.nullable(),
      () => this.data.capsules[0] ?? null,
    );
  }

  capsulesList(): Promise<Result<"capsulesList">> {
    return this.call("capsulesList", CapsuleRefSchema.array(), () => this.data.capsules);
  }

  verifyStart(capsuleId: CapsuleId, onEvent: JobListener<VerificationReport>): Promise<JobId> {
    return this.call("verifyStart", null, () => {
      const capsule = this.capsule(capsuleId);
      return this.startJob(
        "verify",
        VerificationReportSchema,
        onEvent,
        ["reading", "hashing"],
        split(capsule.file_count, capsule.size, 8),
        () => ({ ...this.data.verification, capsule_id: capsule.id }),
      );
    });
  }

  restoreTargetPick(): Promise<Result<"restoreTargetPick">> {
    return this.call("restoreTargetPick", ScopeGrantSchema.nullable(), () =>
      this.grant("RestoreTarget"),
    );
  }

  restorePrecheck(input: RestoreInput): Promise<Result<"restorePrecheck">> {
    return this.call("restorePrecheck", PrecheckReportSchema, () => {
      const capsule = this.capsule(input.capsule_id);
      this.target(input.target_scope_id);
      const issues = validName(input.dest_name)
        ? []
        : ["The folder name must not be empty or contain a slash."];
      return {
        ok: issues.length === 0,
        issues,
        required_bytes: capsule.size,
        available_bytes: 412316860416,
      };
    });
  }

  restoreStart(input: RestoreInput, onEvent: JobListener<RestoreReport>): Promise<JobId> {
    return this.call("restoreStart", null, () => {
      const capsule = this.capsule(input.capsule_id);
      const target = this.target(input.target_scope_id);
      if (!validName(input.dest_name)) {
        throw new DormbitError(
          "PATH_UNSAFE",
          "The folder name must not be empty or contain a slash.",
          false,
        );
      }
      const opId = this.newId();
      return this.startJob(
        "restore",
        RestoreReportSchema,
        onEvent,
        ["staged", "written", "verified", "committed"],
        split(capsule.file_count, capsule.size, 6),
        () => ({
          op_id: opId,
          capsule_id: capsule.id,
          destination_display: `${target.root_display}/${input.dest_name}`,
          files_restored: capsule.file_count,
          bytes_restored: capsule.size,
          verified: true,
        }),
      );
    });
  }

  restoreRecover(request: RecoverRequest): Promise<Result<"restoreRecover">> {
    return this.call("restoreRecover", RecoverResultSchema, () => ({
      op_id: request.op_id,
      state: "Cleaned",
    }));
  }

  briefGet(projectId: string): Promise<Result<"briefGet">> {
    return this.call("briefGet", BriefSchema, () => this.brief(projectId));
  }

  briefExport(projectId: string): Promise<Result<"briefExport">> {
    return this.call("briefExport", ExportResultSchema, () => {
      const p = this.project(projectId);
      const text = this.brief(projectId)
        .sections.map((s) => `## ${s.title}\n\n${s.body}\n`)
        .join("\n");
      return {
        file_name: `${p.name.value}-brief.md`,
        bytes: new TextEncoder().encode(text).length,
      };
    });
  }

  settingsGet(): Promise<Result<"settingsGet">> {
    return this.call("settingsGet", SettingsSchema, () => this.data.settings);
  }

  settingsSet(patch: SettingsPatch): Promise<Result<"settingsSet">> {
    return this.call("settingsSet", SettingsSchema, () => {
      if (patch.reduce_motion !== null) this.data.settings.reduce_motion = patch.reduce_motion;
      if (patch.capsule_max_mb !== null) this.data.settings.capsule_max_mb = patch.capsule_max_mb;
      return this.data.settings;
    });
  }

  consentSet(adapterId: string, granted: boolean): Promise<Result<"consentSet">> {
    return this.call("consentSet", ConsentRecordSchema, () => {
      const adapter = this.data.adapters.find((a) => a.id === adapterId);
      if (!adapter) throw denied("That adapter");
      if (granted && adapter.support === "unsupported") {
        throw new DormbitError("SCOPE_DENIED", `${adapter.name} is not supported.`, false);
      }
      adapter.granted = granted;
      return { adapter_id: adapter.id, granted, updated_at: this.now() };
    });
  }

  adaptersList(): Promise<Result<"adaptersList">> {
    return this.call("adaptersList", AdapterInfoSchema.array(), () => this.data.adapters);
  }

  // ---- helpers ----------------------------------------------------------------------------

  /** Applies an override, runs the body, and parses the result so callers get a copy. */
  private async call<T>(
    name: keyof DormbitClient,
    schema: z.ZodType<T> | null,
    body: () => T,
  ): Promise<T> {
    const overrides: Partial<Record<keyof DormbitClient, unknown>> = this.options.overrides ?? {};
    const override = overrides[name];
    await Promise.resolve();
    if (override instanceof DormbitError) throw override;
    const value = override === undefined ? body() : (override as T);
    return schema === null ? value : schema.parse(value);
  }

  private startJob<R>(
    kind: MockJobKind,
    resultSchema: z.ZodType<R>,
    onEvent: JobListener<R>,
    states: string[],
    chunks: Chunk[],
    result: () => R,
  ): string {
    const id = this.newId();
    const job: Job = { kind, cancelled: false, finished: false };
    this.jobs.set(id, job);
    const eventSchema = JobEventSchema(resultSchema);
    const emit = (e: JobEvent<R>): void => {
      onEvent(eventSchema.parse(e));
    };
    const filesTotal = chunks.reduce((n, c) => n + c.files, 0);
    const bytesTotal = chunks.reduce((n, c) => n + c.bytes, 0);
    const steps: JobEvent<R>[] = states.map((state) => ({ kind: "state", state }));
    let files = 0;
    let bytes = 0;
    for (const c of chunks) {
      files += c.files;
      bytes += c.bytes;
      steps.push({
        kind: "progress",
        files_done: files,
        files_total: filesTotal,
        bytes_done: bytes,
        bytes_total: bytesTotal,
      });
    }
    const run = async (): Promise<void> => {
      for (const step of steps) {
        await this.tick();
        if (job.cancelled) break;
        emit(step);
      }
      await this.tick();
      if (job.cancelled) {
        emit({ kind: "cancelled" });
        return;
      }
      const failure = this.options.failJobs?.[kind];
      emit(failure ? { kind: "failed", error: failure } : { kind: "done", result: result() });
    };
    void run().finally(() => {
      job.finished = true;
    });
    return id;
  }

  private grant(kind: ScopeKind): ScopeGrant {
    const existing = this.data.scopes.find(
      (s) => s.kind === kind && s.root_display === PICKED[kind],
    );
    if (existing) return existing;
    const g: ScopeGrant = {
      id: this.newId(),
      kind,
      root_display: PICKED[kind],
      granted_at: this.now(),
      via: "NativePicker",
    };
    this.data.scopes.push(g);
    return g;
  }

  private project(id: string): ProjectRecord {
    const p = this.data.projects.find((x) => x.id === id);
    if (!p) throw denied("That project");
    return p;
  }

  private plan(id: string): CapturePlan {
    const plan = this.data.plans.get(id);
    if (!plan) throw denied("That plan");
    return plan;
  }

  private capsule(id: string): CapsuleRef {
    const c = this.data.capsules.find((x) => x.id === id);
    if (!c) throw denied("That capsule");
    return c;
  }

  private target(id: string): ScopeGrant {
    const t = this.data.scopes.find((s) => s.id === id && s.kind === "RestoreTarget");
    if (!t) throw denied("That destination");
    return t;
  }

  private brief(projectId: string): Brief {
    const p = this.project(projectId);
    return (
      this.data.briefs.get(p.id) ?? {
        sections: [
          {
            title: "Where you stopped",
            body: `Last commit on \`${p.git?.head.branch ?? "unknown"}\`.`,
          },
        ],
        outcomes: {
          files: "not_captured",
          context: "partial",
          session: "not_captured",
          build: "not_captured",
        },
        evidence_index: [],
      }
    );
  }

  private newId(): string {
    this.seq += 1;
    return `5c1f3a2e-8d4b-4c6a-9f10-${(0x6d2a1c000000 + this.seq).toString(16)}`;
  }

  /** A fixed clock, so mock data is the same on every run. */
  private now(): string {
    return new Date(Date.UTC(2026, 10, 4, 12, 0, this.seq)).toISOString().replace(".000Z", "Z");
  }
}

function withTotals(plan: CapturePlan): CapturePlan {
  const inc = plan.items.filter((i) => i.decision === "include");
  const exc = plan.items.filter((i) => i.decision === "exclude");
  const sum = (xs: PlanItem[]): number => xs.reduce((n, i) => n + i.size, 0);
  return {
    ...plan,
    groups: [
      { name: "Included", file_count: inc.length, bytes: sum(inc) },
      { name: "Excluded", file_count: exc.length, bytes: sum(exc) },
    ],
    totals: {
      files: plan.items.length,
      bytes: sum(plan.items),
      included_files: inc.length,
      included_bytes: sum(inc),
      secret_scan_skipped: 0,
    },
    digest: fakeDigest(plan.items.map((i) => `${i.path}:${i.decision}`).join("\n")),
  };
}

/** Splits real totals into progress chunks that add up exactly. */
function split(files: number, bytes: number, parts: number): Chunk[] {
  const out: Chunk[] = [];
  for (let i = 0; i < parts; i++) {
    const f0 = Math.floor((files * i) / parts);
    const f1 = Math.floor((files * (i + 1)) / parts);
    const b0 = Math.floor((bytes * i) / parts);
    const b1 = Math.floor((bytes * (i + 1)) / parts);
    out.push({ files: f1 - f0, bytes: b1 - b0 });
  }
  return out;
}

const validName = (name: string): boolean =>
  name.trim() !== "" && !name.includes("/") && name !== "." && name !== "..";

/** A stand-in for the plan digest (the real one is SHA-256 in Rust): changes with any decision. */
function fakeDigest(s: string): string {
  let out = "";
  for (let seed = 0; seed < 8; seed++) {
    let h = 0x811c9dc5 ^ seed;
    for (let i = 0; i < s.length; i++) {
      h ^= s.charCodeAt(i);
      h = Math.imul(h, 0x01000193);
    }
    out += (h >>> 0).toString(16).padStart(8, "0");
  }
  return out;
}
