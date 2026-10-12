#!/usr/bin/env node
/**
 * cross-verify-fixture.mjs - JavaScript half of the Rust-vs-JS verifier
 * cross-check (work order T1.7, closes D8). CI runs this plus the Rust test
 * src-tauri/tests/cross_language.rs against the SAME committed fixture in
 * src-tauri/fixtures/cross-check/.
 *
 * This script re-implements exactly the verification steps of
 * verifier/verify.js (chain re-hash + Ed25519 detached signature over the
 * canonical SCHOLARSCRIBE-MANIFEST-v1 payload) using the SAME vendored
 * libraries the offline verifier ships (verifier/vendor/). Keep the canonical
 * payload builder in sync with verifier/verify.js:60-81 and Rust's
 * provenance.rs:canonical_manifest_payload; the committed payload.txt pins all
 * three together.
 *
 * Exit 0 = the JS verifier agrees with the Rust fixture. Exit 1 = drift.
 */
import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";


const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const FIX = join(root, "src-tauri", "fixtures", "cross-check");
const nodeRequire = createRequire(import.meta.url);

// Vendored UMD builds used by the offline verifier page. The repo's
// package.json sets "type": "module", so require() would misinterpret these
// .js UMD files as ESM; instead evaluate each in a function scope that
// provides the CJS globals the UMD wrapper expects.
function loadVendor(path) {
  const code = readFileSync(path, "utf8");
  const fakeModule = { exports: {} };
  // sha256.min.js's UMD wrapper calls require("crypto") when it detects Node;
  // hand the sandbox a real require so it can use it.
  const loader = new Function("module", "exports", "self", "require", code);
  const fakeSelf = {};
  loader(fakeModule, fakeModule.exports, fakeSelf, nodeRequire);
  if (fakeModule.exports && Object.keys(fakeModule.exports).length) return fakeModule.exports;
  return fakeSelf.nacl;
}

const nacl = loadVendor(join(root, "verifier", "vendor", "nacl-fast.min.js"));
const jsSha = loadVendor(join(root, "verifier", "vendor", "sha256.min.js"));

const manifest = JSON.parse(readFileSync(join(FIX, "manifest.json"), "utf8"));
const payloadCommitted = readFileSync(join(FIX, "payload.txt"), "utf8").replace(/\n$/, "");
const pubKeyLine = readFileSync(join(FIX, "public-key.txt"), "utf8").trim();

function sha256Hex(s) { return jsSha.sha256(s); }

function fail(msg) { console.error("cross-verify-fixture: FAIL"); console.error("  - " + msg); process.exit(1); }

// 1. Fingerprint: sha256(pubkey) must equal manifest.author_key_fingerprint.
const pubHex = pubKeyLine.replace(/^ed25519-pub:/, "");
const pubBytes = new Uint8Array(pubHex.match(/.{2}/g).map((h) => parseInt(h, 16)));
const fp = sha256Hex(pubBytes); // js-sha256 accepts Uint8Array directly (raw bytes)
if ("ed25519:" + fp !== manifest.author_key_fingerprint) {
  fail(`fingerprint mismatch: manifest=${manifest.author_key_fingerprint} computed=ed25519:${fp}`);
}

// 2. Canonical payload (MUST mirror verify.js:60-81 and Rust canonical_manifest_payload).
function canonicalPayload(m) {
  const lines = [];
  lines.push("SCHOLARSCRIBE-MANIFEST-v1");
  lines.push(String(m.version));
  lines.push(m.document_hash);
  lines.push(m.author_key_fingerprint);
  lines.push(String(m.generated_at));
  lines.push(m.generator);
  lines.push(String(m.revision_count));
  for (const s of m.sessions) {
    lines.push([s.session_id, s.author, String(s.start_time), String(s.end_time), s.snapshot_hash,
      String(s.chars_added), String(s.chars_removed), String(s.largest_insertion),
      s.prev_record_hash, s.record_hash].join("|"));
  }
  lines.push(m.style_consistency && m.style_consistency.distance_score != null
    ? Number(m.style_consistency.distance_score).toFixed(4)
    : "null");
  lines.push(m.style_consistency ? m.style_consistency.baseline_source : "");
  lines.push(m.style_consistency ? (m.style_consistency.metrics_compared || []).join(",") : "");
  // verify.js builds with a trailing "" element then join("\n"), producing the
  // trailing newline the Rust builder emits.
  lines.push("");
  return lines.join("\n");
}

const payload = canonicalPayload(manifest);
if (payload !== payloadCommitted) {
  fail(`canonical payload drift between JS and the committed fixture (Rust-built):\n--- JS ---\n${payload}\n--- fixture ---\n${payloadCommitted}`);
}

// 3. Signature: Ed25519 detached verify with tweetnacl (same lib as verify.js).
const sigHex = manifest.signature.replace(/^ed25519:/, "");
const sigBytes = new Uint8Array(sigHex.match(/.{2}/g).map((h) => parseInt(h, 16)));
const msgBytes = new Uint8Array(Buffer.from(payload, "utf8"));
const ok = nacl.sign.detached.verify(msgBytes, sigBytes, pubBytes);
if (!ok) fail("Ed25519 signature does not verify under tweetnacl (same library as the browser verifier)");

// 4. Chain re-hash (record_hash = sha256 of canonical record string, genesis linkage).
const GENESIS = "sha256:" + "0".repeat(64);
function canonicalRecordString(rec) {
  // MUST mirror verify.js:38-51 and Rust provenance.rs:335-349: exactly 10
  // newline-joined fields, NO trailing record_hash and NO trailing newline.
  return ["scholarscribe-record-v1", rec.session_id, rec.author, String(rec.start_time),
    String(rec.end_time), rec.snapshot_hash, String(rec.chars_added), String(rec.chars_removed),
    String(rec.largest_insertion), rec.prev_record_hash].join("\n");
}
const lastHashByAuthor = new Map();
for (const rec of manifest.sessions) {
  const prev = lastHashByAuthor.get(rec.author) || GENESIS;
  if (rec.prev_record_hash !== prev) fail(`chain linkage broken at session ${rec.session_id}`);
  const recomputed = "sha256:" + sha256Hex(canonicalRecordString(rec));
  if (recomputed !== rec.record_hash) fail(`record hash re-derivation mismatch at session ${rec.session_id}`);
  lastHashByAuthor.set(rec.author, rec.record_hash);
}

console.log(`cross-verify-fixture: OK (JS verifier agrees with the Rust fixture: fingerprint, canonical payload, signature, ${manifest.sessions.length} chained session(s))`);
