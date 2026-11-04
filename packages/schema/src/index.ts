// Zod schemas for every IPC type. Wire types come from Rust (ts-rs) in ../generated;
// the type-equality tests in ../test keep these schemas and those types in lockstep.
import { z } from "zod";

/** The 13 codes Rust can send. Order matches `ErrorCode::ALL` in dormbit-core. */
export const ERROR_CODES = [
  "SCOPE_DENIED",
  "PATH_UNSAFE",
  "PLAN_DIGEST_MISMATCH",
  "FILE_CHANGED_DURING_CAPTURE",
  "CAPSULE_MALFORMED",
  "CAPSULE_TRUNCATED",
  "CAPSULE_HASH_MISMATCH",
  "CAPSULE_VERSION_UNSUPPORTED",
  "RESTORE_DEST_EXISTS",
  "RESTORE_COLLISION",
  "RESTORE_VOLUME_UNSUPPORTED",
  "RESTORE_INSUFFICIENT_SPACE",
  "CANCELLED",
] as const;

export const ErrorCodeSchema = z.enum(ERROR_CODES);
export type ErrorCode = z.infer<typeof ErrorCodeSchema>;

export const DormbitErrorSchema = z.strictObject({
  code: ErrorCodeSchema,
  message: z.string(),
  retryable: z.boolean(),
});
export type DormbitErrorShape = z.infer<typeof DormbitErrorSchema>;

export const BuildProfileSchema = z.enum(["debug", "release"]);

export const AppInfoSchema = z.strictObject({
  name: z.string(),
  version: z.string(),
  core_version: z.string(),
  profile: BuildProfileSchema,
  min_macos: z.string(),
});
export type AppInfo = z.infer<typeof AppInfoSchema>;

export const AppInfoEventSchema = z.discriminatedUnion("kind", [
  z.strictObject({ kind: z.literal("info"), info: AppInfoSchema }),
  z.strictObject({ kind: z.literal("done") }),
]);
export type AppInfoEvent = z.infer<typeof AppInfoEventSchema>;

/** Codes the TypeScript side may raise: the wire codes plus one produced only by the client. */
export type ClientErrorCode = ErrorCode | "IPC_SCHEMA_MISMATCH";

/** The error every client method rejects with. */
export class DormbitError extends Error {
  readonly code: ClientErrorCode;
  readonly retryable: boolean;

  constructor(code: ClientErrorCode, message: string, retryable: boolean) {
    super(message);
    this.name = "DormbitError";
    this.code = code;
    this.retryable = retryable;
  }

  /** Builds one from a validated wire error. */
  static fromWire(e: DormbitErrorShape): DormbitError {
    return new DormbitError(e.code, e.message, e.retryable);
  }

  /** Raised when a message from Rust fails validation. Never retryable. */
  static schemaMismatch(what: string): DormbitError {
    return new DormbitError("IPC_SCHEMA_MISMATCH", `unexpected ${what} from the app`, false);
  }
}
