// The desktop app's main window: the guided journey over the real folders the user grants.
import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import type { ComponentType } from "react";
import { NativeApi } from "./api";
import { type JourneyData, type StepProps, STEPS, STEP_VIEWS } from "./steps";

function subscribeMotion(cb: () => void) {
  const m = window.matchMedia("(prefers-reduced-motion: reduce)");
  m.addEventListener("change", cb);
  return () => {
    m.removeEventListener("change", cb);
  };
}
function useReducedMotion(): boolean {
  return useSyncExternalStore(
    subscribeMotion,
    () => window.matchMedia("(prefers-reduced-motion: reduce)").matches,
    () => true,
  );
}

function OnThisMac() {
  return (
    <aside className="real" aria-labelledby="mac-title">
      <h2 id="mac-title">On this Mac</h2>
      <ul>
        <li>DORMBIT reads only the folders you choose in the macOS panel.</li>
        <li>
          It never runs your code, scripts, or the git command, and never changes your project.
        </li>
        <li>
          Capsules go into DORMBIT&apos;s own library folder; restores create a new folder only.
        </li>
        <li>No account, no network calls, no telemetry.</li>
      </ul>
    </aside>
  );
}

export function Journey({
  api = new NativeApi(),
  version,
}: {
  api?: NativeApi | undefined;
  version?: string | undefined;
}) {
  const [data, setData] = useState<JourneyData>({});
  const [done, setDone] = useState<ReadonlySet<number>>(new Set());
  const [current, setCurrent] = useState(0);
  const [nav, setNav] = useState(0);
  const heading = useRef<HTMLHeadingElement>(null);
  const reducedMotion = useReducedMotion();

  useEffect(() => {
    if (nav > 0) heading.current?.focus();
  }, [nav]);

  const firstOpen = STEPS.findIndex((_, i) => !done.has(i));
  const open = (i: number) => {
    const blocking = STEPS.findIndex((_, j) => j < i && !done.has(j));
    setCurrent(blocking === -1 ? i : blocking);
    setNav((n) => n + 1);
  };
  const startOver = () => {
    setData({});
    setDone(new Set());
    setCurrent(0);
    setNav((n) => n + 1);
  };

  const View: ComponentType<StepProps> | undefined = STEP_VIEWS[current];
  return (
    <div className="demo">
      <header>
        <div className="top">
          <span className="brand">DORMBIT</span>
          <span className="muted">Pick up where your project stopped.</span>
          <button type="button" className="btn" onClick={startOver}>
            Start over
          </button>
        </div>
      </header>
      <div className="layout">
        <nav className="rail" aria-label="Journey steps">
          <ol>
            {STEPS.map((title, i) => {
              const state = done.has(i)
                ? "done"
                : i === current
                  ? "current"
                  : i === firstOpen
                    ? "next"
                    : "locked";
              const glyph = { done: "✓", current: "▸", next: "○", locked: "·" }[state];
              return (
                <li key={title} className={`rail-step rail-${state}`}>
                  <button
                    type="button"
                    aria-current={i === current ? "step" : undefined}
                    onClick={() => {
                      open(i);
                    }}
                  >
                    <span className="rail-num">{String(i + 1).padStart(2, "0")}</span>
                    <span className="rail-title">{title}</span>
                    <span className="rail-state">
                      <span aria-hidden="true">{glyph}</span> {state}
                    </span>
                  </button>
                </li>
              );
            })}
          </ol>
        </nav>
        <main className="stage">
          <h1 ref={heading} tabIndex={-1}>
            <span className="muted mono">Step {String(current + 1).padStart(2, "0")}</span>{" "}
            {STEPS[current]}
          </h1>
          {View && (
            <View
              key={`${String(nav)}-${String(current)}`}
              api={api}
              data={data}
              reducedMotion={reducedMotion}
              update={(patch: Partial<JourneyData>) => {
                setData((d) => ({ ...d, ...patch }));
              }}
              done={() => {
                setDone((s) => new Set([...s, current]));
              }}
            />
          )}
          {done.has(current) && current + 1 < STEPS.length && (
            <button
              type="button"
              className="btn btn-next"
              onClick={() => {
                open(current + 1);
              }}
            >
              Next: {STEPS[current + 1]} →
            </button>
          )}
        </main>
        <OnThisMac />
      </div>
      <footer className="demo-foot">
        <span className="mono">DORMBIT {version ?? ""}</span>
      </footer>
    </div>
  );
}
