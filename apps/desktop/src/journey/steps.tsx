// The desktop journey's steps over real folders. Each calls one native command and shows
// exactly what the core returned.
import { useState } from "react";
import type {
  CaptureOutput,
  CapturePlan,
  CoreBrief,
  CoreRestoreReport,
  DiscoverResult,
  EvidenceResult,
  RestoreOutcome,
  VerifyReport,
} from "@dormbit/schema";
import {
  CitedLine,
  ErrorNote,
  FindingRow,
  Phases,
  REASON_WORDS,
  RestoreView,
  VerifyView,
  byRule,
  bytes,
  useAction,
} from "@dormbit/ui";
import type { NativeApi, ReadGrant, TargetGrant } from "./api";

export interface JourneyData {
  grant?: ReadGrant | undefined;
  discover?: DiscoverResult | undefined;
  projectId?: string | undefined;
  evidence?: EvidenceResult | undefined;
  plan?: CapturePlan | undefined;
  digest?: string | undefined;
  capture?: CaptureOutput | undefined;
  verify?: VerifyReport | undefined;
  target?: TargetGrant | undefined;
  refused?: RestoreOutcome | undefined;
  restore?: CoreRestoreReport | undefined;
  brief?: { brief: CoreBrief; markdown: string } | undefined;
  saved?: string | undefined;
}

export interface StepProps {
  api: NativeApi;
  data: JourneyData;
  update: (patch: Partial<JourneyData>) => void;
  done: () => void;
  reducedMotion: boolean;
}

export const STEPS = [
  "Choose a folder",
  "Evidence",
  "Preservation plan",
  "Decide and approve",
  "Make the capsule",
  "Verify the capsule",
  "Restore into a new folder",
  "Resurrection Brief",
] as const;

function project(data: JourneyData) {
  return data.discover?.result.projects.find((p) => p.id === data.projectId);
}

// ---- 1. Choose a folder ------------------------------------------------------------------

export function StepFolder({ api, data, update, done }: StepProps) {
  const { busy, error, run } = useAction();
  const [cancelled, setCancelled] = useState(false);
  return (
    <section>
      <p>
        Grant DORMBIT one folder to look in. macOS shows its own folder panel, and DORMBIT gets
        access to that folder only. It never follows symlinks and never runs your code.
      </p>
      <button
        type="button"
        className="btn btn-primary"
        disabled={busy}
        onClick={() => {
          void run(async () => {
            const g = await api.scopePick();
            setCancelled(g === null);
            if (g === null) return;
            update({ grant: g, discover: await api.discover(g.id), projectId: undefined });
          });
        }}
      >
        Choose a folder…
      </button>
      <ErrorNote error={error} />
      {cancelled && <p className="muted">No folder chosen. Nothing was read.</p>}
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
                  <dd className="mono">
                    {data.discover?.root_display}/{p.rel_root}
                  </dd>
                  <dt>Latest file change</dt>
                  <dd>
                    {p.latest_mtime?.slice(0, 10) ?? "unknown"}
                    {p.idle_days !== null && ` (${String(p.idle_days)} days ago)`}
                  </dd>
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
                <button
                  type="button"
                  className="btn"
                  onClick={() => {
                    update({ projectId: p.id, evidence: undefined, plan: undefined });
                    done();
                  }}
                >
                  Open {p.name}
                </button>
              </li>
            ))}
          </ul>
          {data.discover.result.projects.length === 0 && (
            <p className="note note-warn">
              <span aria-hidden="true">!</span> No project found here. A project has a manifest (for
              example <code className="code">package.json</code>) or a Git folder.
            </p>
          )}
        </div>
      )}
    </section>
  );
}

// ---- 2. Evidence -------------------------------------------------------------------------

