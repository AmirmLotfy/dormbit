import { Channel, invoke } from "@tauri-apps/api/core";
import {
  AppInfoEventSchema,
  AppInfoSchema,
  DormbitError,
  DormbitErrorSchema,
  type AppInfo,
  type AppInfoEvent,
} from "@dormbit/schema";

/** The two Tauri calls this module needs, so tests can replace them. */
export interface AppInfoHost {
  /** Starts the command and wires `onMessage` to the Channel. Resolves with the raw return. */
  call(onMessage: (raw: unknown) => void): Promise<unknown>;
}

/** The real host: a Tauri Channel plus `invoke("app_info")`. */
export const tauriHost: AppInfoHost = {
  call(onMessage) {
    const channel = new Channel<unknown>();
    channel.onmessage = onMessage;
    return invoke("app_info", { onEvent: channel });
  },
};

/**
 * Loads app info, validating every message and the return value with Zod.
 * Any failure to parse becomes `IPC_SCHEMA_MISMATCH` (never retryable).
 * An error sent by Rust is passed through as a `DormbitError` when it has the right shape.
 */
export async function loadAppInfo(
  onEvent: (e: AppInfoEvent) => void,
  host: AppInfoHost = tauriHost,
): Promise<AppInfo> {
  let bad: DormbitError | undefined;
  const onMessage = (raw: unknown): void => {
    const parsed = AppInfoEventSchema.safeParse(raw);
    if (parsed.success) {
      onEvent(parsed.data);
    } else {
      bad ??= DormbitError.schemaMismatch("event");
    }
  };

  let ret: unknown;
  try {
    ret = await host.call(onMessage);
  } catch (e) {
    const wire = DormbitErrorSchema.safeParse(e);
    throw wire.success ? DormbitError.fromWire(wire.data) : DormbitError.schemaMismatch("error");
  }
  if (bad) throw bad;

  const info = AppInfoSchema.safeParse(ret);
  if (!info.success) throw DormbitError.schemaMismatch("response");
  return info.data;
}
