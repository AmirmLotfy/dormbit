// Zod schemas for the §8 models and the command argument and result types, field for field
// with `crates/dormbit-core/src/model/ipc.rs`. Objects are strict: an unknown key fails.
import { z } from "zod";
import { DormbitErrorSchema } from "./error.js";

// ---- primitives ------------------------------------------------------------------------------

/** 0 to 2^53 - 1, the integers JavaScript represents exactly (Rust `SafeU64`). */
export const SafeU64Schema = z.number().int().min(0).max(Number.MAX_SAFE_INTEGER);

export const ScopeIdSchema = z.string();
export const ProjectIdSchema = z.string();
export const PlanIdSchema = z.string();
export const CapsuleIdSchema = z.string();
export const JobIdSchema = z.string();
export const OpIdSchema = z.string();
export const AdapterIdSchema = z.string();

// ---- jobs ------------------------------------------------------------------------------------

/** The events a job streams. Takes the schema of the job's result. */
export const JobEventSchema = <R extends z.ZodType>(result: R) =>
  z.discriminatedUnion("kind", [
    z.strictObject({ kind: z.literal("state"), state: z.string() }),
    z.strictObject({
      kind: z.literal("progress"),
      files_done: SafeU64Schema,
      files_total: SafeU64Schema,
      bytes_done: SafeU64Schema,
      bytes_total: SafeU64Schema,
    }),
    z.strictObject({ kind: z.literal("done"), result }),
    z.strictObject({ kind: z.literal("failed"), error: DormbitErrorSchema }),
    z.strictObject({ kind: z.literal("cancelled") }),
  ]);

// ---- scope -----------------------------------------------------------------------------------

export const ScopeKindSchema = z.enum(["Read", "CapsuleLibrary", "RestoreTarget"]);
export const ScopeViaSchema = z.enum(["NativePicker"]);
export const ScopeGrantSchema = z.strictObject({
  id: ScopeIdSchema,
  kind: ScopeKindSchema,
  root_display: z.string(),
  granted_at: z.string(),
  via: ScopeViaSchema,
});

// ---- findings and projects -------------------------------------------------------------------

export const EvidenceRefSchema = z.strictObject({ path: z.string(), detail: z.string() });
export const FindingStatusSchema = z.enum(["confirmed", "inferred", "unknown"]);
export const FindingSchema = z.strictObject({
  id: z.string(),
  rule: z.string(),
  statement: z.string(),
  status: FindingStatusSchema,
  evidence: z.array(EvidenceRefSchema),
});
export const NamedValueSchema = z.strictObject({ value: z.string(), source: z.string() });
export const GitHeadSchema = z.strictObject({
  branch: z.string().nullable(),
  detached: z.boolean(),
  commit: z.string(),
  time: z.string(),
  summary: z.string(),
});
export const GitUpstreamSchema = z.strictObject({
  ahead: SafeU64Schema,
  behind: SafeU64Schema,
  as_of_last_fetch: z.boolean(),
});
export const GitStatusSchema = z.strictObject({
  modified: SafeU64Schema,
  staged: SafeU64Schema,
  untracked: SafeU64Schema,
  deleted: SafeU64Schema,
  conflicted: SafeU64Schema,
});
export const GitSnapshotSchema = z.strictObject({
  head: GitHeadSchema,
  upstream: GitUpstreamSchema.nullable(),
  status: GitStatusSchema,
  stash_count: SafeU64Schema,
  worktree_external: z.boolean(),
  submodules: z.array(z.string()),
  lfs_detected: z.boolean(),
  risky_config: z.array(z.string()),
  remotes_redacted: z.array(z.string()),
});
export const ActivitySchema = z.strictObject({
  latest_mtime: z.string().nullable(),
  scanned: SafeU64Schema,
  partial: z.boolean(),
});
export const SizeInfoSchema = z.strictObject({
  total: SafeU64Schema,
  rebuildable: SafeU64Schema,
  partial: z.boolean(),
});
export const MarksSchema = z.strictObject({ dormant: z.boolean() });
export const PreservationSchema = z.strictObject({
  last_capsule: CapsuleIdSchema.nullable(),
  last_verified_at: z.string().nullable(),
  last_verified_restore: z.string().nullable(),
});
export const ProjectRecordSchema = z.strictObject({
  id: ProjectIdSchema,
  scope_id: ScopeIdSchema,
  rel_root: z.string(),
  name: NamedValueSchema,
  stacks: z.array(FindingSchema),
  git: GitSnapshotSchema.nullable(),
  activity: ActivitySchema,
  size: SizeInfoSchema,
  docs: z.array(z.string()),
  manifests: z.array(z.string()),
  agent_artifacts: z.array(z.string()),
  marks: MarksSchema,
  preservation: PreservationSchema,
});
export const DiscoveryResultSchema = z.strictObject({
  projects_found: SafeU64Schema,
  entries_scanned: SafeU64Schema,
});

// ---- plan ------------------------------------------------------------------------------------

