import "@dormbit/ui/src/journey/journey.css";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { MockClient } from "@dormbit/client";
import { App } from "./App";
import type { AppInfoHost } from "./appInfo";
import { loadAppInfo } from "./appInfo";
import { Journey } from "./journey/Journey";

// Development only: `VITE_DORMBIT_CLIENT=mock pnpm --filter @dormbit/desktop dev` runs the old
// shell in a plain browser on MockClient. Release builds never take this branch.
const mock =
  import.meta.env.DEV && import.meta.env.VITE_DORMBIT_CLIENT === "mock"
    ? new MockClient()
    : undefined;
const host: AppInfoHost | undefined = mock
  ? { call: (onMessage) => mock.appInfo(onMessage) }
  : undefined;

const root = document.getElementById("root");
if (!root) throw new Error("missing #root");
const view = createRoot(root);

function render(version?: string) {
  view.render(
    <StrictMode>
      {mock ? <App host={host} mock={mock} /> : <Journey version={version} />}
    </StrictMode>,
  );
}
render();
if (!mock) {
  loadAppInfo(() => undefined).then(
    (info) => {
      render(info.version);
    },
    () => undefined,
  );
}

// Recording builds only: clicks through the journey for the demo video.
if (import.meta.env.VITE_AUTOPILOT === "1") {
  void import("./journey/autopilot").then((m) => m.autopilot());
}
