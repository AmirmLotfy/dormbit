// Strict schemas for the core result types the demo engine (and, in Task 22/26/30, the
// desktop commands) return: evidence, discovery, capture, verification, restore, and the
// brief. They mirror the Rust serde shapes field for field; unknown fields fail.
// STATUS D-48: these replace the earlier placeholder IPC shapes when the Tauri commands land.
import { z } from "zod";
import { DormbitErrorSchema, WireErrorCodeSchema } from "./error.js";
import { GitTreatmentSchema, SafeU64Schema } from "./models.js";

const n = SafeU64Schema;
const int = z.number().int();

// ---- evidence ------------------------------------------------------------------------------

export const SpanSchema = z.discriminatedUnion("t", [
  z.strictObject({ t: z.literal("Text"), v: z.string() }),
  z.strictObject({ t: z.literal("Quote"), v: z.string() }),
  z.strictObject({ t: z.literal("Code"), v: z.string() }),
  z.strictObject({ t: z.literal("Count"), v: n }),
]);
export type Span = z.infer<typeof SpanSchema>;

export const FindingStateSchema = z.enum(["confirmed", "inferred", "unknown", "unsupported"]);

export const CoreEvidenceRefSchema = z.discriminatedUnion("kind", [
  z.strictObject({
    kind: z.literal("file_lines"),
    path: z.string(),
    start_line: n,
    end_line: n,
    anchor: z.string().nullable(),
  }),
  z.strictObject({ kind: z.literal("manifest_key"), path: z.string(), pointer: z.string() }),
  z.strictObject({
    kind: z.literal("file_stat"),
    path: z.string(),
    stat: z.enum(["file", "dir", "absent"]),
    size: n.nullable(),
  }),
  z.strictObject({ kind: z.literal("git_ref"), name: z.string() }),
  z.strictObject({ kind: z.literal("git_commit"), oid: z.string() }),
  z.strictObject({ kind: z.literal("git_status"), modified: n, staged: n, untracked: n }),
  z.strictObject({ kind: z.literal("git_config_key"), key: z.string() }),
  z.strictObject({ kind: z.literal("agent_session"), adapter: z.string(), session_id: z.string() }),
  z.strictObject({ kind: z.literal("capsule"), capsule_id: z.string(), pointer: z.string() }),
  z.strictObject({ kind: z.literal("restore"), op_id: z.string() }),
  z.strictObject({ kind: z.literal("rule_limit"), rule: z.string(), limit: n, dropped: n }),
]);
export type CoreEvidenceRef = z.infer<typeof CoreEvidenceRefSchema>;

export const CoreFindingSchema = z.strictObject({
  id: z.string().regex(/^F-[0-9a-f]{12}$/),
  rule: z.string(),
  statement: z.array(SpanSchema),
  status: FindingStateSchema,
  evidence: z.array(CoreEvidenceRefSchema).min(1),
});
export type CoreFinding = z.infer<typeof CoreFindingSchema>;

export const EvidenceResultSchema = z.strictObject({
  findings: z.array(CoreFindingSchema),
  files_scanned: n,
  files_skipped: n,
});
export type EvidenceResult = z.infer<typeof EvidenceResultSchema>;

// ---- discovery -----------------------------------------------------------------------------

export const DiscoveredProjectSchema = z.strictObject({
  id: z.string(),
  rel_root: z.string(),
  name: z.string(),
  name_source: z.string(),
  manifests: z.array(z.string()),
  docs: z.array(z.string()),
  agent_artifacts: z.array(z.string()),
  has_git: z.boolean(),
  latest_mtime: z.string().nullable(),
  latest_mtime_unix: int.nullable(),
  files_scanned: n,
  total_bytes: n,
  rebuildable_bytes: n,
  partial: z.boolean(),
  idle_days: int.nullable(),
});
export type DiscoveredProject = z.infer<typeof DiscoveredProjectSchema>;

export const DiscoverResultSchema = z.strictObject({
  root_display: z.string(),
  result: z.strictObject({
    projects: z.array(DiscoveredProjectSchema),
    entries_scanned: n,
    partial: z.boolean(),
  }),
});
export type DiscoverResult = z.infer<typeof DiscoverResultSchema>;

// ---- capture -------------------------------------------------------------------------------

export const PhaseSchema = z.enum([
  "preparing",
  "hashing",
  "rechecking",
  "assembling",
  "committing",
  "recording",
  "done",
]);
export const JobProgressSchema = z.strictObject({
  phase: PhaseSchema,
  files_done: n,
  files_total: n,
  bytes_done: n,
  bytes_total: n,
});
export type JobProgress = z.infer<typeof JobProgressSchema>;

