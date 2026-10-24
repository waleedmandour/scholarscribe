// ScholarScribe. Regression test for the CRLF-vs-lockfile parity bug.
//
// History (word-assist-ci run 40113154808, node (windows-latest) leg,
// 2026-10-24): crateVersion() in check-tauri-parity.mjs matched
// `name = "<crate>"\nversion = "…"` with a hard-coded \n. On a Windows
// autocrlf checkout src-tauri/Cargo.lock had CRLF bytes (no eol=lf rule
// for it existed in .gitattributes then), so all three crate versions
// resolved as "absent" and the gate failed falsely:
//   PARITY: MISSING tauri=absent in src-tauri/Cargo.lock, …
//
// This test pins BOTH halves of the fix so neither can regress silently:
//   1. the parser must resolve identical versions from LF and CRLF bytes;
//   2. (attribute half, checked in CI clones) .gitattributes must keep
//      src-tauri/Cargo.lock at LF bytes on every checkout — see the
//      CARD C1-FIX clone-matrix verification record.
//
// Pure node: no dependencies, runs as part of `npm test` on every OS.

import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { crateVersion } from "./check-tauri-parity.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const lockLf = readFileSync(join(root, "src-tauri", "Cargo.lock"), "utf8");
// Simulate a core.autocrlf=true checkout of the same file.
const lockCrlf = lockLf.replace(/\r?\n/g, "\r\n");

const crates = ["tauri", "tauri-plugin-shell", "tauri-plugin-dialog"];
let failed = 0;

for (const c of crates) {
  const vLf = crateVersion(lockLf, c);
  const vCrlf = crateVersion(lockCrlf, c);
  if (!vLf) {
    console.error(
      `CRLF-REGRESSION: ${c} unresolvable even from LF bytes — the parity parser itself is broken`
    );
    failed += 1;
    continue;
  }
  if (vLf !== vCrlf) {
    console.error(
      `CRLF-REGRESSION: ${c} resolves to ${vLf} from LF but ${vCrlf} from CRLF — the hard-coded \\n bug is back`
    );
    failed += 1;
    continue;
  }
  console.log(`CRLF-REGRESSION: ok  ${c} v${vLf} resolves identically from LF and CRLF bytes`);
}

if (failed) {
  console.error(`CRLF-REGRESSION: ${failed} failure(s)`);
  process.exit(1);
}
console.log("CRLF-REGRESSION: crate-version parsing is line-ending independent.");
