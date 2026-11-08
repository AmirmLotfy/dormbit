// Journey views shared by the web demo and the desktop app. They only render what the core
// returned; nothing here invents a result. Statuses pair a glyph with a word.
import { useEffect, useMemo, useState } from "react";
import type {
  CoreFinding,
  CoreRestoreReport,
  EvidenceResult,
  JobProgress,
  VerifyReport,
} from "@dormbit/schema";
import { DormbitError } from "@dormbit/schema";
import { bytes } from "./spans";

export function errorText(e: unknown): string {
  if (e instanceof DormbitError) return `${e.code}: ${e.message}`;
  return "Something went wrong in the demo engine.";
}

export function useAction() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const run = async (f: () => Promise<void> | void) => {
    setBusy(true);
    setError(null);
    try {
      await f();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };
  return { busy, error, run };
}

export function ErrorNote({ error }: { error: string | null }) {
  if (error === null) return null;
  return (
    <p className="note note-fail" role="alert">
      <span aria-hidden="true">✕</span> {error}
    </p>
  );
}

/** Shows real progress events one by one; all at once under reduced motion (Pacing). */
export function Phases({
  events,
  reducedMotion,
}: {
  events: readonly JobProgress[];
  reducedMotion: boolean;
}) {
  const [shown, setShown] = useState(reducedMotion ? events.length : 0);
  useEffect(() => {
    if (reducedMotion) {
      setShown(events.length);
      return;
    }
    setShown(0);
    const id = setInterval(() => {
      setShown((n) => {
        if (n >= events.length) {
          clearInterval(id);
          return n;
        }
        return n + 1;
      });
    }, 120);
    return () => {
      clearInterval(id);
    };
  }, [events, reducedMotion]);
  const phases = useMemo(() => {
    const last = new Map<string, JobProgress>();
    for (const e of events.slice(0, shown)) last.set(e.phase, e);
    return [...last.values()];
  }, [events, shown]);
  const finished = shown >= events.length;
  return (
    <>
      <p className="sr-only" role="status">
        {finished ? `All ${String(events.length)} progress events shown.` : ""}
      </p>
      <ol className="phases">
        {phases.map((p) => (
          <li key={p.phase} className="phase">
            <span className="mono phase-name">{p.phase}</span>
            <span>
              {String(p.files_done)} of {String(p.files_total)} items · {bytes(p.bytes_done)} of{" "}
              {bytes(p.bytes_total)}
            </span>
          </li>
        ))}
      </ol>
    </>
  );
}

export const byRule = (e: EvidenceResult | undefined, ...rules: string[]): CoreFinding[] =>
  e?.findings.filter((f) => rules.includes(f.rule)) ?? [];

export const REASON_WORDS: Record<string, string> = {
  sensitive_env: "Sensitive: environment file, never captured",
  rebuildable: "Rebuildable: tools recreate it",
  data_file: "Data file: needs your decision",
  git_hooks: "Git hooks: never captured",
  project_file: "Project file",
  git_internal: "Git internal",
};

export const CHECK: Record<
  VerifyReport["checks"][number]["status"],
  { glyph: string; word: string }
> = {
  pass: { glyph: "✓", word: "Pass" },
  fail: { glyph: "✕", word: "Fail" },
  unknown: { glyph: "?", word: "Unknown" },
  not_run: { glyph: "–", word: "Not run" },
};

export function VerifyView({ report }: { report: VerifyReport }) {
  const verified = report.status === "verified";
  return (
    <div>
      <p className={`seal ${verified ? "seal-on" : "seal-off"}`} role="status">
        <span aria-hidden="true">{verified ? "◉" : "◌"}</span>{" "}
        {verified ? "Verified" : report.status === "failed" ? "Failed" : "Unsupported"}
        {report.error_code !== null && <span className="mono"> · {report.error_code}</span>}
      </p>
      <p className="muted">{String(report.checks.length)} checks.</p>
      <ol className="checks">
        {report.checks.map((c) => (
          <li key={c.id} className={`check check-${c.status}`}>
            <span className="mono">{c.id}</span>
            <span>
              <span aria-hidden="true">{CHECK[c.status].glyph}</span> {CHECK[c.status].word}
            </span>
            <span className="muted">{c.detail}</span>
          </li>
        ))}
      </ol>
      {report.failed_files.length > 0 && (
        <>
          <h2>Damaged files</h2>
          <ul>
            {report.failed_files.map((f) => (
              <li key={f.path}>
                <span className="mono">{f.path}</span> · {f.reason.replaceAll("_", " ")}
              </li>
            ))}
          </ul>
        </>
      )}
      <p className="muted">
        Registry: {report.registry_match}. {report.limitations.join(" ")}
      </p>
    </div>
  );
}

export function RestoreView({ report }: { report: CoreRestoreReport }) {
  const ok = report.status === "verified";
  return (
    <div>
      <p className={`note ${ok ? "note-ok" : "note-fail"}`} role="status">
        <span aria-hidden="true">{ok ? "✓" : "✕"}</span> {report.status.replaceAll("_", " ")} ·{" "}
        <span className="mono">{report.target_display}</span>
        {report.error && <span className="mono"> · {report.error.code}</span>}
      </p>
      <dl className="facts">
        <dt>Files written and re-hashed</dt>
        <dd>
          {String(report.files_verified.files)} files, {String(report.mismatches.length)} mismatches
        </dd>
        <dt>Writes outside the destination</dt>
        <dd>{String(report.writes_outside_destination)}</dd>
        <dt>Git HEAD and refs</dt>
        <dd>{report.git_check.refs.kind.replaceAll("_", " ")}</dd>
        <dt>Not restored</dt>
        <dd>{String(report.not_restored.length)} items (each with its reason in the report)</dd>
      </dl>
      {report.mismatches.length > 0 && (
        <>
          <h2>Files that didn&apos;t match</h2>
          <ul>
            {report.mismatches.map((m) => (
              <li key={m.path} className="mono">
                {m.path}
              </li>
            ))}
          </ul>
        </>
      )}
      {report.warnings.length > 0 && <p className="muted">{report.warnings.join(" ")}</p>}
    </div>
  );
}