export const DecisionSchema = z.enum(["include", "exclude", "undecided"]);
export const PlanItemKindSchema = z.enum(["file", "dir", "symlink", "special"]);
export const PlanItemSchema = z.strictObject({
  path: z.string(),
  kind: PlanItemKindSchema,
  size: SafeU64Schema,
  decision: DecisionSchema,
  reason: z.string(),
  editable: z.boolean(),
  warning: z.string().nullable(),
});
export const PlanGroupSchema = z.strictObject({
  name: z.string(),
  file_count: SafeU64Schema,
  bytes: SafeU64Schema,
});
export const GitTreatmentSchema = z.enum(["full", "working_tree_only"]);
export const PlanTotalsSchema = z.strictObject({
  files: SafeU64Schema,
  bytes: SafeU64Schema,
  included_files: SafeU64Schema,
  included_bytes: SafeU64Schema,
  secret_scan_skipped: SafeU64Schema,
});
export const CapturePlanSchema = z.strictObject({
  plan_id: PlanIdSchema,
  items: z.array(PlanItemSchema),
  groups: z.array(PlanGroupSchema),
  git_treatment: GitTreatmentSchema,
  totals: PlanTotalsSchema,
  digest: z.string(),
});
export const PlanApproveInputSchema = z.strictObject({
  digest: z.string(),
  acks: z.array(z.string()),
  overrides: z.array(z.string()),
});
export const ApprovedPlanSchema = z.strictObject({
  digest: z.string(),
  approved_at: z.string(),
  acks: z.array(z.string()),
  overrides: z.array(z.string()),
});

// ---- capsules and verification ---------------------------------------------------------------

export const CapsuleRefSchema = z.strictObject({
  id: CapsuleIdSchema,
  project_name: z.string(),
  created_at: z.string(),
  size: SafeU64Schema,
  file_count: SafeU64Schema,
});
export const CaptureResultSchema = z.strictObject({ capsule: CapsuleRefSchema });
export const CheckStatusSchema = z.enum(["pass", "fail", "skipped"]);
export const RegistryMatchSchema = z.enum(["Match", "Mismatch", "NotInRegistry"]);
export const VerificationCheckSchema = z.strictObject({
  id: z.string(),
  status: CheckStatusSchema,
  detail: z.string(),
});
export const VerificationReportSchema = z.strictObject({
  capsule_id: CapsuleIdSchema,
  file_sha256: z.string(),
  manifest_sha256: z.string(),
  checks: z.array(VerificationCheckSchema),
  failed_files: z.array(z.string()),
  registry_match: RegistryMatchSchema,
  status: CheckStatusSchema,
});

// ---- restore ---------------------------------------------------------------------------------

export const RestoreStateSchema = z.enum(["Staged", "Written", "Verified", "Committed", "Cleaned"]);
export const RestoreJournalSchema = z.strictObject({
  op_id: OpIdSchema,
  staging: z.string(),
  owner_token: z.string(),
  state: RestoreStateSchema,
});
export const RestoreInputSchema = z.strictObject({
  capsule_id: CapsuleIdSchema,
  target_scope_id: ScopeIdSchema,
  dest_name: z.string(),
});
export const PrecheckReportSchema = z.strictObject({
  ok: z.boolean(),
  issues: z.array(z.string()),
  required_bytes: SafeU64Schema,
  available_bytes: SafeU64Schema,
});
export const RestoreReportSchema = z.strictObject({
  op_id: OpIdSchema,
  capsule_id: CapsuleIdSchema,
  destination_display: z.string(),
  files_restored: SafeU64Schema,
  bytes_restored: SafeU64Schema,
  verified: z.boolean(),
});
export const RecoverActionSchema = z.enum(["discard"]);
export const RecoverRequestSchema = z.strictObject({
  op_id: OpIdSchema,
  action: RecoverActionSchema,
});
export const RecoverResultSchema = z.strictObject({ op_id: OpIdSchema, state: RestoreStateSchema });

// ---- brief, settings, consent, adapters ------------------------------------------------------

export const BriefSectionSchema = z.strictObject({ title: z.string(), body: z.string() });
export const OutcomeSchema = z.enum(["preserved", "partial", "not_captured"]);
export const BriefOutcomesSchema = z.strictObject({
  files: OutcomeSchema,
  context: OutcomeSchema,
  session: OutcomeSchema,
  build: OutcomeSchema,
});
export const BriefSchema = z.strictObject({
  sections: z.array(BriefSectionSchema),
  outcomes: BriefOutcomesSchema,
  evidence_index: z.array(EvidenceRefSchema),
});
export const ExportResultSchema = z.strictObject({ file_name: z.string(), bytes: SafeU64Schema });
export const SettingsSchema = z.strictObject({
  reduce_motion: z.boolean(),
  capsule_max_mb: SafeU64Schema,
});
export const SettingsPatchSchema = z.strictObject({
  reduce_motion: z.boolean().nullable(),
  capsule_max_mb: SafeU64Schema.nullable(),
});
export const ConsentRecordSchema = z.strictObject({
  adapter_id: AdapterIdSchema,
  granted: z.boolean(),
  updated_at: z.string(),
});
export const AdapterSupportSchema = z.enum(["supported", "unsupported"]);
export const AdapterInfoSchema = z.strictObject({
  id: AdapterIdSchema,
  name: z.string(),
  support: AdapterSupportSchema,
  granted: z.boolean(),
});