const GROUPS: [string, string[]][] = [
  [
    "Where you stopped",
    [
      "identity.name",
      "git.branch",
      "git.feature_branch",
      "git.wip_commit",
      "git.uncommitted",
      "git.stash",
      "docs.present",
    ],
  ],
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

export function StepEvidence({ api, data, update, done }: StepProps) {
  const { busy, error, run } = useAction();
  const p = project(data);
  return (
    <section>
      <p>
        Every line below is a finding with its evidence. Inferred lines say so; what DORMBIT
        can&apos;t know stays unknown. Git is read with libgit2; the git command never runs.
      </p>
      <button
        type="button"
        className="btn btn-primary"
        disabled={busy || p === undefined}
        onClick={() => {
          void run(async () => {
            if (p === undefined) return;
            update({ evidence: await api.evidence(p.id) });
            done();
          });
        }}
      >
        Read the project
      </button>
      <ErrorNote error={error} />
      {data.evidence &&
        GROUPS.map(([title, rules]) => (
          <div key={title}>
            <h2>{title}</h2>
            <ul className="findings">
              {byRule(data.evidence, ...rules).map((f) => (
                <FindingRow key={f.id} f={f} />
              ))}
            </ul>
          </div>
        ))}
    </section>
  );
}

// ---- 3. Plan -----------------------------------------------------------------------------

export function StepPlan({ api, data, update, done }: StepProps) {
  const { busy, error, run } = useAction();
  const plan = data.plan;
  return (
    <section>
      <p>
        DORMBIT sorts every file by fixed rules. Secret files are locked out and never read; you
        decide the rest.
      </p>
      <button
        type="button"
        className="btn btn-primary"
        disabled={busy || data.projectId === undefined}
        onClick={() => {
          void run(async () => {
            if (data.projectId === undefined) return;
            update({ plan: await api.planCreate(data.projectId), digest: undefined });
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

// ---- 4. Decide and approve ---------------------------------------------------------------

export function StepApprove({ api, data, update, done }: StepProps) {
  const { busy, error, run } = useAction();
  const [ticked, setTicked] = useState<ReadonlySet<string>>(new Set());
  const plan = data.plan;
  if (!plan)
    return (
      <p className="note note-warn">
        <span aria-hidden="true">!</span> Build the plan in step 3 first.
      </p>
    );
  const undecided = plan.items.filter((i) => i.decision === "undecided");
  // Every included item with a warning needs the user's own tick.
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
                  value={d}
                  checked={i.decision === d}
                  onChange={() => {
                    void run(async () => {
                      update({
                        plan: await api.planSetDecision(plan.plan_id, i.path, d),
                        digest: undefined,
                      });
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
          void run(async () => {
            const ok = await api.planApprove(plan.plan_id, plan.digest, acks);
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
          <code className="code">{data.digest}</code>. Any change to the plan changes this digest
          and needs a new approval.
        </p>
      )}
    </section>
  );
}

// ---- 5. Capture --------------------------------------------------------------------------

export function StepCapture({ api, data, update, done, reducedMotion }: StepProps) {
  const { busy, error, run } = useAction();
  return (
    <section>
      <p>
        The capture reads every approved file twice (hashing, then rechecking) before it writes
        anything final, into DORMBIT&apos;s own library folder. Your project folder is never
        changed.
      </p>
      <button
        type="button"
        className="btn btn-primary"
        disabled={busy || data.digest === undefined || data.capture !== undefined}
        onClick={() => {
          void run(async () => {
            if (!data.plan) return;
            update({ capture: await api.capture(data.plan.plan_id), verify: undefined });
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
            <dt>Format</dt>
            <dd>A standard tar archive with a JSON manifest; any tar tool can list it.</dd>
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

// ---- 6. Verify ---------------------------------------------------------------------------

export function StepVerify({ api, data, update, done }: StepProps) {
  const { busy, error, run } = useAction();
  return (
    <section>
      <p>A separate verifier re-reads the capsule from disk and checks it rule by rule.</p>
      <button
        type="button"
        className="btn btn-primary"
        disabled={busy || data.capture === undefined}
        onClick={() => {
          void run(async () => {
            if (!data.capture) return;
            const v = await api.verify(data.capture.record.capsule_id);
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

// ---- 7. Restore --------------------------------------------------------------------------

export function StepRestore({ api, data, update, done }: StepProps) {
  const { busy, error, run } = useAction();
  const p = project(data);
  const [name, setName] = useState(p?.rel_root.split("/").pop() ?? "restored-project");
  return (
    <section>
      <p>
        Restore never overwrites anything. It stages every file in a hidden folder, verifies them,
        then commits with one exclusive rename that macOS refuses if the name is taken.
      </p>
      <button
        type="button"
        className="btn"
        disabled={busy}
        onClick={() => {
          void run(async () => {
            const t = await api.restoreTargetPick();
            if (t !== null) update({ target: t, refused: undefined });
          });
        }}
      >
        Choose where to restore…
      </button>
      {data.target && (
        <p>
          Destination <span className="mono">{data.target.root_display}</span>
          {data.target.entries.length > 0 ? " already contains: " : " is empty."}
          {data.target.entries.map((e) => (
            <code key={e.name} className="code">
              {e.name}
            </code>
          ))}
        </p>
      )}
      <label className="field">
        New folder name
        <input
          id="dest-name"
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
        disabled={busy || data.target === undefined || name.trim() === ""}
        onClick={() => {
          void run(async () => {
            const r = await api.restore(name.trim());
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
          written. Choose a new name.
        </div>
      )}
      {data.restore && <RestoreView report={data.restore} />}
    </section>
  );
}

// ---- 8. Brief ----------------------------------------------------------------------------

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
          void run(async () => {
            update({ brief: await api.brief() });
            done();
          });
        }}
      >
        Write the brief
      </button>
      <ErrorNote error={error} />
      {b && (
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
              disabled={busy}
              onClick={() => {
                void run(async () => {
                  const r = await api.briefExport();
                  if (r !== null) update({ saved: r.file_name });
                });
              }}
            >
              Save the brief…
            </button>
          </div>
          {data.saved !== undefined && (
            <p className="note note-ok" role="status">
              <span aria-hidden="true">✓</span> Saved as <span className="mono">{data.saved}</span>.
            </p>
          )}
        </>
      )}
    </section>
  );
}

export const STEP_VIEWS = [
  StepFolder,
  StepEvidence,
  StepPlan,
  StepApprove,
  StepCapture,
  StepVerify,
  StepRestore,
  StepBrief,
] as const;
