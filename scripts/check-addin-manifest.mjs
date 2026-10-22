// ScholarScribe. Structural manifest check (v0.1 work order A4 / D3).
//
// Checks the add-in manifest that ships to Word: well-formed structure,
// the elements Microsoft's manifest reference requires for a TaskPaneApp,
// the stable GUID, the {PORT} placeholder contract (the on-disk file reads
// 38080 and the listener substitutes the configured port), HTTPS
// SourceLocation, and LF-only bytes (embedded + served byte-for-byte).
//
// This is NOT Microsoft's validator: the proposal to run
// `npx office-addin-manifest validate` once per release (owner-run, dev
// tool) lives in docs/WORD_ASSIST_INSTALL.md and docs/RELEASE_PLAN_v0.1.md.

import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const xml = readFileSync(join(repoRoot, "word-addin/dist/manifest.xml"), "utf8");

let failures = 0;
const fail = (m) => {
  failures += 1;
  console.error("FAIL " + m);
};
const expect = (cond, m) => (cond ? console.log("ok   " + m) : fail(m));

// Required elements for TaskPaneApp (add-in-only manifest reference).
for (const el of [
  "<OfficeApp",
  'xsi:type="TaskPaneApp"',
  "<Id>",
  "<Version>",
  "<ProviderName>",
  "<DefaultLocale>",
  "<DisplayName DefaultValue=",
  "<Description DefaultValue=",
  "<IconUrl DefaultValue=",
  "<AppDomains>",
  "<Hosts>",
  '<Host Name="Document"/>',
  "<Requirements>",
  '<Set Name="WordApi"',
  "<DefaultSettings>",
  "<SourceLocation DefaultValue=",
  "<Permissions>ReadWriteDocument</Permissions>",
]) {
  expect(xml.includes(el), `manifest contains ${el}`);
}

// Stable GUID: a real UUID v4 shape, and the exact value recorded here so
// a silent GUID change is caught (a new GUID would duplicate the add-in
// for every existing user).
const GUID = "3f4c8d61-9a27-4b5e-b2c8-01d3e5f7a9b4";
const idMatch = xml.match(/<Id>([0-9a-f-]{36})<\/Id>/);
expect(idMatch !== null, "manifest <Id> is a 36-char id");
if (idMatch) {
  expect(idMatch[1] === GUID, `manifest GUID is the stable ${GUID}`);
}

// Placeholder contract: on-disk file carries {PORT}; no accidental
// hard-coded localhost URL other than the placeholders.
expect(xml.includes("{PORT}"), "manifest uses the {PORT} placeholder");
expect(xml.includes("https://localhost:{PORT}"), "SourceLocation/IconUrl are https://localhost:{PORT}");
expect(!/https:\/\/localhost:\d+/.test(xml), "no hard-coded port in the on-disk manifest");

// Minimum requirement set: WordApi 1.1 (the client probes 1.6/1.7/1.8 and
// degrades; the manifest must not gate above the floor).
expect(/<Set Name="WordApi" MinVersion="1\.1"\/>/.test(xml), "WordApi floor stays at 1.1");

// LF-only (the file is embedded via include_str! and served byte-for-byte).
expect(!Buffer.from(xml, "utf8").includes(0x0d), "manifest is LF-only");

if (failures > 0) {
  console.error(`\nmanifest check: ${failures} failure(s)`);
  process.exit(1);
}
console.log("manifest check: OK");
