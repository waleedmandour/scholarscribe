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

// --- 1. rebuild into a temp dir and compare -----------------------------
const tmp = mkdtempSync(join(tmpdir(), "addin-dist-"));
execFileSync(join(repoRoot, "node_modules", ".bin", "tsc"), ["-p", join(addin, "tsconfig.json"), "--outDir", tmp], {
  stdio: "inherit",
});
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
  if (!built.includes(f)) {
    fail(`dist file ${f} has no built counterpart (stale leftover? regenerate dist)`);
  }
}

// --- 2. every built module must be embedded in the listener -------------
const serverSrc = readFileSync(join(repoRoot, "src-tauri/src/assist/server.rs"), "utf8");
for (const f of built) {
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
for (const f of [...built, "taskpane.html", "manifest.xml"]) {
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
console.log("addin dist integrity: OK (" + built.length + " modules byte-identical, all embedded, LF-only)");
