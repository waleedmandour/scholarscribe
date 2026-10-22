// A2 — keys, timing, and watermark unit tests.

import { test } from "node:test";
import assert from "node:assert/strict";
import { paragraphKeyByPosition, paragraphFingerprint, sentenceKey } from "../src/keys.js";
import { Cooldown, Debouncer, Epoch, RepollBudget } from "../src/timing.js";
import { Watermark } from "../src/watermark.js";

// ----------------------------------------------------------------- keys

test("paragraph keys are positional and stable for the same index", () => {
  assert.equal(paragraphKeyByPosition(0), "p0");
  assert.equal(paragraphKeyByPosition(12), "p12");
});

test("sentence keys combine paragraph key and sentence start", () => {
  assert.equal(sentenceKey("p3", 21), "p3:21");
});

test("fingerprints distinguish text changes under the same key", () => {
  const a = paragraphFingerprint("p0", "hello world");
  const b = paragraphFingerprint("p0", "hello worlds");
  assert.notEqual(a, b);
  assert.equal(paragraphFingerprint("p0", "hello world"), a);
});

test("fingerprints distinguish the same text under different keys", () => {
  assert.notEqual(paragraphFingerprint("p0", "same"), paragraphFingerprint("p1", "same"));
});

// --------------------------------------------------------------- timing

test("Debouncer fires once after the quiet period", () => {
  let fired = 0;
  const d = new Debouncer(50);
  d.schedule(() => {
    fired += 1;
  });
  d.schedule(() => {
    fired += 1;
  }); // re-arm cancels the first
  assert.equal(d.pending, true);
  setTimeout(() => {
    assert.equal(fired, 1);
    assert.equal(d.pending, false);
  }, 70);
});

test("Debouncer cancel prevents firing", () => {
  let fired = 0;
  const d = new Debouncer(30);
  d.schedule(() => {
    fired += 1;
  });
  assert.equal(d.cancel(), true);
  assert.equal(d.cancel(), false);
  assert.equal(d.pending, false);
  setTimeout(() => {
    assert.equal(fired, 0);
  }, 50);
});

test("Epoch discards stale observations", () => {
  const e = new Epoch();
  const s1 = e.bump();
  const s2 = e.bump();
  assert.ok(e.isCurrent(s2));
  assert.ok(!e.isCurrent(s1));
});

test("Cooldown gates by gap and reset forces admission", () => {
  const c = new Cooldown(400);
  assert.ok(c.admit(0));
  assert.ok(!c.admit(399));
  assert.ok(c.admit(400));
  c.reset();
  assert.ok(c.admit(401));
});

test("RepollBudget admits at most max times", () => {
  const b = new RepollBudget(3, 1500);
  assert.equal(b.admit(), 1500);
  assert.equal(b.admit(), 1500);
  assert.equal(b.admit(), 1500);
  assert.equal(b.admit(), null);
  assert.equal(b.spent, true);
  b.reset();
  assert.equal(b.admit(), 1500);
});

// ------------------------------------------------------------ watermark

test("watermark suppresses overlapping self-edits inside the grace window", () => {
  const w = new Watermark(2000);
  w.push(10, 14, 100);
  assert.ok(w.isSelfEvent(12, 16, 150)); // overlap
  assert.ok(w.isSelfEvent(0, 11, 150)); // left edge
  assert.ok(!w.isSelfEvent(14, 20, 150)); // right of range (half-open)
  assert.ok(!w.isSelfEvent(5, 9, 150)); // left of range
});

test("watermark expires after the grace window", () => {
  const w = new Watermark(2000);
  w.push(10, 14, 100);
  assert.ok(!w.isSelfEvent(10, 14, 2101)); // > graceMs later
});

test("watermark pruning removes only expired entries", () => {
  const w = new Watermark(2000);
  w.push(0, 2, 0);
  w.push(4, 6, 1500);
  assert.ok(w.isSelfEvent(4, 6, 2100)); // second entry still fresh
  assert.equal(w.size, 1); // first was pruned
});
