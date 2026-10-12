#!/usr/bin/env node
/**
 * check-references.mjs - T1.3 enforcement test (runs in CI via `npm test`).
 *
 * Rule: every author-year citation used in user-facing text must exist in
 * src/lib/references.json (the single source of references), and every entry
 * in references.json must carry a DOI. This exists because v2.2.1 shipped an
 * unverified citation ("Laban et al. (2024)") and two contradictory
 * Weber-Wulff identities; see docs/DECISIONS.md (2026-10-13, T1.3).
 *
 * Exit 0 = pass, exit 1 = fail with a list of violations.
 */
import { readFileSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const refPath = join(root, "src", "lib", "references.json");

/** User-facing sources where citations may appear. */
const SOURCES = [
  "src-tauri/src/appeal_letter.rs",
  "src/components/DetectorLiteracy.svelte",
  "src/components/RiskProfiler.svelte",
  "src/components/AppealLetter.svelte",
  "src/components/About.svelte",
  "README.md",
  "USER_MANUAL.md",
  "USER_GUIDE.md",
  "docs/ETHICS.md",
];

// "X et al. (2023)", "X et al. 2023", "X, W., et al. (2023)", "X et al., 2023"
const CITATION_RE = /\b([A-Z][A-Za-z\u00C0-\u024F -]+?)\s*(?:,\s*[A-Z]\.(?:\s*[A-Z]\.)*)?,?\s+et al\.?,?\s*\(?(20\d\d)\)?/g;

const refs = JSON.parse(readFileSync(refPath, "utf8"));
const known = new Map();
for (const [key, r] of Object.entries(refs.references || {})) {
  if (!r.doi) fail(`reference "${key}" has no DOI`);
  const family = (r.authors?.[0] || "").split(",")[0].trim().toLowerCase();
  known.set(family, { key, year: r.year });
}

const problems = [];
function fail(msg) { problems.push(msg); }

for (const rel of SOURCES) {
  const abs = join(root, rel);
  if (!existsSync(abs)) { fail(`listed source does not exist: ${rel}`); continue; }
  const text = readFileSync(abs, "utf8");
  for (const m of text.matchAll(CITATION_RE)) {
    const family = m[1].trim().split(/\s+/).pop().replace(/-\s/g, "-").toLowerCase();
    const year = m[2];
    const hit = known.get(family) || known.get(family.split("-").pop());
    if (!hit) { fail(`${rel}: citation "${family} et al. (${year})" is not in src/lib/references.json`); }
    else if (String(hit.year) !== year) { fail(`${rel}: "${m[1].trim()} et al." year ${year} does not match references.json year ${hit.year} (key ${hit.key})`); }
  }
}

// Every known reference should actually be used somewhere (keep the file honest).
const allText = SOURCES.map((rel) => {
  const abs = join(root, rel);
  return existsSync(abs) ? readFileSync(abs, "utf8") : "";
}).join("\n");
for (const [family, { key }] of known) {
  if (!allText.toLowerCase().includes(family)) {
    fail(`references.json entry "${key}" (family "${family}") is not cited in any user-facing source`);
  }
}

// Removed references must stay removed.
for (const [gone, info] of Object.entries(refs.removed_references || {})) {
  if (gone === "laban-2024" && /Laban/i.test(allText)) {
    fail(`removed reference "laban-2024" reappeared in user-facing text (removed ${info.removed_date}: ${info.reason.slice(0, 120)}...)`);
  }
}

if (problems.length) {
  console.error("check-references: FAIL");
  for (const p of problems) console.error("  - " + p);
  process.exit(1);
}
console.log(`check-references: OK (${known.size} verified references, ${SOURCES.length} sources scanned)`);