export const CapsuleRecordSchema = z.strictObject({
  capsule_id: z.string(),
  project_id: z.string(),
  project_name: z.string(),
  library_grant: z.string(),
  file_name: z.string(),
  file_sha256: z.string(),
  manifest_sha256: z.string(),
  plan_digest: z.string(),
  size: n,
  file_count: n,
  created_at: int,
  last_verified_at: int.nullable(),
});
export type CapsuleRecord = z.infer<typeof CapsuleRecordSchema>;

export const ManifestEntrySchema = z.discriminatedUnion("type", [
  z.strictObject({
    type: z.literal("file"),
    path: z.string(),
    size: n,
    mode: n,
    mtime: int,
    sha256: z.string(),
    object: z.strictObject({
      encoding: z.enum(["gzip", "identity"]),
      stored_size: n,
      stored_sha256: z.string(),
    }),
  }),
  z.strictObject({
    type: z.literal("symlink"),
    path: z.string(),
    target: z.string(),
    restored: z.boolean(),
  }),
]);

export const ManifestSchema = z.strictObject({
  schema_version: n,
  capsule_id: z.string(),
  created_at: z.string(),
  app_version: z.string(),
  project: z.strictObject({ name: z.string(), name_source: z.string() }),
  plan_digest: z.string(),
  git: z
    .strictObject({
      treatment: GitTreatmentSchema,
      head: z
        .strictObject({
          branch: z.string().nullable(),
          detached: z.boolean(),
          commit: z.string().nullable(),
        })
        .nullable(),
      refs: z.array(z.strictObject({ name: z.string(), target: z.string() })),
    })
    .nullable(),
  entries: z.array(ManifestEntrySchema),
  excluded: z.array(z.strictObject({ path: z.string(), group: z.string(), reason: z.string() })),
  warnings: z.array(
    z.strictObject({
      id: z.string(),
      code: z.string(),
      path: z.string(),
      line: n.nullable(),
      acknowledged: z.boolean(),
    }),
  ),
  limitations: z.array(z.string()),
  agent_refs: z.array(z.strictObject({ adapter_id: z.string(), reference: z.string() })),
  totals: z.strictObject({
    files: n,
    symlinks: n,
    objects: n,
    bytes: n,
    stored_bytes: n,
    excluded: n,
  }),
});
export type Manifest = z.infer<typeof ManifestSchema>;

export const CaptureOutputSchema = z.strictObject({
  events: z.array(JobProgressSchema),
  record: CapsuleRecordSchema,
  manifest: ManifestSchema,
  not_captured: z.array(z.string()),
});
export type CaptureOutput = z.infer<typeof CaptureOutputSchema>;

// ---- verification --------------------------------------------------------------------------

export const CheckIdSchema = z.enum([
  "V01_MARKER",
  "V02_HEADER",
  "V03_ENTRY_TYPE_NAME",
  "V04_ORDER",
  "V05_PADDING",
  "V06_LIMITS",
  "V07_MANIFEST",
  "V08_PATHS",
  "V09_OBJECT_SET",
  "V10_STORED_HASH",
  "V11_CONTENT_HASH",
  "V12_TOTALS",
  "V13_TRAILER",
  "V14_REGISTRY",
]);
export const VerifyReportSchema = z.strictObject({
  capsule_id: z.string().nullable(),
  file_sha256: z.string().nullable(),
  manifest_sha256: z.string().nullable(),
  checks: z.array(
    z.strictObject({
      id: CheckIdSchema,
      status: z.enum(["pass", "fail", "unknown", "not_run"]),
      detail: z.string(),
    }),
  ),
  failed_files: z.array(
    z.strictObject({
      path: z.string(),
      reason: z.enum([
        "stored_hash_mismatch",
        "content_hash_mismatch",
        "size_mismatch",
        "unverifiable",
      ]),
    }),
  ),
  registry_match: z.enum(["Match", "Mismatch", "NotInRegistry", "NotChecked"]),
  status: z.enum(["verified", "failed", "unsupported"]),
  error_code: WireErrorCodeSchema.nullable(),
  limitations: z.array(z.string()),
});
export type VerifyReport = z.infer<typeof VerifyReportSchema>;

// ---- restore -------------------------------------------------------------------------------

const NotRestoredSchema = z.strictObject({ path: z.string(), reason: z.string() });

export const RestorePrecheckSchema = z.strictObject({
  checks: z.array(
    z.strictObject({
      id: z.string(),
      status: z.enum(["pass", "fail", "warn", "not_applicable"]),
      code: WireErrorCodeSchema.nullable(),
      detail: z.string(),
    }),
  ),
  volume: z.strictObject({
    fs_type: z.string(),
    block_size: n,
    free_bytes: n,
    case: z.enum(["Sensitive", "Insensitive", "Unknown"]),
  }),
  required_bytes: n,
  files_total: n,
  bytes_total: n,
  skipped: z.array(NotRestoredSchema),
  target_display: z.string(),
  blocking_failed: z.boolean(),
});
export type RestorePrecheck = z.infer<typeof RestorePrecheckSchema>;

