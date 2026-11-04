// Compile-time proof that each Zod schema infers exactly its Rust-generated type
// (Requirement 12.2). Checked by `pnpm typecheck`. Add a line when a type is added; the
// coverage test fails until every generated type has a schema.
import { expectTypeOf } from "vitest";
import type { z } from "zod";
import type * as G from "../generated/index.js";
import type * as S from "../src/index.js";

expectTypeOf<z.infer<typeof S.ActivitySchema>>().toEqualTypeOf<G.Activity>();
expectTypeOf<z.infer<typeof S.AdapterIdSchema>>().toEqualTypeOf<G.AdapterId>();
expectTypeOf<z.infer<typeof S.AdapterInfoSchema>>().toEqualTypeOf<G.AdapterInfo>();
expectTypeOf<z.infer<typeof S.AdapterSupportSchema>>().toEqualTypeOf<G.AdapterSupport>();
expectTypeOf<z.infer<typeof S.AppInfoSchema>>().toEqualTypeOf<G.AppInfo>();
expectTypeOf<z.infer<typeof S.AppInfoEventSchema>>().toEqualTypeOf<G.AppInfoEvent>();
expectTypeOf<z.infer<typeof S.ApprovedPlanSchema>>().toEqualTypeOf<G.ApprovedPlan>();
expectTypeOf<z.infer<typeof S.BriefSchema>>().toEqualTypeOf<G.Brief>();
expectTypeOf<z.infer<typeof S.BriefOutcomesSchema>>().toEqualTypeOf<G.BriefOutcomes>();
expectTypeOf<z.infer<typeof S.BriefSectionSchema>>().toEqualTypeOf<G.BriefSection>();
expectTypeOf<z.infer<typeof S.BuildProfileSchema>>().toEqualTypeOf<G.BuildProfile>();
expectTypeOf<z.infer<typeof S.CapsuleIdSchema>>().toEqualTypeOf<G.CapsuleId>();
expectTypeOf<z.infer<typeof S.CapsuleRefSchema>>().toEqualTypeOf<G.CapsuleRef>();
expectTypeOf<z.infer<typeof S.CapturePlanSchema>>().toEqualTypeOf<G.CapturePlan>();
expectTypeOf<z.infer<typeof S.CaptureResultSchema>>().toEqualTypeOf<G.CaptureResult>();
expectTypeOf<z.infer<typeof S.CheckStatusSchema>>().toEqualTypeOf<G.CheckStatus>();
expectTypeOf<z.infer<typeof S.ConsentRecordSchema>>().toEqualTypeOf<G.ConsentRecord>();
expectTypeOf<z.infer<typeof S.DecisionSchema>>().toEqualTypeOf<G.Decision>();
expectTypeOf<z.infer<typeof S.DiscoveryResultSchema>>().toEqualTypeOf<G.DiscoveryResult>();
expectTypeOf<z.infer<typeof S.DormbitErrorSchema>>().toEqualTypeOf<G.DormbitError>();
expectTypeOf<z.infer<typeof S.ErrorCodeSchema>>().toEqualTypeOf<
  G.ErrorCode | "IPC_SCHEMA_MISMATCH"
>();
expectTypeOf<z.infer<typeof S.WireErrorCodeSchema>>().toEqualTypeOf<G.ErrorCode>();
expectTypeOf<z.infer<typeof S.EvidenceRefSchema>>().toEqualTypeOf<G.EvidenceRef>();
expectTypeOf<z.infer<typeof S.ExportResultSchema>>().toEqualTypeOf<G.ExportResult>();
expectTypeOf<z.infer<typeof S.FindingSchema>>().toEqualTypeOf<G.Finding>();
expectTypeOf<z.infer<typeof S.FindingStatusSchema>>().toEqualTypeOf<G.FindingStatus>();
expectTypeOf<z.infer<typeof S.GitHeadSchema>>().toEqualTypeOf<G.GitHead>();
expectTypeOf<z.infer<typeof S.GitSnapshotSchema>>().toEqualTypeOf<G.GitSnapshot>();
expectTypeOf<z.infer<typeof S.GitStatusSchema>>().toEqualTypeOf<G.GitStatus>();
expectTypeOf<z.infer<typeof S.GitTreatmentSchema>>().toEqualTypeOf<G.GitTreatment>();
expectTypeOf<z.infer<typeof S.GitUpstreamSchema>>().toEqualTypeOf<G.GitUpstream>();
expectTypeOf<
  z.infer<ReturnType<typeof S.JobEventSchema<typeof S.DiscoveryResultSchema>>>
