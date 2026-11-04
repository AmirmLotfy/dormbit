// Check_Config (foundation Requirements 20.9, 21.10, 23.8): verifies the Kiro setup in .kiro/
// against the foundation spec. Exits 0 when everything holds; otherwise prints each failure
// and exits 1.
//
//   node scripts/kiro/check-config.mjs [repo-root]
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

const root = process.argv[2] ?? new URL("../..", import.meta.url).pathname;
const failures = [];
const fail = (msg) => failures.push(msg);
const read = (rel) => readFileSync(join(root, rel), "utf8");
const has = (rel) => existsSync(join(root, rel));

/** Front matter with a small YAML subset: scalars, flow lists, block lists, and nested maps. */
function frontMatter(text) {
  const m = /^---\n([\s\S]*?)\n---\n?/.exec(text);
  if (!m) return null;
  const lines = m[1].split("\n");
  const scalar = (v) => {
    v = v.trim();
    if (v.startsWith("[") && v.endsWith("]")) return JSON.parse(v);
    if (v.startsWith('"') && v.endsWith('"')) return JSON.parse(v);
    if (v === "true" || v === "false") return v === "true";
    return v;
  };
  function block(start, indent) {
    const obj = {};
    let i = start;
    while (i < lines.length) {
      const line = lines[i];
      if (line.trim() === "") {
        i++;
        continue;
      }
      const ind = line.length - line.trimStart().length;
      if (ind < indent) break;
      const t = line.trim();
      if (t.startsWith("- ")) break;
      const kv = /^([\w-]+):(?:\s(.*))?$/.exec(t);
      if (!kv) {
        i++;
        continue;
      }
      const key = kv[1];
      const rest = kv[2] ?? "";
      if (rest.trim() !== "") {
        obj[key] = scalar(rest);
        i++;
        continue;
      }
      let j = i + 1;
      while (j < lines.length && lines[j].trim() === "") j++;
      const next = lines[j] ?? "";
      const nind = next.length - next.trimStart().length;
      if (nind < indent || (nind === indent && !next.trim().startsWith("- "))) {
        obj[key] = null;
        i++;
        continue;
      }
      if (next.trim().startsWith("- ")) {
        i = j - 1;
        const arr = [];
        i++;
        while (i < lines.length && lines[i].trim().startsWith("- ")) {
          const itemInd = lines[i].length - lines[i].trimStart().length;
          const first = lines[i].trim().slice(2);
          const ikv = /^([\w-]+):(?:\s(.*))?$/.exec(first);
          if (ikv) {
            const item = { [ikv[1]]: scalar(ikv[2] ?? "") };
            i++;
            const [sub, ni] = block(i, itemInd + 2);
            Object.assign(item, sub);
            i = ni;
            arr.push(item);
          } else {
            arr.push(scalar(first));
            i++;
          }
        }
        obj[key] = arr;
      } else {
        const [sub, ni] = block(i + 1, nind);
        obj[key] = sub;
        i = ni;
      }
    }
    return [obj, i];
  }
  return block(0, 0)[0];
}

// ---- Requirement 20: steering --------------------------------------------------------------
const ALWAYS = ["product", "tech", "structure", "security", "definition-of-done", "project-status"];
const FILEMATCH = {
  rust: ["**/*.rs"],
  frontend: ["apps/**", "packages/**"],
  "ui-design": ["packages/ui/**"],
  testing: ["**/*.test.ts", "**/*.test.tsx", "**/tests/**", "e2e/**"],
};
const MANUAL = ["hackathon", "release"];
const steering = {};
for (const name of [...ALWAYS, ...Object.keys(FILEMATCH), ...MANUAL]) {
  const rel = `.kiro/steering/${name}.md`;
  if (!has(rel)) {
    fail(`steering: ${rel} is missing`);
    continue;
  }
  const text = read(rel);
  steering[name] = text;
  const fm = frontMatter(text);
  const want = ALWAYS.includes(name) ? "always" : MANUAL.includes(name) ? "manual" : "fileMatch";
  if (!fm || fm.inclusion !== want)
    fail(`steering: ${rel} must have inclusion: ${want} (found ${fm?.inclusion ?? "none"})`);
  if (want === "fileMatch") {
    const pats = [].concat(fm?.fileMatchPattern ?? []);
    for (const p of FILEMATCH[name])
      if (!pats.includes(p)) fail(`steering: ${rel} fileMatchPattern lacks "${p}"`);
  }
}
if (
  steering["project-status"] &&
  !steering["project-status"].includes("#[[file:docs/status/HANDOFF.md]]")
) {
  fail("steering: project-status.md must include #[[file:docs/status/HANDOFF.md]]");
}
const sec = steering.security ?? "";
for (const [what, re] of [
  ["no std::fs in core", /std::fs/],
  ["no git CLI", /git CLI/i],
  ["no fs or shell plugins", /plugin/i],
  ["no raw HTML", /raw HTML/i],
  ["no unwrap in core", /unwrap/],
  ["never read real agent stores", /real agent stores/i],
  ["delegate to rust-security", /rust-security/],
])
  if (!re.test(sec)) fail(`steering: security.md doesn't state "${what}"`);
