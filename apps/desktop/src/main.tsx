import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { MockClient } from "@dormbit/client";
import { App } from "./App";
import type { AppInfoHost } from "./appInfo";

// Development only: `VITE_DORMBIT_CLIENT=mock pnpm --filter @dormbit/desktop dev` runs the UI
// in a plain browser on MockClient. Release builds never take this branch.
const mock =
  import.meta.env.DEV && import.meta.env.VITE_DORMBIT_CLIENT === "mock"
    ? new MockClient()
    : undefined;
const host: AppInfoHost | undefined = mock
  ? { call: (onMessage) => mock.appInfo(onMessage) }
  : undefined;

const root = document.getElementById("root");
if (!root) throw new Error("missing #root");
createRoot(root).render(
  <StrictMode>
    <App host={host} mock={mock} />
  </StrictMode>,
);
