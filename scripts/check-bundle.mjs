#!/usr/bin/env node
// Bundle scan for the judge demo (judge-demo Requirements 2.1, 2.4, 15.1): the built site must
// contain no file or folder picker API, no file input, and no analytics code.
// Usage: node scripts/check-bundle.mjs [dist folder]
import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

const root = process.argv[2] ?? "apps/demo/dist";
const BANNED = [
  "showDirectoryPicker",
  "showOpenFilePicker",
  "showSaveFilePicker",
  "webkitdirectory",
  'type="file"',
  'type:"file"',
  "googletagmanager",
  "google-analytics",
  "gtag(",
  "plausible.io",
  "segment.io",
  "mixpanel",
  "posthog",
  "sentry.io",
];

async function* files(dir) {
  for (const e of await readdir(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) yield* files(p);
    else if (/\.(html|js|mjs|css|json|map)$/.test(e.name)) yield p;
  }
}

let found = 0;
let scanned = 0;
for await (const f of files(root)) {
  scanned++;
  const text = await readFile(f, "utf8");
  for (const b of BANNED) {
    if (text.includes(b)) {
      console.error(`${f}: contains ${b}`);
      found++;
    }
  }
}
if (scanned === 0) {
  console.error(`check-bundle: no files under ${root}`);
  process.exit(1);
}
console.log(`check-bundle: ${String(scanned)} files scanned, ${String(found)} problems`);
process.exit(found === 0 ? 0 : 1);