const prod = steering.product ?? "";
for (const label of [
  "Files restored and verified.",
  "Context reconstructed (inferred from evidence).",
  "AI session resumed: not performed.",
  "Builds/runs: unknown (never executed).",
])
  if (!prod.includes(label)) fail(`steering: product.md lacks the outcome label "${label}"`);
for (const [what, re] of [
  ["truthfulness", /truthful/i],
  ["no deletion", /no deletion/i],
  ["no code execution", /no code execution/i],
  ["local-first", /local-first/i],
]) {
  if (!re.test(prod)) fail(`steering: product.md doesn't state ${what}`);
}
const dod = steering["definition-of-done"] ?? "";
for (const s of [
  "Planned",
  "In progress",
  "Implemented",
  "Tested",
  "Verified",
  "Released",
  "Submitted",
]) {
  if (!dod.includes(`| ${s} |`)) fail(`steering: definition-of-done.md lacks the status ${s}`);
}
if (!/Evidence_Runner/.test(dod))
  fail("steering: definition-of-done.md doesn't require the Evidence_Runner");
const hack = steering.hackathon ?? "";
for (const gate of [
  "installs outside the repo",
  "writes outside the workspace",
  "the repo target",
  "the release",
  "Screen Recording",
  "Chrome extension",
  "making the video public",
  "final Devpost submit",
]) {
  if (!hack.includes(gate)) fail(`steering: hackathon.md lacks the approval gate "${gate}"`);
}

