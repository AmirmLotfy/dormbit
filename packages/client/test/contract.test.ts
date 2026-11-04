import { runClientContract } from "../src/contract.js";
import { MockClient } from "../src/index.js";

runClientContract("MockClient", () => new MockClient());
runClientContract(
  "MockClient (microtask ticks)",
  () => new MockClient({ tick: () => Promise.resolve() }),
);
