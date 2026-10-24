// ScholarScribe. Add-in dist integrity check (v0.1 work order A2 / D3).
//
// 1. Rebuilds word-addin/dist with tsc into a temp dir and byte-compares
//    every file against the committed dist — a stale dist fails CI.
// 2. Verifies every built module is embedded in the Rust listener route
//    table (src/assist/server.rs ADDIN_MODULE_FILES) — a module missing
//    from the server fails CI.
// 3. Verifies taskpane.html loads taskpane.js as a module (the ESM serve
//    model) and that the dist files are LF-only.
//
// Exit 0 = dist matches source; nonzero with a diff report otherwise.

import { execFileSync } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const addin = join(repoRoot, "word-addin");
const dist = join(addin, "dist");

let failures = 0;
function fail(msg) {
  failures += 1;
  console.error("FAIL " + msg);
}

// Hand-maintained dist assets: NOT tsc outputs, so they are exempt from
// the byte-compare below, but they carry their own contracts (embedded
// in the listener, LF-only, loaded in the right order).
//  - legacy-check.js: classic ES5 runtime probe for webviews without
//    ES-module support (A2 integration). tsc must NOT build it — it has
//    to stay a classic script so legacy engines can even parse it.
const HAND_MAINTAINED = new Set(["legacy-check.js"]);

// --- 1. rebuild into a temp dir and compare -----------------------------
const tmp = mkdtempSync(join(tmpdir(), "addin-dist-"));
// Invoke tsc via its JS entry with the running node binary. The
// node_modules/.bin shims are platform-specific (`tsc` is an extensionless
// sh script that spawnSync cannot execute on Windows — ENOENT — while the
// real Windows entry is tsc.cmd; see word-assist-ci run 40115931743,
// node (windows-latest)). Going through node + typescript/bin/tsc is
// identical behaviour on every OS.
execFileSync(
  process.execPath,
  [join(repoRoot, "node_modules", "typescript", "bin", "tsc"), "-p", join(addin, "tsconfig.json"), "--outDir", tmp],
  { stdio: "inherit" },
);
const built = readdirSync(tmp).filter((f) => f.endsWith(".js"));
const committed = readdirSync(dist).filter((f) => f.endsWith(".js"));

for (const f of built) {
  if (!committed.includes(f)) {
    fail(`built module ${f} is missing from word-addin/dist (run npm run addin:build)`);
    continue;
  }
  const a = readFileSync(join(tmp, f));
  const b = readFileSync(join(dist, f));
  if (!a.equals(b)) {
    fail(`${f} differs: committed dist is stale (sha256 committed ${sha(b)} vs built ${sha(a)})`);
  }
}
for (const f of committed) {
  if (!built.includes(f) && !HAND_MAINTAINED.has(f)) {
    fail(`dist file ${f} has no built counterpart (stale leftover? regenerate dist)`);
  }
}

// --- 2. every built module AND hand-maintained script must be embedded -
const serverSrc = readFileSync(join(repoRoot, "src-tauri/src/assist/server.rs"), "utf8");
for (const f of [...built, ...HAND_MAINTAINED]) {
  const route = f === "taskpane.js" ? "/addin/taskpane.js" : "/addin/" + f;
  if (!serverSrc.includes(`"/addin/${f}"`)) {
    fail(`${f} is not embedded in server.rs ADDIN_MODULE_FILES (route ${route})`);
  }
}

// --- 3. html loads the entry as a module; dist is LF-only ---------------
const html = readFileSync(join(dist, "taskpane.html"), "utf8");
if (!html.includes('<script type="module" src="taskpane.js">')) {
  fail('taskpane.html must load taskpane.js with type="module" (ESM serve model)');
}
if (HAND_MAINTAINED.has("legacy-check.js")) {
  const probe = html.indexOf('src="legacy-check.js"');
  const entry = html.indexOf('<script type="module" src="taskpane.js">');
  if (probe === -1) {
    fail("taskpane.html must load legacy-check.js (webviews without ES modules otherwise show a dead pane)");
  } else if (entry !== -1 && probe > entry) {
    fail("legacy-check.js must load BEFORE the module entry (it is the fallback for engines that skip module scripts)");
  }
}
for (const f of [...built, ...HAND_MAINTAINED, "taskpane.html", "manifest.xml"]) {
  const p = join(dist, f);
  if (!existsSync(p)) continue; // already reported above
  const bytes = readFileSync(p);
  if (bytes.includes(0x0d)) {
    fail(`${f} contains CR bytes — dist must be LF-only (.gitattributes enforces checkout; regenerate)`);
  }
}

function sha(buf) {
  return createHash("sha256").update(buf).digest("hex").slice(0, 16);
}

if (failures > 0) {
  console.error(`\naddin dist integrity: ${failures} failure(s)`);
  console.error("Fix: npm run addin:build, then commit word-addin/dist/");
  process.exit(1);
}
console.log(
  "addin dist integrity: OK (" + built.length + " modules byte-identical, " +
    HAND_MAINTAINED.size + " hand-maintained checked by contract, all embedded, LF-only)",
);
