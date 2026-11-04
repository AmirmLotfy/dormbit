// DormbitClient, the MockClient, and helpers. The contract suite is a separate entry
// (`@dormbit/client/contract`) so production bundles never import Vitest.
export { CLIENT_METHODS, type DormbitClient, type JobListener } from "./DormbitClient.js";
export {
  MockClient,
  type MockClientOptions,
  type MockJobKind,
  type MockOverrides,
} from "./mock/MockClient.js";
export { IDS as MOCK_IDS } from "./mock/data.js";
