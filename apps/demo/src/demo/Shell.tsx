// The judge demo shell: Synthetic_Banner, Guided_Rail, Whats_Real_Panel, Reset_Control, and
// the current step. The core is real Rust compiled to WebAssembly; the files are synthetic.
import {
  Component,
  type ReactNode,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { DormbitError } from "@dormbit/schema";
import { DemoApi, missingFeatures } from "./api";
import { errorText } from "@dormbit/ui";
import { type DemoData, STEPS, STEP_VIEWS } from "./steps";

const HOME = import.meta.env.BASE_URL;

const SHA = import.meta.env.VITE_BUILD_SHA;
const BUILD_LABEL =
  SHA !== undefined && /^[0-9a-f]{7,40}$/.test(SHA)
    ? `Preview build ${SHA.slice(0, 7)}`
    : "Local build";

export const BANNER = "Synthetic Demo — No Local Files Accessed";

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

/** Shows an engine or schema error thrown while a step renders, instead of a blank page. */
class StepBoundary extends Component<{ children: ReactNode }, { error: string | null }> {
  override state = { error: null as string | null };
  static getDerivedStateFromError(e: unknown) {
    return { error: errorText(e) };
  }
  override render() {
    if (this.state.error !== null)
      return (
        <p className="note note-fail" role="alert">
          <span aria-hidden="true">✕</span> {this.state.error} Use Reset to start over.
        </p>
      );
    return this.props.children;
  }
}

type Load =
  | { kind: "loading" }
  | { kind: "missing"; what: string[] }
  | { kind: "error"; message: string }
  | { kind: "ready"; api: DemoApi };

function WhatsReal() {
  return (
    <aside className="real" aria-labelledby="real-title">
      <h2 id="real-title">What&apos;s real here</h2>
      <ul>
        <li>
          <strong>Real:</strong> the planner, capsule writer, verifier, restore, evidence rules,
          brief, and every hash are DORMBIT&apos;s Rust code compiled to WebAssembly.
        </li>
        <li>
          <strong>Synthetic:</strong> the files are a fictional project and live only in this
          tab&apos;s memory. Nothing is read from or written to your computer.
        </li>
        <li>
          <strong>Mocked or precomputed:</strong> the folder picker is a mock; the Git snapshot and
          agent session metadata are precomputed from the fixture; the clock is fixed; the
          working-tree status check runs only in the desktop app.
        </li>
        <li>
          <strong>Paced:</strong> steps 6 (capture) and 8 (restore) show their real progress events
          one at a time so you can read them. With reduced motion, they appear at once.
        </li>
        <li>
          <strong>Independent check:</strong> step 9 re-hashes with your browser&apos;s WebCrypto, a
          SHA-256 implementation separate from the WebAssembly core.
        </li>
      </ul>
    </aside>
  );
}

export function Shell() {
  const [load, setLoad] = useState<Load>({ kind: "loading" });
  const [data, setData] = useState<DemoData>({});
  const [done, setDone] = useState<ReadonlySet<number>>(new Set());
  const [current, setCurrent] = useState(0);
  const [notice, setNotice] = useState<string | null>(null);
  // Counts navigations and resets: focus moves to the step heading only after one of them,
  // never on page load, and Reset remounts the step even when it is already step 1.
  const [nav, setNav] = useState(0);
  const heading = useRef<HTMLHeadingElement>(null);
  const reducedMotion = useReducedMotion();

  useEffect(() => {
    const missing = missingFeatures();
    if (missing.length > 0) {
      setLoad({ kind: "missing", what: missing });
      return;
    }
    DemoApi.load().then(
      (api) => {
        setLoad({ kind: "ready", api });
      },
      (e: unknown) => {
        setLoad({
          kind: "error",
          message:
            e instanceof DormbitError ? `${e.code}: ${e.message}` : "The demo could not start.",
        });
      },
    );
  }, []);

  useEffect(() => {
    if (nav > 0) heading.current?.focus();
  }, [nav]);

  const firstOpen = STEPS.findIndex((_, i) => !done.has(i));
  const open = (i: number) => {
    const blocking = STEPS.findIndex((_, j) => j < i && !done.has(j));
    if (blocking !== -1) {
      setNotice(
        `Step ${String(i + 1)} needs step ${String(blocking + 1)} (${STEPS[blocking] ?? ""}) first.`,
      );
      setCurrent(blocking);
      setNav((n) => n + 1);
      return;
    }
    setNotice(null);
    setCurrent(i);
    setNav((n) => n + 1);
  };

  const reset = () => {
    if (load.kind === "ready") load.api.reset();
    setData({});
    setDone(new Set());
    setNotice(null);
    setCurrent(0);
    setNav((n) => n + 1);
  };

  const View = STEP_VIEWS[current];
  return (
    <div className="demo">
      <header>
        <p className="banner" role="note">
          <span aria-hidden="true">◆</span> {BANNER}
        </p>
        <div className="top">
          <a className="brand" href={HOME}>
            DORMBIT
          </a>
          <span className="muted">Pick up where your project stopped.</span>
          <button type="button" className="btn" onClick={reset}>
            Reset
          </button>
        </div>
      </header>
      <div className="layout">
        <nav className="rail" aria-label="Demo steps">
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
              const word = { done: "done", current: "current", next: "next", locked: "locked" }[
                state
              ];
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
                      <span aria-hidden="true">{glyph}</span> {word}
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
          {notice !== null && (
            <p className="note note-warn" role="status">
              <span aria-hidden="true">!</span> {notice}
            </p>
          )}
          {load.kind === "loading" && (
            <p role="status">Loading the DORMBIT core and the synthetic project…</p>
          )}
          {load.kind === "missing" && (
            <p className="note note-fail" role="alert">
              <span aria-hidden="true">✕</span> This browser lacks {load.what.join(" and ")}, which
              the demo needs.
            </p>
          )}
          {load.kind === "error" && (
            <p className="note note-fail" role="alert">
              <span aria-hidden="true">✕</span> {load.message} The demo didn&apos;t start.
            </p>
          )}
          {load.kind === "ready" && View && (
            <StepBoundary key={`${String(nav)}-${String(current)}`}>
              <View
                api={load.api}
                data={data}
                reducedMotion={reducedMotion}
                update={(patch) => {
                  setData((d) => ({ ...d, ...patch }));
                }}
                done={() => {
                  setDone((s) => new Set([...s, current]));
                }}
              />
            </StepBoundary>
          )}
          {load.kind === "ready" && done.has(current) && current + 1 < STEPS.length && (
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
        <WhatsReal />
      </div>
      <footer className="demo-foot">
        <span className="mono">{BUILD_LABEL}</span>
        <span aria-hidden="true"> · </span>
        <a href="https://github.com/AmirmLotfy/dormbit">Source on GitHub</a>
      </footer>
    </div>
  );
}
