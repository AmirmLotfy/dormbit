import { z } from "zod";

/** The 13 codes Rust can send. Order matches `ErrorCode::ALL` in dormbit-core. */
export const WIRE_ERROR_CODES = [
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

/** Raised only by TypeScript, when a message from Rust fails validation (Requirement 14.4). */
export const CLIENT_ONLY_CODE = "IPC_SCHEMA_MISMATCH";

/** Codes that may arrive over IPC. */
export const WireErrorCodeSchema = z.enum(WIRE_ERROR_CODES);
/** Every code the UI can see: the wire codes plus the client-only code (Requirement 12.5). */
export const ErrorCodeSchema = z.enum([...WIRE_ERROR_CODES, CLIENT_ONLY_CODE]);
export type ClientErrorCode = z.infer<typeof ErrorCodeSchema>;

/** The error value Rust sends. An unknown code fails to parse. */
export const DormbitErrorSchema = z.strictObject({
  code: WireErrorCodeSchema,
  message: z.string(),
  retryable: z.boolean(),
});
export type DormbitErrorShape = z.infer<typeof DormbitErrorSchema>;

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
    return new DormbitError(CLIENT_ONLY_CODE, `unexpected ${what} from the app`, false);
  }
}

/**
 * Turns anything a host call rejected with into a DormbitError. A value with the wire shape
 * keeps its code; anything else, including an unknown code, becomes IPC_SCHEMA_MISMATCH.
 */
export function parseError(e: unknown): DormbitError {
  if (e instanceof DormbitError) return e;
  const wire = DormbitErrorSchema.safeParse(e);
  return wire.success ? DormbitError.fromWire(wire.data) : DormbitError.schemaMismatch("error");
}
