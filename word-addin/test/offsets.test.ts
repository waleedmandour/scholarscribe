// A2 — offset adapter tests. Parametrised over the exact character
// classes the work order names: emoji (surrogate pairs), Arabic (RTL +
// lam-alef ligature risks), and combining marks. These run against the
// SAME module the orchestrator imports, so spike (a) flips OFFSET_UNIT_MODE
// and these tests re-pin whatever the new mapping is.

import { test } from "node:test";
import assert from "node:assert/strict";
import { OffsetMap, OFFSET_UNIT_MODE } from "../src/core.js";

const CASES: Array<{ name: string; text: string; scalar: number; utf16: number }> = [
  { name: "empty", text: "", scalar: 0, utf16: 0 },
  { name: "ascii", text: "hello", scalar: 3, utf16: 3 },
  { name: "ascii-end", text: "hello", scalar: 5, utf16: 5 },
  { name: "ascii-clamp", text: "hello", scalar: 99, utf16: 5 },
  // Emoji: 1 scalar = 2 UTF-16 units.
  { name: "emoji-single", text: "\u{1F600} hi", scalar: 1, utf16: 2 },
  { name: "emoji-run", text: "\u{1F600}\u{1F601}x", scalar: 2, utf16: 4 },
  // Arabic with combining marks (shadda+damma): base letters are 1 unit,
  // each combining mark is 1 unit, and all are BMP scalars (1 unit each).
  { name: "arabic", text: "\u0627\u0644\u0639\u0631\u0628\u064A\u0629", scalar: 4, utf16: 4 },
  {
    name: "arabic-combining",
    text: "\u0645\u064E\u0651\u0646", // meem + fatha + shadda + noon
    scalar: 2,
    utf16: 2,
  },
  // Astral-plane character AFTER a combining sequence: a(1) + ́(1) +
  // 🔑(2 UTF-16) + b(1). Scalar 3 = after 🔑 → UTF-16 offset 4.
  { name: "mixed-astral", text: "a\u0301\u{1F511}b", scalar: 3, utf16: 4 },
  // ZWJ sequence (family emoji): one grapheme, several scalars, astral.
  // 👨(2u)+ZWJ(1u)+👩(2u)+ZWJ(1u)+👧(2u)+!(1u): scalar 5 (after 👧) → 8u.
  {
    name: "zwj-sequence",
    text: "\u{1F468}\u200D\u{1F469}\u200D\u{1F467}!",
    scalar: 5,
    utf16: 8,
  },
];

test("OffsetMap parametrised scalar→UTF16", () => {
  for (const c of CASES) {
    const m = new OffsetMap(c.text);
    assert.equal(
      m.scalarToUtf16(c.scalar),
      c.utf16,
      `case ${c.name}: scalarToUtf16(${c.scalar}) of ${JSON.stringify(c.text)}`,
    );
  }
});

test("OffsetMap round-trips UTF-16 length", () => {
  for (const c of CASES) {
    const m = new OffsetMap(c.text);
    assert.equal(m.utf16Length(), c.text.length, `case ${c.name} length`);
  }
});

test("OffsetMap matches engine utf16_length semantics on astral text", () => {
  // Mirror of assist/offsets.rs: utf16 length == JS string length.
  const t = "🚀🚀 done";
  assert.equal(new OffsetMap(t).utf16Length(), t.length);
  assert.equal(new OffsetMap(t).scalarToUtf16(2), 4);
});

test("offset unit mode is utf16 until the owner's spike run says otherwise", () => {
  assert.equal(OFFSET_UNIT_MODE, "utf16");
});