// ---- Requirement 21: custom agents ---------------------------------------------------------
const TESTS = [
  "**/tests/**",
  "**/*.test.ts",
  "**/*.test.tsx",
  "**/*.test-d.ts",
  "**/*.spec.ts",
  "**/proptest-regressions/**",
];
const AGENTS = {
  architect: {
    steering: [],
    skills: [],
    write: ["docs/adr/**", "docs/architecture/**", ".kiro/specs/**"],
    shell: [],
  },
  "rust-security": {
    steering: ["rust", "testing"],
    skills: [],
    write: ["crates/**", "fixtures/malicious/**", "docs/security/**"],
    shell: ["cargo *"],
  },
  frontend: {
    steering: ["frontend", "ui-design", "testing"],
    skills: ["visual-qa"],
    write: ["apps/**", "packages/**"],
    shell: ["pnpm *"],
  },
  qa: {
    steering: ["rust", "frontend", "testing"],
    skills: ["visual-qa", "release-gate"],
    write: [...TESTS, "e2e/**", "docs/evidence/**"],
    shell: ["cargo test *", "pnpm test*"],
  },
  release: {
    steering: ["release", "hackathon"],
    skills: ["release-gate", "demo-video"],
    write: [".github/**", "docs/release/**", "docs/submission/**", "media/**", "README.md"],
    shell: ["cargo build *", "codesign *"],
    ask: ["gh *"],
  },
  reviewer: {
    steering: ["rust", "frontend", "testing"],
    skills: [],
    write: null,
    shell: ["cargo test *", "pnpm test*"],
  },
};
const sameSet = (a, b) => a.length === b.length && a.every((x) => b.includes(x));
for (const [name, want] of Object.entries(AGENTS)) {
  const rel = `.kiro/agents/${name}.md`;
  if (!has(rel)) {
    fail(`agent: ${rel} is missing`);
    continue;
  }
  const fm = frontMatter(read(rel));
  if (!fm) {
    fail(`agent ${name}: no front matter`);
    continue;
  }
  if ("model" in fm) fail(`agent ${name}: has a model field`);
  if ("hooks" in fm) fail(`agent ${name}: has a hooks field (the IDE ignores such agents)`);
  const res = fm.resources ?? [];
  for (const s of [...ALWAYS, ...want.steering])
    if (!res.includes(`file://.kiro/steering/${s}.md`))
      fail(`agent ${name}: resources lack steering ${s}.md`);
  for (const k of want.skills)
    if (!res.includes(`skill://${k}`)) fail(`agent ${name}: resources lack skill ${k}`);
  const rules = fm.permissions?.rules ?? [];
  const writeDeny = rules.filter(
    (r) => r.capability === "fs_write" && r.effect === "deny" && sameSet(r.match ?? [], ["**"]),
  );
  if (writeDeny.length !== 1) {
    fail(`agent ${name}: needs exactly one fs_write deny on "**"`);
  } else if (want.write === null) {
    if ((writeDeny[0].exclude ?? []).length > 0)
      fail(`agent ${name}: must have no write permission`);
  } else if (!sameSet(writeDeny[0].exclude ?? [], want.write)) {
    fail(
      `agent ${name}: write scope is [${(writeDeny[0].exclude ?? []).join(", ")}], expected [${want.write.join(", ")}]`,
    );
  }
  if (rules.some((r) => r.capability === "fs_write" && r.effect === "allow" && want.write === null))
    fail(`agent ${name}: must not allow any write`);
  const shellAllow = rules
    .filter((r) => r.capability === "shell" && r.effect === "allow")
    .flatMap((r) => r.match ?? []);
  const shellAsk = rules
    .filter((r) => r.capability === "shell" && r.effect === "ask")
    .flatMap((r) => r.match ?? []);
  const shellDeny = rules.filter(
    (r) => r.capability === "shell" && r.effect === "deny" && (r.match ?? []).includes("*"),
  );
  if (shellDeny.length !== 1) fail(`agent ${name}: needs one shell deny on "*"`);
  for (const c of want.shell)
    if (!shellAllow.includes(c)) fail(`agent ${name}: shell allow lacks "${c}"`);
  if (want.shell.length === 0 && shellAllow.length > 0)
    fail(`agent ${name}: must not allow shell commands`);
  for (const c of want.ask ?? [])
    if (!shellAsk.includes(c)) fail(`agent ${name}: shell ask lacks "${c}"`);
  if (
    name === "frontend" &&
    !rules.some(
      (r) =>
        r.capability === "fs_write" &&
        r.effect === "deny" &&
        (r.match ?? []).includes("packages/schema/generated/**"),
    )
  ) {
    fail("agent frontend: must deny writes to packages/schema/generated/**");
  }
  if (
    rules.some(
      (r) =>
        r.capability === "mcp" &&
        r.effect === "allow" &&
        (r.match ?? []).some((m) => m.startsWith("playwright-chrome")),
    )
  ) {
    fail(`agent ${name}: must not get playwright-chrome tools`);
  }
}
if (has(".kiro/agents/reviewer.md")) {
  const lines = read(".kiro/agents/reviewer.md").trim().split("\n");
  if (!/`APPROVED`.*`NEEDS_CHANGES`/.test(lines.join("\n")))
    fail("agent reviewer: prompt must require a final APPROVED or NEEDS_CHANGES line");
}

// ---- Requirement 22: hooks -----------------------------------------------------------------
const HOOKS = {
  format: {
    trigger: "PostFileSave",
    matcher: "\\.(rs|ts|tsx|css|json|md)$",
    script: "scripts/hooks/format.sh",
  },
  "stop-gate": { trigger: "Stop", script: "scripts/hooks/stop-gate.sh" },
  "secret-scan": { trigger: "PreToolUse", script: "scripts/hooks/secret-scan.sh" },
  "security-tests": {
    trigger: "PostFileSave",
    matcher: "crates/dormbit-(core|fs)/src/(path|scope|capsule|verify|restore|rename)",
    script: "scripts/hooks/security-tests.sh",
  },
  "review-reminder": {
    trigger: "PostTaskExec",
    prompt: "Run the reviewer sub-agent on this task's diff and record the verdict in STATUS.md.",
  },
};
for (const [name, want] of Object.entries(HOOKS)) {
  const rel = `.kiro/hooks/${name}.json`;
  if (!has(rel)) {
    fail(`hook: ${rel} is missing`);
    continue;
  }
  let j;
  try {
    j = JSON.parse(read(rel));
  } catch {
    fail(`hook: ${rel} is not valid JSON`);
    continue;
  }
  const h = j.hooks?.[0];
  if (j.version !== "v1") fail(`hook ${name}: version must be "v1"`);
  if (h?.trigger !== want.trigger) fail(`hook ${name}: trigger must be ${want.trigger}`);
  if (want.matcher && h?.matcher !== want.matcher)
    fail(`hook ${name}: matcher must be ${want.matcher}`);
  if (want.script && !(h?.action?.command ?? "").includes(want.script))
    fail(`hook ${name}: must run ${want.script}`);
  if (want.script && !has(want.script)) fail(`hook ${name}: ${want.script} is missing`);
  if (want.prompt && !(h?.action?.prompt ?? "").includes(want.prompt))
    fail(`hook ${name}: prompt must include "${want.prompt}"`);
}
// task-tests and release-gate are Skills when the task-hook payload can't name the task (22.15).
for (const name of ["task-tests", "release-gate"]) {
  if (!has(`.kiro/hooks/${name}.json`) && !has(`.kiro/skills/${name}/SKILL.md`))
    fail(`${name}: needs a hook or a skill`);
  if (!has(`scripts/hooks/${name}.sh`)) fail(`${name}: scripts/hooks/${name}.sh is missing`);
}
if (
  readdirSync(join(root, ".kiro/hooks")).some(
    (f) => f.startsWith("probe-") && f !== "probe-task-exec.json",
  )
) {
  fail("hooks: leftover probe hooks in .kiro/hooks");
}

