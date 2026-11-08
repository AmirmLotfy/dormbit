#!/usr/bin/env node
// Plans the demo video timeline from media/script/script.md: words per voice line at a
// reading pace, plus the fixed music-only beats, and prints each segment's start and end.
// Usage: node scripts/media/script-timing.mjs [words-per-minute]
import { readFile } from "node:fs/promises";

const wpm = Number(process.argv[2] ?? 150);
const md = await readFile(new URL("../../media/script/script.md", import.meta.url), "utf8");
const body = md.slice(md.indexOf("## Lines"), md.indexOf("## Why each claim"));

const lines = [...body.matchAll(/^### (V\d\d): (.+)\n\n([\s\S]+?)(?=\n### |\n*$)/gm)].map((m) => ({
  id: m[1],
  title: m[2],
  words: m[3].replace(/[`()]/g, "").trim().split(/\s+/).length,
}));

// Pauses inside a line (sentence breaks) and music-only beats between lines, in seconds.
const PAUSE = { V01: 1.0, V08: 2.0, V07: 0.8, V10: 0.6 };
const BEAT_BEFORE = { V01: 2.0, V02: 1.5, V06: 3.5, V08: 3.0, V09: 1.5 };
const TAIL = 1.0;

const fmt = (s) => `${String(Math.floor(s / 60))}:${(s % 60).toFixed(1).padStart(4, "0")}`;
let t = 0;
let speech = 0;
for (const l of lines) {
  t += BEAT_BEFORE[l.id] ?? 0;
  const d = (l.words / wpm) * 60 + (PAUSE[l.id] ?? 0.5);
  speech += d;
  console.log(`${l.id}  ${fmt(t)}-${fmt(t + d)}  ${String(l.words).padStart(3)} words  ${l.title}`);
  t += d;
}
t += TAIL;
const words = lines.reduce((n, l) => n + l.words, 0);
console.log(
  `\n${String(words)} words at ${String(wpm)} wpm: ${fmt(speech)} of speech, ${fmt(t)} total`,
);
if (t < 155 || t > 165) console.log("Outside the 155-165 s target (Requirement 16.1).");
