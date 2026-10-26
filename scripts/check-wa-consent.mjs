// ScholarScribe. Word Assist consent-path regression gate.
//
// Bug class: `word_assist_enabled` in persisted settings gates
// `word_assist_enable` (src-tauri/src/assist_commands.rs), and
// persistence.rs documents that "the disclosure dialog on the Word Assist
// tab is the only code path that sets it to true". The v0.1.0 preview
// shipped with NO code path that ever wrote the flag: every enable
// attempt failed with "Word Assist is not opted in. Enable it from the
// Word Assist tab (the disclosure dialog)." — a dead end with no
// workaround inside the app.
//
// This gate pins the consent path in src/components/WordAssist.svelte:
//   1. acceptDisclosure() routes the opt-in through persistSettings(true);
//   2. persistSettings writes s.word_assist_enabled = true when opted in;
//   3. the write is gated on the opt-in parameter, so "Save settings"
//      (applySettings -> persistSettings()) can never silently opt in.
//
// Dependency-free, same style as the other check-* gates; wired into the
// npm test chain so every node leg of every workflow runs it.

import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const file = join(root, "src", "components", "WordAssist.svelte");
const source = readFileSync(file, "utf8");

let failed = false;

function require(re, what) {
  if (re.test(source)) {
    console.log(`ok  ${what}`);
  } else {
    console.error(`WA-CONSENT: MISSING  ${what}`);
    failed = true;
  }
}

// 1. The disclosure acceptance must pass the opt-in flag to persistSettings.
require(
  /acceptDisclosure\(\)\s*\{[\s\S]{0,600}?persistSettings\(\s*true\s*\)/,
  "acceptDisclosure() calls persistSettings(true)"
);

// 2. The persisted master opt-in must be written on the consent path.
require(
  /s\.word_assist_enabled\s*=\s*true\s*;/,
  "persistSettings writes s.word_assist_enabled = true"
);

// 3. The write must be gated on the opt-in parameter (a plain settings
//    save must not flip the master opt-in behind the disclosure).
require(
  /if\s*\(\s*optIn\s*\)\s*\{[\s\S]{0,600}?s\.word_assist_enabled\s*=\s*true\s*;/,
  "the opt-in write is gated on the disclosure opt-in flag (optIn)"
);

if (failed) {
  console.error(
    "WA-CONSENT: the disclosure consent path is broken — word_assist_enable " +
      "would refuse every enable with 'Word Assist is not opted in.' " +
      "Restore the consent write in src/components/WordAssist.svelte."
  );
  process.exit(1);
}

console.log(
  "WA-CONSENT: consent path intact (disclosure acceptance persists word_assist_enabled)."
);
