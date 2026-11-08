// The 12 journey-8 steps. Each step calls the real core through DemoApi and shows exactly
// what it returned; nothing here invents a result.
import { useEffect, useMemo, useState } from "react";
import type {
  ByteChange,
  CaptureOutput,
  CapturePlan,
  CoreBrief,
  CoreFinding,
  CoreRestoreReport,
  Destination,
  DiscoverResult,
  EvidenceResult,
  JobProgress,
  RestoreOutcome,
  VerifyReport,
} from "@dormbit/schema";
import { DormbitError } from "@dormbit/schema";
import { type DemoApi, randomControl, sha256Hex } from "./api";
import { CitedLine, FindingRow, Spans, bytes } from "./spans";

export interface DemoData {
  discover?: DiscoverResult | undefined;
  evidence?: EvidenceResult | undefined;
  plan?: CapturePlan | undefined;
  digest?: string | undefined;
  capture?: CaptureOutput | undefined;
  verify?: VerifyReport | undefined;
  refused?: RestoreOutcome | undefined;
  restore?: CoreRestoreReport | undefined;
  rehash?: { path: string; ok: boolean; present: boolean }[] | undefined;
  brief?: { brief: CoreBrief; markdown: string } | undefined;
  change?: { control: string; change: ByteChange } | undefined;
  caught?:
    | {
        verify?: VerifyReport | undefined;
        restore?: RestoreOutcome | undefined;
        dest?: Destination | undefined;
      }
    | undefined;
}

export interface StepProps {
  api: DemoApi;
  data: DemoData;
  update: (patch: Partial<DemoData>) => void;
  done: () => void;
  reducedMotion: boolean;
}

export const STEPS = [
  "Find projects",
  "Project overview",
  "Evidence",
  "Preservation plan",
  "Decide and approve",
  "Make the capsule",
  "Verify the capsule",
  "Restore into a new folder",
  "Independent check",
  "Resurrection Brief",
  "Damage it yourself",
  "Watch it get caught",
] as const;

export function errorText(e: unknown): string {
  if (e instanceof DormbitError) return `${e.code}: ${e.message}`;
  return "Something went wrong in the demo engine.";
}

