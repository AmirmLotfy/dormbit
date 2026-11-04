// Compile-time checks, run by `pnpm typecheck`.
import { expectTypeOf } from "vitest";
import type { CLIENT_METHODS, DormbitClient } from "../src/index.js";
import type { MockClient } from "../src/index.js";

// CLIENT_METHODS lists every method, and nothing else (Requirement 13.1).
expectTypeOf<(typeof CLIENT_METHODS)[number]>().toEqualTypeOf<keyof DormbitClient>();
// The mock implements the whole interface (Requirement 13.3).
expectTypeOf<MockClient>().toExtend<DormbitClient>();
