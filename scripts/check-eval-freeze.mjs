#!/usr/bin/env node
// Held-out freeze integrity check (hardening round, owner directive).
// Every file listed in docs/EVAL/FROZEN.txt must exist and hash to the
// recorded SHA-256. CI (word-assist-gates) runs this after the H7 step;
// locally: `node scripts/check-eval-freeze.mjs`.
//
// FROZEN.txt format: "<sha256 hex>  <repo-relative path>" per line
// (sha256sum-compatible). To freeze a NEW file, run:
//   sha256sum docs/EVAL/heldout.v1.jsonl >> docs/EVAL/FROZEN.txt
// and have the owner approve the addition. Frozen files are never
// edited in place — a new version is a new file (LABELLING_PROTOCOL
// rule 7).

import { createHash } from "node:crypto";
import { readFileSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const frozenListPath = join(repoRoot, "docs", "EVAL", "FROZEN.txt");

let failures = 0;
let entries = 0;

const lines = readFileSync(frozenListPath, "utf8")
  .split("\n")
  .map((l) => l.trim())
  .filter((l) => l.length > 0 && !l.startsWith("#"));

for (const line of lines) {
  entries += 1;
  const m = line.match(/^([0-9a-f]{64})\s+\*?(.+)$/);
  if (!m) {
    console.error(`FROZEN.txt: unparseable line: ${line}`);
    failures += 1;
    continue;
  }
  const [, expected, relPath] = m;
  const abs = join(repoRoot, relPath);
  if (!existsSync(abs)) {
    console.error(`MISSING: ${relPath} (listed in FROZEN.txt)`);
    failures += 1;
    continue;
  }
  const actual = createHash("sha256").update(readFileSync(abs)).digest("hex");
  if (actual !== expected) {
    console.error(`CHANGED: ${relPath}\n  frozen: ${expected}\n  actual: ${actual}`);
    failures += 1;
  } else {
    console.log(`ok: ${relPath} @ ${expected.slice(0, 12)}…`);
  }
}

if (entries === 0) {
  console.error("FROZEN.txt lists no files — nothing is frozen; refusing to pass");
  failures += 1;
}

if (failures > 0) {
  console.error(`\nfreeze check FAILED: ${failures} problem(s) across ${entries} entries`);
  process.exit(1);
}
console.log(`\nfreeze check OK: ${entries} frozen file(s) unchanged`);
