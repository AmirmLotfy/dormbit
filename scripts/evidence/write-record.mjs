// Writes one Evidence record for scripts/evidence/run.sh (foundation Requirement 17).
// Values arrive as arguments and the JSON is built with JSON.stringify, so nothing from the
// shell is spliced into JSON text. Also redacts the home folder in the raw log.
import { readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { isAbsolute, relative } from "node:path";

const argv = process.argv.slice(2);
const sep = argv.indexOf("--");
const opts = {};
for (let i = 0; i < (sep < 0 ? argv.length : sep); i += 2) {
  opts[argv[i].replace(/^--/, "")] = argv[i + 1] ?? "";
}
const command = sep < 0 ? [] : argv.slice(sep + 1);

const home = process.env.HOME || homedir();
/** Fixed-string replacement of the home folder with `~` (never a regex built from the path). */
const redact = (s) => (home && home !== "/" ? String(s).split(home).join("~") : String(s));

const raw = readFileSync(opts.log, "utf8");
const clean = redact(raw);
if (clean !== raw) writeFileSync(opts.log, clean);
// Strip ANSI color codes before parsing. Built from a string so no control character sits in a regex literal.
const ansi = new RegExp(`${String.fromCharCode(27)}\\[[0-9;?]*[A-Za-z]`, "g");
const text = clean.replace(ansi, "");
const lines = text.split(/\r?\n/);

const counts = { passed: 0, failed: 0, skipped: 0 };
const sources = [];
const num = (re, s) => {
  const m = re.exec(s);
  return m ? Number(m[1]) : 0;
};

// cargo: "test result: ok. N passed; M failed; K ignored"
let found = false;
for (const m of text.matchAll(/test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored/g)) {
  counts.passed += Number(m[1]);
  counts.failed += Number(m[2]);
  counts.skipped += Number(m[3]);
  found = true;
}
if (found) sources.push("cargo");

// Vitest: "Tests  N passed | M failed | K skipped (T)", any subset
found = false;
for (const line of lines) {
  const m = /^\s*Tests\s+(.*\d+ (?:passed|failed|skipped|todo).*)$/.exec(line);
  if (!m) continue;
  counts.passed += num(/(\d+) passed/, m[1]);
  counts.failed += num(/(\d+) failed/, m[1]);
  counts.skipped += num(/(\d+) skipped/, m[1]) + num(/(\d+) todo/, m[1]);
  found = true;
}
if (found) sources.push("vitest");

// Playwright final summary: "  N passed (12.3s)", "  M failed", "  K skipped", "  F flaky"
found = false;
for (const line of lines) {
  const m = /^\s*(\d+) (passed|failed|skipped|flaky)\b/.exec(line);
  if (!m) continue;
  const n = Number(m[1]);
  if (m[2] === "failed") counts.failed += n;
  else if (m[2] === "skipped") counts.skipped += n;
  else counts.passed += n;
  found = true;
}
if (found) sources.push("playwright");

const iso = (ms) => new Date(ms).toISOString().replace(/\.\d{3}Z$/, "Z");
const started = Number(opts.started);
const finished = Number(opts.finished);
const rel = relative(opts.root, opts.log);
const env = {};
for (const k of ["PROPTEST_CASES", "FC_NUM_RUNS"]) {
  if (process.env[k] !== undefined) env[k] = redact(process.env[k]);
}

const record = {
  schema_version: 1,
  task: process.env.EVIDENCE_TASK === undefined ? null : redact(process.env.EVIDENCE_TASK),
  command: command.map(redact),
  cwd: redact(opts.cwd),
  started_at: iso(started),
  finished_at: iso(finished),
  duration_ms: finished - started,
  exit_code: Number(opts.exit),
  counts: sources.length > 0 ? counts : null,
  counts_source: sources.length > 0 ? sources.join("+") : "unknown",
  env,
  git: { sha: redact(opts.sha), dirty: opts.dirty === "true" },
  host: { os: redact(opts.os), arch: redact(opts.arch) },
  log: rel.startsWith("..") || isAbsolute(rel) ? redact(opts.log) : rel,
};

writeFileSync(opts.record, `${JSON.stringify(record, null, 2)}\n`);
