// A2×H1/H2 — simulated-response tests for the pairing/session failure
// paths against the HARDENED engine's exact status codes and messages
// (server.rs is the source of truth for every string asserted here):
// wrong code, burn/lockout, cooldown (route limiter 429), global 429,
// expiry, and 401 after revoke/rotate on /v1/check. Also: nothing that
// looks like a pairing code or a bearer token may reach a user-visible
// string.

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  ApiError,
  classifyCheckFailure,
  classifyPairFailure,
  parsePairOk,
  sanitizeForDisplay,
} from "../src/pairing.js";

// ------------------------------------------------- /v1/pair failure matrix

test("wrong code (401) maps to wrong-code with burn warning", () => {
  const v = classifyPairFailure(
    401,
    "wrong or expired pairing code; get a fresh code from the Word Assist tab",
  );
  assert.equal(v.kind, "wrong-code");
  assert.equal(v.canRetrySameCode, true);
  assert.match(v.message, /wrong/i);
  assert.match(v.message, /5 wrong attempts burn the code/);
});

test("lockout after burn (401) demands a NEW code and names the cooldown", () => {
  const v = classifyPairFailure(
    401,
    "pairing is disabled for now; open the Word Assist tab and request a new code",
  );
  assert.equal(v.kind, "locked");
  assert.equal(v.canRetrySameCode, false);
  assert.match(v.message, /cooldown/);
  assert.match(v.message, /NEW code/);
});

test("pair route limiter (429) is a wait, not a burn", () => {
  const v = classifyPairFailure(429, "too many pairing attempts; wait a minute");
  assert.equal(v.kind, "rate-limited");
  assert.equal(v.canRetrySameCode, true);
  assert.match(v.message, /Wait one minute/);
});

test("global limiter (429) on any route is the same wait state", () => {
  const v = classifyPairFailure(429, "rate limited");
  assert.equal(v.kind, "rate-limited");
});

test("expired code (401) asks for a fresh code and names the TTL", () => {
  const v = classifyPairFailure(
    401,
    "pairing code expired; get a fresh code from the Word Assist tab",
  );
  assert.equal(v.kind, "expired");
  assert.match(v.message, /5 minutes/);
});

test("no active code (401) points at the tab", () => {
  const v = classifyPairFailure(401, "no pairing code is active; get a code from the Word Assist tab");
  assert.equal(v.kind, "no-code");
  assert.equal(v.canRetrySameCode, false);
});

test("server errors and unknown statuses stay actionable", () => {
  assert.equal(classifyPairFailure(500, "token generation failed").kind, "server");
  assert.equal(classifyPairFailure(403, "unexpected Origin").kind, "network");
});

// ------------------------------------------------ /v1/check failure matrix

test("401 after revoke or rotate (H2) => repair: clear token, re-pair", () => {
  for (const msg of [
    "bearer token missing",
    "unknown or revoked token",
    "token expired",
  ]) {
    const v = classifyCheckFailure(401, msg);
    assert.equal(v.action, "repair");
    assert.match(v.message, /revoked, rotated or expired/);
    assert.match(v.message, /fresh pairing code/);
  }
});

test("global 429 on check => back off, keep the session", () => {
  const v = classifyCheckFailure(429, "rate limited");
  assert.equal(v.action, "backoff");
  assert.ok(!/re-pair/i.test(v.message));
});

test("other check failures are reported verbatim, no session damage", () => {
  const v = classifyCheckFailure(400, 'expected {"paragraph_id": ...}');
  assert.equal(v.action, "report");
  assert.equal(v.message, 'expected {"paragraph_id": ...}');
});

// ------------------------------------------------------- pair success body

test("parsePairOk accepts the hardened success body and rejects bad ones", () => {
  assert.equal(parsePairOk({ token: "abc123" }), "abc123");
  const expectApiError = (fn: () => unknown): void => {
    let threw = false;
    try {
      fn();
    } catch (e) {
      threw = true;
      assert.ok(e instanceof ApiError, "expected ApiError, got " + String(e));
    }
    assert.ok(threw, "expected a throw");
  };
  expectApiError(() => parsePairOk({}));
  expectApiError(() => parsePairOk({ token: "" }));
  expectApiError(() => parsePairOk({ token: 7 }));
});

// ------------------------------------------------------------ no leaking

test("sanitizeForDisplay strips token- and code-shaped secrets", () => {
  const token = "a".repeat(32) + "b".repeat(32); // 64 hex chars
  assert.ok(sanitizeForDisplay("token=" + token).includes("<redacted>"));
  assert.ok(!sanitizeForDisplay("token=" + token).includes("aaaa"));
  assert.ok(sanitizeForDisplay("code AB3D9K entered").includes("<redacted>"));
  assert.ok(sanitizeForDisplay("code AB3D-9K entered").includes("<redacted>"));
  // Ordinary prose with normal words survives untouched.
  assert.equal(
    sanitizeForDisplay("paired; watching paragraph changes"),
    "paired; watching paragraph changes",
  );
  // Six-letter lowercase words are not code-shaped (case matters).
  assert.equal(sanitizeForDisplay("running paired mode"), "running paired mode");
});