const TotalsSchema = z.strictObject({ files: n, bytes: n });

export const CoreRestoreReportSchema = z.strictObject({
  schema_version: n,
  op_id: z.string(),
  capsule_id: z.string(),
  target_display: z.string(),
  started_at: z.string(),
  finished_at: z.string(),
  status: z.enum(["verified", "failed", "rolled_back", "committed_unverified"]),
  recovered: z.boolean(),
  error: DormbitErrorSchema.nullable(),
  files_written: TotalsSchema,
  files_verified: TotalsSchema,
  mismatches: z.array(
    z.strictObject({
      path: z.string(),
      expected_sha256: z.string(),
      actual_sha256: z.string().nullable(),
      expected_size: n,
      actual_size: n.nullable(),
      expected_mode: n,
      actual_mode: n.nullable(),
    }),
  ),
  not_restored: z.array(NotRestoredSchema),
  executables: z.array(z.string()),
  stripped_mode_bits: z.array(z.strictObject({ path: z.string(), removed: z.array(z.string()) })),
  source_unchanged: z.discriminatedUnion("kind", [
    z.strictObject({ kind: z.literal("unchanged") }),
    z.strictObject({ kind: z.literal("changed"), paths: z.array(z.string()) }),
    z.strictObject({ kind: z.literal("not_checked"), reason: z.string() }),
  ]),
  writes_outside_destination: n,
  git_check: z.strictObject({
    refs: z.discriminatedUnion("kind", [
      z.strictObject({ kind: z.literal("match") }),
      z.strictObject({ kind: z.literal("mismatch"), refs: z.array(z.string()) }),
      z.strictObject({ kind: z.literal("not_applicable") }),
      z.strictObject({ kind: z.literal("refs_not_recorded") }),
    ]),
    worktree: z.discriminatedUnion("kind", [
      z.strictObject({ kind: z.literal("match") }),
      z.strictObject({ kind: z.literal("differs"), detail: z.string() }),
      z.strictObject({ kind: z.literal("not_checked"), reason: z.string() }),
      z.strictObject({ kind: z.literal("not_applicable") }),
    ]),
  }),
  warnings: z.array(z.string()),
  limits: z.array(z.string()),
});
export type CoreRestoreReport = z.infer<typeof CoreRestoreReportSchema>;

export const RestoreOutcomeSchema = z.discriminatedUnion("refused", [
  z.strictObject({
    refused: z.literal(true),
    code: WireErrorCodeSchema,
    precheck: RestorePrecheckSchema,
  }),
  z.strictObject({
    refused: z.literal(false),
    events: z.array(JobProgressSchema),
    report: CoreRestoreReportSchema,
  }),
]);
export type RestoreOutcome = z.infer<typeof RestoreOutcomeSchema>;

export const RestoredFilesSchema = z.strictObject({
  root_display: z.string(),
  files: z.array(
    z.strictObject({
      path: z.string(),
      expected_sha256: z.string(),
      size: n,
      present: z.boolean(),
    }),
  ),
});
export type RestoredFiles = z.infer<typeof RestoredFilesSchema>;

export const DestinationSchema = z.strictObject({
  root_display: z.string(),
  entries: z.array(z.strictObject({ name: z.string(), kind: z.string() })),
});
export type Destination = z.infer<typeof DestinationSchema>;

// ---- brief ---------------------------------------------------------------------------------

export const BriefLineSchema = z.strictObject({
  spans: z.array(SpanSchema),
  cites: z.array(z.string()),
});
export type BriefLine = z.infer<typeof BriefLineSchema>;

export const CoreBriefSchema = z.strictObject({
  app_version: z.string(),
  generated_on: z.string(),
  outcomes: z.strictObject({
    files: BriefLineSchema,
    context: BriefLineSchema,
    session: BriefLineSchema,
    build: BriefLineSchema,
  }),
  sections: z.array(
    z.strictObject({ id: z.string(), title: z.string(), lines: z.array(BriefLineSchema) }),
  ),
  evidence_index: z.array(CoreFindingSchema),
});
export type CoreBrief = z.infer<typeof CoreBriefSchema>;

export const BriefResultSchema = z.strictObject({ brief: CoreBriefSchema, markdown: z.string() });

// ---- demo controls -------------------------------------------------------------------------

export const ByteChangeSchema = z.strictObject({
  capsule_offset: n,
  region: z.string(),
  old: n,
  new: n,
});
export type ByteChange = z.infer<typeof ByteChangeSchema>;

export const DemoAppInfoSchema = z.strictObject({
  version: z.string(),
  build: z.literal("wasm"),
  synthetic: z.literal(true),
});