// ---- Requirement 23: skills, MCP, .kiroignore ----------------------------------------------
for (const name of ["visual-qa", "release-gate", "demo-video"]) {
  const rel = `.kiro/skills/${name}/SKILL.md`;
  if (!has(rel)) {
    fail(`skill: ${rel} is missing`);
    continue;
  }
  const fm = frontMatter(read(rel));
  if (fm?.name !== name) fail(`skill ${name}: front matter name must equal the folder name`);
  if (!fm?.description) fail(`skill ${name}: needs a description`);
}
const mcpRel = has(".kiro/settings/mcp.json") ? ".kiro/settings/mcp.json" : "docs/kiro/mcp.json";
if (!has(mcpRel))
  fail("mcp: no .kiro/settings/mcp.json (or the docs/kiro/mcp.json source to paste)");
else {
  const servers = JSON.parse(read(mcpRel)).mcpServers ?? {};
  const pw = servers.playwright;
  const args = pw?.args ?? [];
  if (!pw) fail(`mcp: ${mcpRel} lacks the playwright server`);
  else {
    if (!args.some((a) => /^@playwright\/mcp@\d+\.\d+\.\d+$/.test(a)))
      fail("mcp: playwright must pin @playwright/mcp to an exact version");
    if (!args.includes("--isolated")) fail("mcp: playwright must use --isolated");
    const out = args[args.indexOf("--output-dir") + 1];
    if (out !== ".scratch/playwright")
      fail("mcp: playwright --output-dir must be .scratch/playwright");
    const origins = args[args.indexOf("--allowed-origins") + 1] ?? "";
    if (origins !== "http://localhost:*;https://amirmlotfy.github.io")
      fail(
        "mcp: playwright --allowed-origins must be http://localhost:*;https://amirmlotfy.github.io",
      );
  }
  const chrome = servers["playwright-chrome"];
  if (!chrome || !(chrome.args ?? []).includes("--extension") || chrome.disabled !== true)
    fail("mcp: playwright-chrome must use --extension and be disabled");
  if (Object.keys(servers).some((k) => /github/i.test(k)))
    fail("mcp: no GitHub MCP server is allowed");
}
// The agent cannot write .kiroignore (Kiro denies that path), so the user pastes
// docs/kiro/kiroignore.txt to the repo root. If only the source exists, note it and pass;
// if neither exists, fail.
const kiroignoreRel = has(".kiroignore")
  ? ".kiroignore"
  : has("docs/kiro/kiroignore.txt")
    ? "docs/kiro/kiroignore.txt"
    : null;
if (kiroignoreRel === null) {
  fail(".kiroignore is missing and docs/kiro/kiroignore.txt (its source) is missing too");
} else {
  const lines = read(kiroignoreRel)
    .split("\n")
    .map((l) => l.trim());
  for (const e of ["target/", "node_modules/", "media/raw/", ".scratch/"])
    if (!lines.includes(e)) fail(`${kiroignoreRel} lacks ${e}`);
  if (kiroignoreRel !== ".kiroignore") {
    console.log(
      "NOTE  .kiroignore is not at the repo root yet; paste docs/kiro/kiroignore.txt there (the agent cannot write it).",
    );
  }
}

if (failures.length === 0) {
  console.log("check-config: OK");
} else {
  for (const f of failures) console.log(`FAIL  ${f}`);
  console.log(`check-config: ${failures.length} failure(s)`);
  process.exit(1);
}
