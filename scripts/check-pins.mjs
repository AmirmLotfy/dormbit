#!/usr/bin/env node
// Checks that every npm dependency in the repo is an exact version (no ^ ~ * range),
// that Cargo [workspace.dependencies] entries start with "=", and that Node/pnpm
// versions in package.json agree with .mise.toml. Exits 1 with one line per failure.
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const root = new URL("..", import.meta.url).pathname;
let fail = false;
const report = (msg) => {
  console.error(msg);
  fail = true;
};

function listPackageJsonFiles(dir, out = []) {
  for (const name of readdirSync(dir)) {
    if (name === "node_modules" || name === "target" || name === ".git" || name === ".scratch") continue;
    const p = join(dir, name);
    const st = statSync(p);
    if (st.isDirectory()) listPackageJsonFiles(p, out);
    else if (name === "package.json") out.push(p);
  }
  return out;
}

const RANGE_CHARS = /[\^~*><]/;

for (const file of listPackageJsonFiles(root)) {
  const pkg = JSON.parse(readFileSync(file, "utf8"));
  for (const field of ["dependencies", "devDependencies"]) {
    const deps = pkg[field] ?? {};
    for (const [name, version] of Object.entries(deps)) {
      if (version.startsWith("workspace:")) continue;
      if (RANGE_CHARS.test(version)) {
        report(`${relative(root, file)}: ${field}.${name} uses a range ("${version}"), expected an exact version`);
      }
    }
  }
}

const cargoToml = readFileSync(join(root, "Cargo.toml"), "utf8");
const depsSection = cargoToml.split("[workspace.dependencies]")[1] ?? "";
for (const line of depsSection.split("\n")) {
  const m = line.match(/^(\w[\w-]*)\s*=\s*(.+)$/);
  if (!m) continue;
  const [, name, rhs] = m;
  const versionMatch = rhs.match(/version\s*=\s*"([^"]+)"/) ?? rhs.match(/^"([^"]+)"/);
  if (!versionMatch) continue;
  const version = versionMatch[1];
  if (!version.startsWith("=")) {
    report(`Cargo.toml [workspace.dependencies] ${name} = "${version}" must start with "="`);
  }
}

const pkgJson = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const miseToml = readFileSync(join(root, ".mise.toml"), "utf8");
const nodeInMise = /node\s*=\s*"([^"]+)"/.exec(miseToml)?.[1];
const nodeInPkg = pkgJson.engines?.node;
if (nodeInMise && nodeInPkg && nodeInMise !== nodeInPkg) {
  report(`Node version mismatch: package.json engines.node="${nodeInPkg}" vs .mise.toml node="${nodeInMise}"`);
}
const pmInPkg = /^pnpm@(.+)$/.exec(pkgJson.packageManager ?? "")?.[1];
const pnpmInMise = /pnpm\s*=\s*"([^"]+)"/.exec(miseToml)?.[1];
if (pmInPkg && pnpmInMise && pmInPkg !== pnpmInMise) {
  report(`pnpm version mismatch: package.json packageManager="${pmInPkg}" vs .mise.toml pnpm="${pnpmInMise}"`);
}

const rustToolchain = readFileSync(join(root, "rust-toolchain.toml"), "utf8");
if (!/channel\s*=\s*"\d+\.\d+\.\d+"/.test(rustToolchain)) {
  report(`rust-toolchain.toml channel is not pinned to an exact version`);
}
if (!rustToolchain.includes("wasm32-unknown-unknown")) {
  report(`rust-toolchain.toml is missing the wasm32-unknown-unknown target`);
}

for (const file of listPackageJsonFiles(root).filter((f) => f.includes("/schema/") || f.includes("/client/") || f.includes("/ui/") || f.includes("/desktop/") || f.includes("/demo/"))) {
  const tsconfigPath = join(file, "..", "tsconfig.json");
  try {
    const tsconfig = JSON.parse(readFileSync(tsconfigPath, "utf8"));
    const opts = tsconfig.compilerOptions ?? {};
    for (const flag of ["strict", "noUncheckedIndexedAccess", "exactOptionalPropertyTypes"]) {
      if (opts[flag] === false) {
        report(`${relative(root, tsconfigPath)} turns off ${flag}`);
      }
    }
  } catch {
    // no local tsconfig.json next to this package.json; nothing to check
  }
}

if (!fail) {
  console.log("check-pins: OK");
}
process.exit(fail ? 1 : 0);
