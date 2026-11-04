import { useEffect, useState } from "react";
import type { AppInfo } from "@dormbit/schema";
import type { DormbitClient } from "@dormbit/client";
import { loadAppInfo, type AppInfoHost } from "./appInfo";
import { MockProjects } from "./MockProjects";

type State = { kind: "loading" } | { kind: "ready"; info: AppInfo } | { kind: "error" };

/** Temporary shell screen: shows the name and version that Rust reports. */
export function App({
  host,
  mock,
}: {
  host?: AppInfoHost | undefined;
  mock?: DormbitClient | undefined;
}) {
  const [state, setState] = useState<State>({ kind: "loading" });

  useEffect(() => {
    let live = true;
    let latest: AppInfo | undefined;
    loadAppInfo((e) => {
      if (e.kind === "info") latest = e.info;
    }, host)
      .then((info) => {
        if (live) setState({ kind: "ready", info: latest ?? info });
      })
      .catch(() => {
        if (live) setState({ kind: "error" });
      });
    return () => {
      live = false;
    };
  }, [host]);

  return (
    <main>
      <h1>{state.kind === "ready" ? `${state.info.name} ${state.info.version}` : "DORMBIT"}</h1>
      <p>Pick up where your project stopped.</p>
      {state.kind === "error" ? (
        <p role="alert">The app could not read its own version. Restart DORMBIT.</p>
      ) : null}
      {mock ? <MockProjects client={mock} /> : null}
    </main>
  );
}
