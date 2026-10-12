#!/usr/bin/env node
/**
 * check-network-surface.mjs - T1.7 network-surface gate (runs in CI via `npm test`).
 *
 * Rule 1: every host literal found in app source (src-tauri/src, src, index.html,
 *         tauri.conf.json, capabilities, verifier page) must be declared in
 *         docs/NETWORK_SURFACE.json.
 * Rule 2: every entry there marked enforced_in_security_md must appear in SECURITY.md.
 *
 * This exists because v2.2.1 claimed "single outbound host" while the Google Docs
 * bridge and hf.co pulls existed; see docs/AUDIT.md section 3.
 *
 * Exit 0 = pass, exit 1 = fail.
 */
import { readFileSync, readdirSync, statSync, existsSync } from "node:fs";
import { join, dirname, extname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

const SCAN_DIRS = ["src", "src-tauri/src", "verifier"];
const SCAN_FILES = [
  "index.html",
  "src-tauri/tauri.conf.json",
  "src-tauri/capabilities/default.json",
];
const EXTS = new Set([".rs", ".ts", ".svelte", ".js", ".html", ".json"]);

function* walk(dir) {
  for (const name of readdirSync(dir)) {
    if (name === "vendor" || name === "target" || name === "node_modules") continue;
    const p = join(dir, name);
    const st = statSync(p);
    if (st.isDirectory()) yield* walk(p);
    else if (EXTS.has(extname(name))) yield p;
  }
}

const surface = JSON.parse(readFileSync(join(root, "docs", "NETWORK_SURFACE.json"), "utf8"));
const declared = new Set();
for (const h of surface.hosts) {
  const bare = h.host.split(":")[0].replace(/\{.*\}/, "").trim();
  if (bare) declared.add(bare);
  declared.add(h.host);
}

// Special literal patterns that URL regexes miss.
const SPECIAL = [/hf\.co\b/, /schemas\.openxmlformats\.org/, /schema\.tauri\.app/];
const URL_HOST_RE = /^[a-z]+:\/\/([^/:?#]+)/i;

const found = new Map(); // host -> first file that mentions it
function record(host, file) {
  const bare = host.toLowerCase().replace(/\.$/, "");
  if (!bare || bare.length < 4) return;
  if (!found.has(bare)) found.set(bare, file);
}

const files = new Set(SCAN_FILES.filter((f) => existsSync(join(root, f))));
for (const d of SCAN_DIRS) {
  const abs = join(root, d);
  if (existsSync(abs)) for (const f of walk(abs)) files.add(f);
}

for (const file of files) {
  const text = readFileSync(file, "utf8");
  for (const line of text.split("\n")) {
    for (const m of line.matchAll(/https?:\/\/[^\s"'`<>)\],;]+/gi)) {
      const hm = m[0].match(URL_HOST_RE);
      if (hm) record(hm[1], file);
    }
    for (const re of SPECIAL) if (re.test(line)) record(re.source.replace(/\\b|\\\//g, "").replace(/\\\./g, ".").replace(/\[.\]/g, ""), file);
  }
}
// hf.co literal
for (const file of files) {
  const text = readFileSync(file, "utf8");
  if (/\bhf\.co\b/.test(text)) record("hf.co", file);
  if (/schemas\.openxmlformats\.org/.test(text)) record("schemas.openxmlformats.org", file);
  if (/schema\.tauri\.app/.test(text)) record("schema.tauri.app", file);
}

// Drop host candidates that cannot be real hosts (no dot, or no plausible TLD).
for (const [host, file] of [...found]) {
  if (!/^[a-z0-9.-]+\.[a-z]{2,}$/i.test(host) && !/^\d+\.\d+\.\d+\.\d+$/.test(host)) found.delete(host);
}

const problems = [];
for (const [host, file] of found) {
  if (!declared.has(host)) {
    problems.push(`host "${host}" found in ${file} but not declared in docs/NETWORK_SURFACE.json`);
  }
}
for (const h of surface.hosts) {
  if (h.enforced_in_security_md) {
    const sec = readFileSync(join(root, "SECURITY.md"), "utf8");
    const needle = h.host.split(":")[0];
    if (!sec.includes(needle)) {
      problems.push(`host "${h.host}" is marked enforced_in_security_md but "${needle}" is missing from SECURITY.md`);
    }
  }
}

if (problems.length) {
  console.error("check-network-surface: FAIL");
  for (const p of problems) console.error("  - " + p);
  process.exit(1);
}
console.log(`check-network-surface: OK (${found.size} distinct hosts found in source, all declared; ${surface.hosts.filter((h) => h.enforced_in_security_md).length} entries cross-checked against SECURITY.md)`);