>().toEqualTypeOf<G.JobEvent<G.DiscoveryResult>>();
expectTypeOf<
  z.infer<ReturnType<typeof S.JobEventSchema<typeof S.RestoreReportSchema>>>
>().toEqualTypeOf<G.JobEvent<G.RestoreReport>>();
expectTypeOf<z.infer<typeof S.JobIdSchema>>().toEqualTypeOf<G.JobId>();
expectTypeOf<z.infer<typeof S.MarksSchema>>().toEqualTypeOf<G.Marks>();
expectTypeOf<z.infer<typeof S.NamedValueSchema>>().toEqualTypeOf<G.NamedValue>();
expectTypeOf<z.infer<typeof S.OpIdSchema>>().toEqualTypeOf<G.OpId>();
expectTypeOf<z.infer<typeof S.OutcomeSchema>>().toEqualTypeOf<G.Outcome>();
expectTypeOf<z.infer<typeof S.PlanApproveInputSchema>>().toEqualTypeOf<G.PlanApproveInput>();
expectTypeOf<z.infer<typeof S.PlanGroupSchema>>().toEqualTypeOf<G.PlanGroup>();
expectTypeOf<z.infer<typeof S.PlanIdSchema>>().toEqualTypeOf<G.PlanId>();
expectTypeOf<z.infer<typeof S.PlanItemSchema>>().toEqualTypeOf<G.PlanItem>();
expectTypeOf<z.infer<typeof S.PlanItemKindSchema>>().toEqualTypeOf<G.PlanItemKind>();
expectTypeOf<z.infer<typeof S.PlanTotalsSchema>>().toEqualTypeOf<G.PlanTotals>();
expectTypeOf<z.infer<typeof S.PrecheckReportSchema>>().toEqualTypeOf<G.PrecheckReport>();
expectTypeOf<z.infer<typeof S.PreservationSchema>>().toEqualTypeOf<G.Preservation>();
expectTypeOf<z.infer<typeof S.ProjectIdSchema>>().toEqualTypeOf<G.ProjectId>();
expectTypeOf<z.infer<typeof S.ProjectRecordSchema>>().toEqualTypeOf<G.ProjectRecord>();
expectTypeOf<z.infer<typeof S.RecoverActionSchema>>().toEqualTypeOf<G.RecoverAction>();
expectTypeOf<z.infer<typeof S.RecoverRequestSchema>>().toEqualTypeOf<G.RecoverRequest>();
expectTypeOf<z.infer<typeof S.RecoverResultSchema>>().toEqualTypeOf<G.RecoverResult>();
expectTypeOf<z.infer<typeof S.RegistryMatchSchema>>().toEqualTypeOf<G.RegistryMatch>();
expectTypeOf<z.infer<typeof S.RestoreInputSchema>>().toEqualTypeOf<G.RestoreInput>();
expectTypeOf<z.infer<typeof S.RestoreJournalSchema>>().toEqualTypeOf<G.RestoreJournal>();
expectTypeOf<z.infer<typeof S.RestoreReportSchema>>().toEqualTypeOf<G.RestoreReport>();
expectTypeOf<z.infer<typeof S.RestoreStateSchema>>().toEqualTypeOf<G.RestoreState>();
expectTypeOf<z.infer<typeof S.SafeU64Schema>>().toEqualTypeOf<G.SafeU64>();
expectTypeOf<z.infer<typeof S.ScopeGrantSchema>>().toEqualTypeOf<G.ScopeGrant>();
expectTypeOf<z.infer<typeof S.ScopeIdSchema>>().toEqualTypeOf<G.ScopeId>();
expectTypeOf<z.infer<typeof S.ScopeKindSchema>>().toEqualTypeOf<G.ScopeKind>();
expectTypeOf<z.infer<typeof S.ScopeViaSchema>>().toEqualTypeOf<G.ScopeVia>();
expectTypeOf<z.infer<typeof S.SettingsSchema>>().toEqualTypeOf<G.Settings>();
expectTypeOf<z.infer<typeof S.SettingsPatchSchema>>().toEqualTypeOf<G.SettingsPatch>();
expectTypeOf<z.infer<typeof S.SizeInfoSchema>>().toEqualTypeOf<G.SizeInfo>();
expectTypeOf<z.infer<typeof S.VerificationCheckSchema>>().toEqualTypeOf<G.VerificationCheck>();
expectTypeOf<z.infer<typeof S.VerificationReportSchema>>().toEqualTypeOf<G.VerificationReport>();