function useAction() {
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

function ErrorNote({ error }: { error: string | null }) {
  if (error === null) return null;
  return (
    <p className="note note-fail" role="alert">
      <span aria-hidden="true">✕</span> {error}
    </p>
  );
}

/** Shows real progress events one by one; all at once under reduced motion (Pacing). */
function Phases({
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

const byRule = (e: EvidenceResult | undefined, ...rules: string[]): CoreFinding[] =>
  e?.findings.filter((f) => rules.includes(f.rule)) ?? [];

// ---- 1. Find projects ----------------------------------------------------------------------

export function StepFind({ api, data, update, done }: StepProps) {
  const { busy, error, run } = useAction();
  const [picked, setPicked] = useState<string | null>(null);
  const folders = ["~/Projects", "~/Desktop", "~/Downloads"];
  return (
    <section>
      <p>
        Grant DORMBIT one folder. This is a mock picker over a virtual home folder; in the app,
        macOS shows its own folder dialog and DORMBIT gets access to that folder only.
      </p>
      <fieldset className="picker">
        <legend>Choose a folder (virtual)</legend>
        {folders.map((f) => (
          <label key={f} className="pick">
            <input
              type="radio"
              name="folder"
              value={f}
              checked={picked === f}
              disabled={f !== "~/Projects"}
              onChange={() => {
                setPicked(f);
              }}
            />
            <span className="mono">{f}</span>
            {f !== "~/Projects" && <span className="muted"> (empty in this demo)</span>}
          </label>
        ))}
      </fieldset>
      <button
        type="button"
        className="btn btn-primary"
        disabled={picked === null || busy}
        onClick={() => {
          void run(() => {
            update({ discover: api.discover() });
            done();
          });
        }}
      >
        Grant and scan
      </button>
      <ErrorNote error={error} />
      {data.discover && (
        <div className="shelf">
          <h2>Shelf: {data.discover.root_display}</h2>
          <p className="muted">
            {String(data.discover.result.entries_scanned)} entries looked at.{" "}
            {data.discover.result.partial ? "The scan hit a limit." : "Full scan."}
          </p>
          <ul className="cards">
            {data.discover.result.projects.map((p) => (
              <li key={p.id} className="card">
                <h3>{p.name}</h3>
                <dl className="facts">
                  <dt>Folder</dt>
                  <dd className="mono">~/Projects/{p.rel_root}</dd>
                  <dt>Latest file change</dt>
                  <dd>{p.latest_mtime?.slice(0, 10) ?? "unknown"}</dd>
                  <dt>Size</dt>
                  <dd>
                    {bytes(p.total_bytes)} ({bytes(p.rebuildable_bytes)} rebuildable)
                  </dd>
                  <dt>Found</dt>
                  <dd className="mono">
                    {[...p.manifests, ...(p.has_git ? [".git"] : []), ...p.agent_artifacts].join(
                      ", ",
                    )}
                  </dd>
                </dl>
              </li>
            ))}
          </ul>
          <p className="muted">
            <code className="code">notes/</code> has no manifest and no Git, so it isn&apos;t listed
            as a project.
          </p>
        </div>
      )}
    </section>
  );
}

// ---- 2. Overview ---------------------------------------------------------------------------

function ensureEvidence(api: DemoApi, data: DemoData, update: StepProps["update"]): EvidenceResult {
  if (data.evidence) return data.evidence;
  const e = api.evidence();
  update({ evidence: e });
  return e;
}

export function StepOverview({ api, data, update, done }: StepProps) {
  const { busy, error, run } = useAction();
  const read = (
    <button
      type="button"
      className="btn btn-primary"
      disabled={busy || data.evidence !== undefined}
      onClick={() => {
        void run(() => {
          ensureEvidence(api, data, update);
          done();
        });
      }}
    >
      Read the project
    </button>
  );
  const rows = byRule(
    data.evidence,
    "identity.name",
    "git.branch",
    "git.feature_branch",
    "git.wip_commit",
    "git.uncommitted",
    "git.stash",
    "docs.present",
  );
  return (
    <section>
      <p>
        Every line below is a finding with its evidence. Inferred lines say so; nothing is guessed
        silently.
      </p>
      {read}
      <ErrorNote error={error} />
      <ul className="findings">
        {rows.map((f) => (
          <FindingRow key={f.id} f={f} />
        ))}
      </ul>
    </section>
  );
}

// ---- 3. Evidence ---------------------------------------------------------------------------

export function StepEvidence({ data, done }: StepProps) {
  const next = (
    <button type="button" className="btn btn-primary" onClick={done}>
      Continue to the plan
    </button>
  );
  const groups: [string, string[]][] = [
    ["Stack", ["stack.manifest", "stack.dependency", "stack.framework", "toolchain.file"]],
    [
      "Unfinished work",
      ["kiro.tasks_progress", "kiro.task_unchecked", "work.todo", "work.skipped_test", "work.area"],
    ],
    [
      "Missing information",
      [
        "env.example_key",
        "sensitive.present",
        "unknown.pause_reason",
        "unknown.builds",
        "unknown.deploy_target",
      ],
    ],
  ];
  return (
    <section>
      {groups.map(([title, rules]) => (
        <div key={title}>
          <h2>{title}</h2>
          <ul className="findings">
            {byRule(data.evidence, ...rules).map((f) => (
              <FindingRow key={f.id} f={f} />
            ))}
          </ul>
        </div>
      ))}
      <p className="muted">
        <code className="code">.env</code> was never opened.{" "}
        <code className="code">.env.example</code> contributes key names only, never values.
      </p>
      {next}
    </section>
  );
}

// ---- 4. Plan -------------------------------------------------------------------------------

const REASON_WORDS: Record<string, string> = {
  sensitive_env: "Sensitive: environment file, never captured",
  rebuildable: "Rebuildable: tools recreate it",
  data_file: "Data file: needs your decision",
  git_hooks: "Git hooks: never captured",
  project_file: "Project file",
  git_internal: "Git internal",
};

export function StepPlan({ api, data, update, done }: StepProps) {
  const { busy, error, run } = useAction();
  const plan = data.plan;
  return (
    <section>
      <p>
        DORMBIT sorts every file by fixed rules. Sensitive files are locked out; you decide the
        rest.
      </p>
      <button
        type="button"
        className="btn btn-primary"
        disabled={busy || plan !== undefined}
        onClick={() => {
          void run(() => {
            update({ plan: api.planCreate("full") });
            done();
          });
        }}
      >
        Build the plan
      </button>
      <ErrorNote error={error} />
      {plan && (
        <>
          <table className="table">
            <caption>Groups</caption>
            <thead>
              <tr>
                <th scope="col">Group</th>
                <th scope="col">Items</th>
                <th scope="col">Size</th>
              </tr>
            </thead>
            <tbody>
              {plan.groups.map((g) => (
                <tr key={g.name}>
                  <td>{g.name}</td>
                  <td>{String(g.file_count)}</td>
                  <td>{bytes(g.bytes)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <h2>Items that aren&apos;t routine</h2>
          <ul className="items">
            {plan.items
              .filter((i) => i.reason !== "project_file" && i.reason !== "git_internal")
              .map((i) => (
                <li key={i.path}>
                  <span className="mono">{i.path}</span> · {REASON_WORDS[i.reason] ?? i.reason} ·{" "}
                  <strong>{i.decision}</strong>
                  {!i.editable && <span className="muted"> (locked)</span>}
                </li>
              ))}
          </ul>
        </>
      )}
    </section>
  );
}

// ---- 5. Decide and approve -----------------------------------------------------------------

export function StepApprove({ api, data, update, done }: StepProps) {
  const { busy, error, run } = useAction();
  const [ticked, setTicked] = useState<ReadonlySet<string>>(new Set());
  const plan = data.plan;
  if (!plan)
    return (
      <p className="note note-warn">
        <span aria-hidden="true">!</span> Build the plan in step 4 first.
      </p>
    );
  const undecided = plan.items.filter((i) => i.decision === "undecided");
  // Every included item with a warning needs the user's own tick; nothing is acknowledged
  // on their behalf.
  const warned = plan.items
    .filter((i) => i.warning !== null && i.decision === "include")
    .map((i) => ({
      path: i.path,
      warning: i.warning ?? "",
      ack: `${i.path}#${(i.warning ?? "").split(":")[0] ?? ""}`,
    }));
  const unacked = warned.filter((w) => !ticked.has(w.ack));
  const acks = warned.filter((w) => ticked.has(w.ack)).map((w) => w.ack);
  return (
    <section>
      {plan.items
        .filter(
          (i) =>
            i.editable && (i.reason === "data_file" || i.reason === "gitignored_not_rebuildable"),
        )
        .map((i) => (
          <fieldset key={i.path} className="decide">
            <legend className="mono">{i.path}</legend>
            {(["include", "exclude"] as const).map((d) => (
              <label key={d} className="pick">
                <input
                  type="radio"
                  name={i.path}
                  checked={i.decision === d}
                  onChange={() => {
                    void run(() => {
                      update({ plan: api.planSetDecision(i.path, d), digest: undefined });
                    });
                  }}
                />
                {d === "include" ? "Include in the capsule" : "Leave it out"}
              </label>
            ))}
          </fieldset>
        ))}
      {warned.map((w) => (
        <label key={w.ack} className="pick note note-warn">
          <input
            type="checkbox"
            checked={ticked.has(w.ack)}
            onChange={(e) => {
              const on = e.target.checked;
              setTicked((s) => {
                const n = new Set(s);
                if (on) n.add(w.ack);
                else n.delete(w.ack);
                return n;
              });
            }}
          />
          <span>
            <span aria-hidden="true">!</span> <span className="mono">{w.path}</span>: {w.warning}. I
            want it in the capsule anyway.
          </span>
        </label>
      ))}
      {undecided.length > 0 && (
        <p className="note note-warn">
          <span aria-hidden="true">!</span> Approve stays off until every item has a decision:{" "}
          {undecided.map((u) => u.path).join(", ")}.
        </p>
      )}
      <button
        type="button"
        className="btn btn-primary"
        disabled={undecided.length > 0 || unacked.length > 0 || busy}
        onClick={() => {
          void run(() => {
            const ok = api.planApprove(plan.digest, acks);
            update({ digest: ok.digest });
            done();
          });
        }}
      >
        Approve this plan
      </button>
      <ErrorNote error={error} />
      {data.digest && (
        <p className="note note-ok">
          <span aria-hidden="true">✓</span> Approved. Plan digest{" "}
          <code className="code">{data.digest}</code>. Any change to the plan now changes this
          digest and needs a new approval.
        </p>
      )}
    </section>
  );
}

// ---- 6. Capture ----------------------------------------------------------------------------

export function StepCapture({ api, data, update, done, reducedMotion }: StepProps) {
  const { busy, error, run } = useAction();
  return (
    <section>
      <p>
        The capture reads every approved file twice (hashing, then rechecking) before writing
        anything final.
      </p>
      <button
        type="button"
        className="btn btn-primary"
        disabled={busy || data.capture !== undefined}
        onClick={() => {
          void run(() => {
            update({ capture: api.capture() });
            done();
          });
        }}
      >
        Make the capsule
      </button>
      <ErrorNote error={error} />
      {data.capture && (
        <>
          <Phases events={data.capture.events} reducedMotion={reducedMotion} />
          <dl className="facts">
            <dt>Capsule</dt>
            <dd className="mono">{data.capture.record.file_name}</dd>
            <dt>Size</dt>
            <dd>{bytes(data.capture.record.size)}</dd>
            <dt>Files</dt>
            <dd>
              {String(data.capture.manifest.totals.files)} files,{" "}
              {String(data.capture.manifest.totals.excluded)} items left out
            </dd>
            <dt>SHA-256</dt>
            <dd className="mono wrap">{data.capture.record.file_sha256}</dd>
          </dl>
        </>
      )}
    </section>
  );
}

// ---- 7. Verify -----------------------------------------------------------------------------

const CHECK: Record<VerifyReport["checks"][number]["status"], { glyph: string; word: string }> = {
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

export function StepVerify({ api, data, update, done }: StepProps) {
  const { busy, error, run } = useAction();
  return (
    <section>
      <p>
        A separate verifier re-reads the capsule from the virtual disk and checks it rule by rule.
      </p>
      <button
        type="button"
        className="btn btn-primary"
        disabled={busy}
        onClick={() => {
          void run(() => {
            const v = api.verify();
            update({ verify: v });
            if (v.status === "verified") done();
          });
        }}
      >
        Verify the capsule
      </button>
      <ErrorNote error={error} />
      {data.verify && <VerifyView report={data.verify} />}
    </section>
  );
}

// ---- 8. Restore ----------------------------------------------------------------------------

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

function readDestination(api: DemoApi): Destination | { error: string } {
  try {
    return api.destination();
  } catch (e) {
    return { error: errorText(e) };
  }
}

export function StepRestore({ api, data, update, done }: StepProps) {
  const { busy, error, run } = useAction();
  const [name, setName] = useState("dockline-invoices");
  // Read once when the step opens: what ~/Restores held before this restore.
  const [dest] = useState(() => readDestination(api));
  return (
    <section>
      <p>
        Restore writes into a hidden staging folder, verifies it, then commits with one exclusive
        rename that refuses to replace anything.
      </p>
      {"error" in dest ? (
        <ErrorNote error={dest.error} />
      ) : (
        <p>
          Destination <span className="mono">{dest.root_display}</span> already contains:{" "}
          {dest.entries.map((e) => (
            <code key={e.name} className="code">
              {e.name}
            </code>
          ))}
        </p>
      )}
      <label className="field">
        New folder name
        <input
          className="input mono"
          value={name}
          onChange={(e) => {
            setName(e.target.value);
          }}
        />
      </label>
      <button
        type="button"
        className="btn btn-primary"
        disabled={busy || name.trim() === ""}
        onClick={() => {
          void run(() => {
            const r = api.restore(name.trim());
            if (r.refused) {
              update({ refused: r });
            } else {
              update({ refused: undefined, restore: r.report });
              if (r.report.status === "verified") done();
            }
          });
        }}
      >
        Restore
      </button>
      <ErrorNote error={error} />
      {data.refused?.refused === true && (
        <div className="note note-warn" role="status">
          <span aria-hidden="true">!</span> Refused:{" "}
          <span className="mono">{data.refused.code}</span>.{" "}
          {data.refused.precheck.checks.find((c) => c.status === "fail")?.detail} Nothing was
          written. Try a new name, such as <code className="code">dockline-restored</code>.
        </div>
      )}
      {data.restore && <RestoreView report={data.restore} />}
    </section>
  );
}

// ---- 9. Independent check ------------------------------------------------------------------

export function StepRehash({ api, data, update, done }: StepProps) {
  const { busy, error, run } = useAction();
  const results = data.rehash;
  const okCount = results?.filter((r) => r.ok).length ?? 0;
  return (
    <section>
      <p>
        Your browser re-hashes every restored file with WebCrypto, a SHA-256 implementation separate
        from the WebAssembly core, and compares each with the capsule manifest.
      </p>
      <button
        type="button"
        className="btn btn-primary"
        disabled={busy}
        onClick={() => {
          void run(async () => {
            const list = api.restoredFiles();
            const out: { path: string; ok: boolean; present: boolean }[] = [];
            for (const f of list.files) {
              if (!f.present) {
                out.push({ path: f.path, ok: false, present: false });
                continue;
              }
              const h = await sha256Hex(api.restoredFileBytes(f.path));
              out.push({ path: f.path, ok: h === f.expected_sha256, present: true });
            }
            update({ rehash: out });
            if (out.length > 0 && out.every((r) => r.ok)) done();
          });
        }}
      >
        Re-hash every restored file
      </button>
      <ErrorNote error={error} />
      {results && (
        <>
          <p
            className={`note ${okCount === results.length ? "note-ok" : "note-fail"}`}
            role="status"
          >
            <span aria-hidden="true">{okCount === results.length ? "✓" : "✕"}</span>{" "}
            {String(okCount)} of {String(results.length)} files match the manifest.
          </p>
          <dl className="facts">
            <dt>Git HEAD and refs</dt>
            <dd>{data.restore?.git_check.refs.kind.replaceAll("_", " ") ?? "unknown"}</dd>
            <dt>Working-tree status</dt>
            <dd>Not run in the browser. The desktop app checks it natively with libgit2.</dd>
          </dl>
        </>
      )}
    </section>
  );
}

// ---- 10. Brief -----------------------------------------------------------------------------

function download(name: string, data: BlobPart, type: string) {
  const url = URL.createObjectURL(new Blob([data], { type }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.hidden = true;
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => {
    URL.revokeObjectURL(url);
  }, 1000);
}

export function StepBrief({ api, data, update, done }: StepProps) {
  const { busy, error, run } = useAction();
  const b = data.brief?.brief;
  return (
    <section>
      <p>
        Every line cites the finding it comes from. The four outcomes are always reported
        separately.
      </p>
      <button
        type="button"
        className="btn btn-primary"
        disabled={busy}
        onClick={() => {
          void run(() => {
            update({ brief: api.brief() });
            done();
          });
        }}
      >
        Write the brief
      </button>
      <ErrorNote error={error} />
      {b && data.brief && (
        <>
          <h2>Outcomes</h2>
          <ul className="outcomes">
            {[b.outcomes.files, b.outcomes.context, b.outcomes.session, b.outcomes.build].map(
              (l, i) => (
                <li key={i}>
                  <CitedLine line={l} />
                </li>
              ),
            )}
          </ul>
          {b.sections
            .filter((s) => s.lines.length > 0)
            .map((s) => (
              <div key={s.id}>
                <h2>{s.title}</h2>
                <ul className="brief-lines">
                  {s.lines.map((l, i) => (
                    <li key={i}>
                      <CitedLine line={l} />
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          <div className="row">
            <button
              type="button"
              className="btn"
              onClick={() => {
                download("RESURRECTION-BRIEF.md", data.brief?.markdown ?? "", "text/markdown");
              }}
            >
              Download the brief (.md)
            </button>
            <button
              type="button"
              className="btn"
              onClick={() => {
                const bytesOut = api.capsuleBytes();
                download(
                  data.capture?.record.file_name ?? "capsule.dormbit",
                  new Uint8Array(bytesOut),
                  "application/octet-stream",
                );
              }}
            >
              {data.change ? "Download the capsule (damaged in step 11)" : "Download the capsule"}
            </button>
          </div>
        </>
      )}
    </section>
  );
}

// ---- 11-12. Corruption controls ------------------------------------------------------------

const DAMAGED_FILE = "src/pdf/renderInvoice.ts";

const CONTROLS = [
  [
    "stored_file",
    "Damage a stored file",
    `Changes one byte inside the stored copy of ${DAMAGED_FILE}.`,
  ],
  [
    "capsule_bytes",
    "Damage capsule bytes",
    "Changes one byte outside file data: a header, the manifest, padding, or the trailer.",
  ],
  [
    "after_verification",
    "Damage after verification",
    `Changes one byte of ${DAMAGED_FILE} after the restore's pre-checks pass.`,
  ],
] as const;

export function StepDamage({ api, data, update, done }: StepProps) {
  const { busy, error, run } = useAction();
  return (
    <section>
      <p>
        These controls change real bytes of the capsule in browser memory. They simulate damage to
        virtual bytes; no file on your computer is touched.
      </p>
      <div className="controls">
        {CONTROLS.map(([id, title, help]) => (
          <button
            key={id}
            type="button"
            className="btn control"
            disabled={busy || data.change !== undefined}
            onClick={() => {
              void run(() => {
                if (id === "after_verification" && data.verify?.status !== "verified") {
                  update({ verify: api.verify() });
                }
                const { seed, xor } = randomControl();
                update({
                  change: {
                    control: id,
                    change: api.corrupt(id, seed, xor, id === "capsule_bytes" ? "" : DAMAGED_FILE),
                  },
                  caught: undefined,
                });
                done();
              });
            }}
          >
            <strong>{title}</strong>
            <span className="muted">{help}</span>
          </button>
        ))}
      </div>
      <ErrorNote error={error} />
      {data.change && (
        <p className="note note-warn" role="status">
          <span aria-hidden="true">!</span> {data.change.control.replaceAll("_", " ")}: byte{" "}
          <span className="mono">{String(data.change.change.capsule_offset)}</span> in{" "}
          <span className="mono">{data.change.change.region}</span> changed from{" "}
          <span className="mono">0x{data.change.change.old.toString(16).padStart(2, "0")}</span> to{" "}
          <span className="mono">0x{data.change.change.new.toString(16).padStart(2, "0")}</span>.
        </p>
      )}
    </section>
  );
}

const AFTER_DAMAGE = "dockline-after-damage";

export function StepCaught({ api, data, update, done }: StepProps) {
  const { busy, error, run } = useAction();
  const control = data.change?.control;
  return (
    <section>
      <button
        type="button"
        className="btn btn-primary"
        disabled={busy || control === undefined}
        onClick={() => {
          void run(() => {
            if (control === "after_verification") {
              const r = api.restore(AFTER_DAMAGE);
              update({ caught: { restore: r, dest: api.destination() } });
            } else {
              const v = api.verify();
              update({ caught: { verify: v }, verify: v });
            }
            done();
          });
        }}
      >
        {control === "after_verification" ? "Run the restore" : "Verify the capsule again"}
      </button>
      <ErrorNote error={error} />
      {data.caught?.verify && <VerifyView report={data.caught.verify} />}
      {data.caught?.restore?.refused === false && (
        <>
          <RestoreView report={data.caught.restore.report} />
          {data.caught.dest && (
            <p className="muted">
              <span className="mono">{data.caught.dest.root_display}</span> now contains:{" "}
              {data.caught.dest.entries.map((e) => e.name).join(", ")}.{" "}
              {data.caught.dest.entries.some((e) => e.name === AFTER_DAMAGE)
                ? `${AFTER_DAMAGE} exists.`
                : `No ${AFTER_DAMAGE} folder was created.`}
            </p>
          )}
        </>
      )}
      {data.caught?.restore?.refused === true && (
        <p className="note note-fail" role="status">
          <span aria-hidden="true">✕</span> Refused before writing:{" "}
          <span className="mono">{data.caught.restore.code}</span>
        </p>
      )}
      <p className="muted">Use Reset to put everything back and try another control.</p>
    </section>
  );
}

export const STEP_VIEWS = [
  StepFind,
  StepOverview,
  StepEvidence,
  StepPlan,
  StepApprove,
  StepCapture,
  StepVerify,
  StepRestore,
  StepRehash,
  StepBrief,
  StepDamage,
  StepCaught,
] as const;

export { Spans };
