// The demo's client: the real DORMBIT core compiled to WebAssembly, over a virtual home
// folder. Every result is parsed with its schema from @dormbit/schema; a parse failure is
// IPC_SCHEMA_MISMATCH, and engine rejections keep their DORMBIT error code.
import {
  ApprovedPlanSchema,
  BriefResultSchema,
  ByteChangeSchema,
  CaptureOutputSchema,
  CapturePlanSchema,
  DemoAppInfoSchema,
  DestinationSchema,
  DiscoverResultSchema,
  DormbitError,
  EvidenceResultSchema,
  RestoreOutcomeSchema,
  RestorePrecheckSchema,
  RestoredFilesSchema,
  VerifyReportSchema,
  parseError,
} from "@dormbit/schema";
import type { z } from "zod";
import init, { DemoEngine } from "../wasm/pkg/dormbit_wasm.js";
import wasmUrl from "../wasm/pkg/dormbit_wasm_bg.wasm?url";

const BASE = import.meta.env.BASE_URL;
export const DATASET = {
  tar: `${BASE}demo-data/dockline-invoices.tar`,
  sidecar: `${BASE}demo-data/dockline-invoices.sidecar.json`,
};

function parse<S extends z.ZodType>(schema: S, text: string, what: string): z.infer<S> {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw DormbitError.schemaMismatch(what);
  }
  const r = schema.safeParse(value);
  if (!r.success) throw DormbitError.schemaMismatch(what);
  return r.data;
}

/** Engine errors arrive as DormbitError JSON text. */
function rethrow(e: unknown): never {
  if (typeof e === "string") {
    try {
      throw parseError(JSON.parse(e));
    } catch (inner) {
      if (inner instanceof DormbitError) throw inner;
    }
  }
  throw parseError(e);
}

export type Missing = "WebAssembly" | "WebCrypto";

/** Features the demo needs; checked before anything loads. */
export function missingFeatures(): Missing[] {
  const out: Missing[] = [];
  if (typeof WebAssembly !== "object") out.push("WebAssembly");
  if (typeof crypto === "undefined" || typeof crypto.subtle === "undefined") out.push("WebCrypto");
  return out;
}

export class DemoApi {
  private constructor(private readonly engine: DemoEngine) {}

  /** Loads the core and the synthetic dataset. Only same-origin files are fetched. */
  static async load(): Promise<DemoApi> {
    await init({ module_or_path: wasmUrl });
    const [tarRes, sideRes] = await Promise.all([fetch(DATASET.tar), fetch(DATASET.sidecar)]);
    if (!tarRes.ok || !sideRes.ok) {
      throw new DormbitError("IO_FAILED", "The demo dataset could not be loaded.", false);
    }
    const tar = new Uint8Array(await tarRes.arrayBuffer());
    const sidecar = await sideRes.text();
    try {
      return new DemoApi(new DemoEngine(tar, sidecar));
    } catch (e) {
      rethrow(e);
    }
  }

  private call<S extends z.ZodType>(schema: S, command: string, args: object = {}): z.infer<S> {
    let text: string;
    try {
      text = this.engine.call(command, JSON.stringify(args));
    } catch (e) {
      rethrow(e);
    }
    return parse(schema, text, command);
  }

  appInfo() {
    return this.call(DemoAppInfoSchema, "app_info");
  }
  discover() {
    return this.call(DiscoverResultSchema, "discover");
  }
  evidence() {
    return this.call(EvidenceResultSchema, "evidence");
  }
  planCreate(gitTreatment: "full" | "working_tree_only" = "full") {
    return this.call(CapturePlanSchema, "plan_create", { git_treatment: gitTreatment });
  }
  planSetDecision(path: string, decision: "include" | "exclude") {
    return this.call(CapturePlanSchema, "plan_set_decision", { path, decision });
  }
  planApprove(digest: string, acks: string[]) {
    return this.call(ApprovedPlanSchema, "plan_approve", { digest, acks });
  }
  capture() {
    return this.call(CaptureOutputSchema, "capture");
  }
  verify() {
    return this.call(VerifyReportSchema, "verify");
  }
  destination() {
    return this.call(DestinationSchema, "destination");
  }
  restorePrecheck(targetName: string) {
    return this.call(RestorePrecheckSchema, "restore_precheck", { target_name: targetName });
  }
  restore(targetName: string) {
    return this.call(RestoreOutcomeSchema, "restore", { target_name: targetName });
  }
  restoredFiles() {
    return this.call(RestoredFilesSchema, "restored_files");
  }
  brief() {
    return this.call(BriefResultSchema, "brief");
  }
  corrupt(
    control: "stored_file" | "capsule_bytes" | "after_verification",
    seed: number,
    xor: number,
    path = "",
  ) {
    return this.call(ByteChangeSchema, "corrupt", { control, seed, xor, path });
  }
  capsuleBytes(): Uint8Array {
    try {
      return this.engine.capsule_bytes();
    } catch (e) {
      rethrow(e);
    }
  }
  restoredFileBytes(path: string): Uint8Array {
    try {
      return this.engine.restored_file_bytes(path);
    } catch (e) {
      rethrow(e);
    }
  }
  reset(): void {
    this.engine.reset();
  }
  stateDigest(): string {
    return this.engine.state_digest();
  }
}

/** Independent check: SHA-256 with the browser's WebCrypto, not the WebAssembly core. */
export async function sha256Hex(bytes: Uint8Array): Promise<string> {
  const copy = new Uint8Array(bytes);
  const d = await crypto.subtle.digest("SHA-256", copy);
  return [...new Uint8Array(d)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

/** A random seed and a non-zero XOR mask from the browser's CSPRNG. */
export function randomControl(): { seed: number; xor: number } {
  const a = new Uint32Array(2);
  crypto.getRandomValues(a);
  return { seed: a[0] ?? 0, xor: ((a[1] ?? 1) % 255) + 1 };
}
