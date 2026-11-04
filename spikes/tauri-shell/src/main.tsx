import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";

type AppInfo = { name: string; version: string; tauri: string };
type Grant = { id: string; display: string };

function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [grant, setGrant] = useState<Grant | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<AppInfo>("app_info").then(setInfo, (e: unknown) => setError(String(e)));
  }, []);

  const pick = async () => {
    setError(null);
    try {
      setGrant(await invoke<Grant | null>("pick_folder"));
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <main style={{ fontFamily: "system-ui", padding: 24 }}>
      <h1>DORMBIT spike</h1>
      <p data-testid="info">{info ? `${info.name} ${info.version} (tauri ${info.tauri})` : "loading…"}</p>
      <button type="button" onClick={pick}>
        Choose folder…
      </button>
      {grant && (
        <p data-testid="grant">
          Granted {grant.id}: {grant.display}
        </p>
      )}
      {error && <p role="alert">{error}</p>}
    </main>
  );
}

const root = document.getElementById("root");
if (!root) throw new Error("missing #root");
createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
