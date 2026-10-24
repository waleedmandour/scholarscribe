// ScholarScribe. Workflow shell regression gate (CARD C1-FIX iteration 2).
//
// History (word-assist-ci run 40115931743, rust (windows-latest) legs,
// 2026-10-24): the H7 step used bash syntax (`if …; then`) without
// `shell: bash`. Windows runners default `run:` to pwsh, which cannot parse
// bash control flow — the leg died at that step with
// `Missing '(' after 'if' in if statement.` Linux/macOS runners default to
// bash, so the same defect is invisible there.
//
// Gate: every step whose `run` block contains bash-ONLY syntax must have an
// explicit `shell: bash`. Patterns are bash-only by construction and do not
// fire on pwsh-compatible code: `; then` / `[[` / `; do` / PIPESTATUS.
// (`$( )` is deliberately NOT a pattern — pwsh shares the subexpression
// syntax; `| tee` is not a pattern — pwsh aliases tee to Tee-Object.)
//
// Shell resolution per step: the step's own `shell:` key wins, else a
// file-level `defaults: run: shell:` (approximated as applying to the whole
// file; no workflow in this repo sets one today), else the runner default
// (bash on Linux/macOS, pwsh on Windows) — which does NOT count as bash,
// because the whole point is that Windows legs must still work.
//
// Pure node, dependency-free; runs as part of `npm test` on every leg.

import { readFileSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const wfDir = join(dirname(fileURLToPath(import.meta.url)), "..", ".github", "workflows");
const BASH_ONLY = [/;\s*then\b/, /\[\[/, /;\s*do\b/, /PIPESTATUS/];

let failures = 0;

for (const f of readdirSync(wfDir).filter((n) => n.endsWith(".yml") || n.endsWith(".yaml"))) {
  const lines = readFileSync(join(wfDir, f), "utf8").split("\n");

  // file-level defaults.run.shell (whole-file approximation, documented above)
  let fileDefaultShell = null;
  for (let i = 0; i < lines.length - 2; i += 1) {
    if (/^\s+defaults:\s*$/.test(lines[i]) && /^\s+run:\s*$/.test(lines[i + 1])) {
      const sm = lines[i + 2].match(/^\s+shell:\s*(\S+)/);
      if (sm) fileDefaultShell = sm[1];
    }
  }

  // steps = list items `- ` at any indent that contain a run: key.
  for (let i = 0; i < lines.length; i += 1) {
    const sm = lines[i].match(/^(\s*)-\s+(.*)$/);
    if (!sm) continue;
    const indent = sm[1].length;

    // step block = this line up to the next list item at indent <= `indent`
    let j = i + 1;
    while (j < lines.length) {
      const m = lines[j].match(/^(\s*)-\s+/);
      if (m && m[1].length <= indent) break;
      // a key at shallower-or-equal indent than the step keys also ends the step
      if (/^[^\s#-]/.test(lines[j])) break;
      j += 1;
    }
    const block = lines.slice(i, j);
    if (!block.some((l) => /^\s+run:\s/.test(l))) continue;

    let shell = fileDefaultShell;
    for (const l of block) {
      const km = l.match(/^\s+shell:\s*(\S+)/);
      if (km) shell = km[1];
    }
    if (shell === "bash") continue;

    for (let k = 0; k < block.length; k += 1) {
      const rm = block[k].match(/^(\s*)run:\s*(.*)$/);
      if (!rm) continue;
      const runIndent = rm[1].length;
      let text = rm[2];
      if (["|", ">", "|-", ">-", "|+", ">+"].includes(text.trim())) {
        text = "";
        for (let x = k + 1; x < block.length; x += 1) {
          const l = block[x];
          if (!l.trim()) continue;
          const ind = l.length - l.trimStart().length;
          if (ind <= runIndent) break;
          text += l + "\n";
        }
      }
      if (BASH_ONLY.some((re) => re.test(text))) {
        failures += 1;
        console.error(
          `FAIL ${f}:${i + 1} (${(sm[2] || "").slice(0, 50)}) run block uses bash-only syntax but shell is '${shell ?? "runner default (pwsh on Windows)"}' — add shell: bash`
        );
      }
    }
  }
}

if (failures > 0) {
  console.error(`\nworkflow shell gate: ${failures} failure(s)`);
  process.exit(1);
}
console.log("workflow shell gate: OK (every bash-syntax run block pins shell: bash)");
