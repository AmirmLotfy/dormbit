// Runs the compiled demo engine (WebAssembly) in Node through the same calls the browser
// makes, and checks the capsule hash with WebCrypto. Usage: node apps/demo/scripts/wasm-smoke.mjs
import { readFile } from "node:fs/promises";
import { webcrypto } from "node:crypto";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const pkg = join(here, "../src/wasm/pkg");
const data = join(here, "../public/demo-data");
const { initSync, DemoEngine } = await import(join(pkg, "dormbit_wasm.js"));
initSync({ module: await readFile(join(pkg, "dormbit_wasm_bg.wasm")) });

const tar = new Uint8Array(await readFile(join(data, "dockline-invoices.tar")));
const sidecar = await readFile(join(data, "dockline-invoices.sidecar.json"), "utf8");
const e = new DemoEngine(tar, sidecar);
const call = (cmd, args = {}) => JSON.parse(e.call(cmd, JSON.stringify(args)));
const hex = async (bytes) =>
  [...new Uint8Array(await webcrypto.subtle.digest("SHA-256", bytes))]
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
const check = (ok, what) => {
  if (!ok) {
    console.error("FAIL:", what);
    process.exit(1);
  }
  console.log("ok:", what);
};

const d = call("discover");
check(d.result.projects[0].name === "dockline-invoices", "discovery finds Dockline");
const ev = call("evidence");
check(JSON.stringify(ev).includes("tasks complete"), "evidence has task progress");
let plan = call("plan_create", { git_treatment: "full" });
plan = call("plan_set_decision", { path: "data/dev.sqlite", decision: "exclude" });
call("plan_approve", { digest: plan.digest, acks: [] });
const cap = call("capture");
const bytes = e.capsule_bytes();
check((await hex(bytes)) === cap.record.file_sha256, "WebCrypto SHA-256 equals the capsule record");
check(call("verify").status === "verified", "capsule verifies");
check(
  call("restore", { target_name: "dockline-invoices" }).refused === true,
  "restore into the existing name is refused",
);
const r = call("restore", { target_name: "dockline-restored" });
check(r.report.status === "verified", "restore verified");
const files = call("restored_files").files;
let same = 0;
for (const f of files) {
  if ((await hex(e.restored_file_bytes(f.path))) === f.expected_sha256) same++;
}
check(
  same === files.length && same > 100,
  `independent re-hash: ${same}/${files.length} files match`,
);
const b = call("brief");
check(b.markdown.includes("- AI session resumed: not performed."), "brief has the four outcomes");
console.log("wasm smoke: all checks passed");
