// Zod schemas for every IPC type. Wire types come from Rust (ts-rs) in ../generated; the
// type tests in ../test keep each schema and its generated type identical.
import { z } from "zod";

export * from "./error.js";
export * from "./models.js";
export type * from "../generated/index.js";
// The class wins over the generated wire type of the same name (that one is DormbitErrorShape).
export { DormbitError } from "./error.js";

export const BuildProfileSchema = z.enum(["debug", "release"]);

export const AppInfoSchema = z.strictObject({
  name: z.string(),
  version: z.string(),
  core_version: z.string(),
  profile: BuildProfileSchema,
  min_macos: z.string(),
});

export const AppInfoEventSchema = z.discriminatedUnion("kind", [
  z.strictObject({ kind: z.literal("info"), info: AppInfoSchema }),
  z.strictObject({ kind: z.literal("done") }),
]);
