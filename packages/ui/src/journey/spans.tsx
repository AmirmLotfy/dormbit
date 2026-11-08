// Renders statements and evidence as text nodes only. Untrusted text (quotes, code, paths)
// never becomes markup: there is no raw HTML anywhere in the demo.
import type { BriefLine, CoreEvidenceRef, CoreFinding, Span } from "@dormbit/schema";

export function Spans({ spans }: { spans: readonly Span[] }) {
  return (
    <>
      {spans.map((s, i) => {
        switch (s.t) {
          case "Text":
            return <span key={i}>{s.v}</span>;
          case "Quote":
            return (
              <q key={i} className="quote">
                {s.v}
              </q>
            );
          case "Code":
            return (
              <code key={i} className="code">
                {s.v}
              </code>
            );
          case "Count":
            return (
              <strong key={i} className="count">
                {s.v}
              </strong>
            );
        }
      })}
    </>
  );
}

export function describeRef(r: CoreEvidenceRef): string {
  switch (r.kind) {
    case "file_lines":
      return r.start_line === r.end_line
        ? `${r.path} line ${String(r.start_line)}`
        : `${r.path} lines ${String(r.start_line)}–${String(r.end_line)}`;
    case "manifest_key":
      return `${r.path} ${r.pointer}`;
    case "file_stat":
      return `${r.path === "" ? "project folder" : r.path} (${r.stat === "absent" ? "absent" : "exists"})`;
    case "git_ref":
      return `Git ref ${r.name}`;
    case "git_commit":
      return `Git commit ${r.oid.slice(0, 12)}`;
    case "git_status":
      return "Git status at scan time";
    case "git_config_key":
      return `Git config key ${r.key}`;
    case "agent_session":
      return `${r.adapter} session ${r.session_id} (metadata only)`;
    case "capsule":
      return `capsule ${r.capsule_id} manifest.json ${r.pointer}`;
    case "restore":
      return `restore ${r.op_id} report`;
    case "rule_limit":
      return `rule ${r.rule} limit ${String(r.limit)}`;
  }
}

const STATUS: Record<CoreFinding["status"], { glyph: string; word: string }> = {
  confirmed: { glyph: "●", word: "Confirmed" },
  inferred: { glyph: "◐", word: "Inferred" },
  unknown: { glyph: "○", word: "Unknown" },
  unsupported: { glyph: "⊘", word: "Unsupported" },
};

export function StatusTag({ status }: { status: CoreFinding["status"] }) {
  const s = STATUS[status];
  return (
    <span className={`tag tag-${status}`}>
      <span aria-hidden="true">{s.glyph}</span> {s.word}
    </span>
  );
}

export function FindingRow({ f }: { f: CoreFinding }) {
  return (
    <li className="finding">
      <div className="finding-head">
        <StatusTag status={f.status} />
        <span className="finding-id mono">{f.id}</span>
      </div>
      <p className="finding-text">
        <Spans spans={f.statement} />
      </p>
      <ul className="refs" aria-label="Evidence">
        {f.evidence.map((r, i) => (
          <li key={i} className="mono">
            {describeRef(r)}
          </li>
        ))}
      </ul>
    </li>
  );
}

export function CitedLine({ line }: { line: BriefLine }) {
  return (
    <>
      <Spans spans={line.spans} />
      {line.cites.map((c) => (
        <span key={c} className="cite mono">
          {" "}
          [{c}]
        </span>
      ))}
    </>
  );
}

export function bytes(n: number): string {
  if (n < 1024) return `${String(n)} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}
