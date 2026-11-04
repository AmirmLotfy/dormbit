import type {
  AdapterId,
  AdapterInfo,
  AppInfo,
  AppInfoEvent,
  ApprovedPlan,
  Brief,
  CapsuleId,
  CapsuleRef,
  CapturePlan,
  CaptureResult,
  ConsentRecord,
  Decision,
  DiscoveryResult,
  ExportResult,
  Finding,
  JobEvent,
  JobId,
  PlanApproveInput,
  PlanId,
  PrecheckReport,
  ProjectId,
  ProjectRecord,
  RecoverRequest,
  RecoverResult,
  RestoreInput,
  RestoreReport,
  ScopeGrant,
  ScopeId,
  ScopeKind,
  Settings,
  SettingsPatch,
  VerificationReport,
} from "@dormbit/schema";

/** Receives every event of one job. A job ends with exactly one done, failed, or cancelled. */
export type JobListener<R> = (event: JobEvent<R>) => void;

/**
 * One method per allowlisted command (master plan §6), in plan order. No parameter carries a
 * filesystem path: folders are referred to by grant ID (SCP-1). `itemPath` is a path relative
 * to the project root inside an approved plan, never an absolute path. Every method rejects
 * with a `DormbitError`.
 */
export interface DormbitClient {
  appInfo(onEvent?: (e: AppInfoEvent) => void): Promise<AppInfo>;
  scopePick(kind: ScopeKind): Promise<ScopeGrant | null>;
  scopeList(): Promise<ScopeGrant[]>;
  scopeRevoke(scopeId: ScopeId): Promise<void>;
  discoveryStart(scopeId: ScopeId, onEvent: JobListener<DiscoveryResult>): Promise<JobId>;
  discoveryCancel(jobId: JobId): Promise<void>;
  projectsList(): Promise<ProjectRecord[]>;
  projectGet(projectId: ProjectId): Promise<ProjectRecord>;
  projectSetDormant(projectId: ProjectId, dormant: boolean): Promise<ProjectRecord>;
  evidenceGet(projectId: ProjectId): Promise<Finding[]>;
  planCreate(projectId: ProjectId): Promise<CapturePlan>;
  planSetDecision(planId: PlanId, itemPath: string, decision: Decision): Promise<CapturePlan>;
  planApprove(planId: PlanId, input: PlanApproveInput): Promise<ApprovedPlan>;
  captureStart(planId: PlanId, onEvent: JobListener<CaptureResult>): Promise<JobId>;
  jobCancel(jobId: JobId): Promise<void>;
  capsulePick(): Promise<CapsuleRef | null>;
  capsulesList(): Promise<CapsuleRef[]>;
  verifyStart(capsuleId: CapsuleId, onEvent: JobListener<VerificationReport>): Promise<JobId>;
  restoreTargetPick(): Promise<ScopeGrant | null>;
  restorePrecheck(input: RestoreInput): Promise<PrecheckReport>;
  restoreStart(input: RestoreInput, onEvent: JobListener<RestoreReport>): Promise<JobId>;
  restoreRecover(request: RecoverRequest): Promise<RecoverResult>;
  briefGet(projectId: ProjectId): Promise<Brief>;
  briefExport(projectId: ProjectId): Promise<ExportResult>;
  settingsGet(): Promise<Settings>;
  settingsSet(patch: SettingsPatch): Promise<Settings>;
  consentSet(adapterId: AdapterId, granted: boolean): Promise<ConsentRecord>;
  adaptersList(): Promise<AdapterInfo[]>;
}

/** The method names in plan order. A type test keeps this equal to `keyof DormbitClient`. */
export const CLIENT_METHODS = [
  "appInfo",
  "scopePick",
  "scopeList",
  "scopeRevoke",
  "discoveryStart",
  "discoveryCancel",
  "projectsList",
  "projectGet",
  "projectSetDormant",
  "evidenceGet",
  "planCreate",
  "planSetDecision",
  "planApprove",
  "captureStart",
  "jobCancel",
  "capsulePick",
  "capsulesList",
  "verifyStart",
  "restoreTargetPick",
  "restorePrecheck",
  "restoreStart",
  "restoreRecover",
  "briefGet",
  "briefExport",
  "settingsGet",
  "settingsSet",
  "consentSet",
  "adaptersList",
] as const satisfies readonly (keyof DormbitClient)[];
