// A2 — apply-flow tests: the "applies exactly once" guarantee is proven
// here at the pure layer (planInlineEdit/planTier1Edits), which the
// orchestrator must call before every Word.write.

import { test } from "node:test";
import assert from "node:assert/strict";
import { planInlineEdit, planTier1Edits } from "../src/apply.js";
import { OffsetMap, type AssistIssue } from "../src/core.js";

function issue(over: Partial<AssistIssue>): AssistIssue {
  return {
    id: "i1",
    category: "grammar",
    rule_id: "Rule",
    start: 0,
    end: 3,
    start_utf16: 0,
    end_utf16: 3,
    problem_text: "teh",
    message: "Typo",
    suggestions: [{ kind: "replaceWith", text: "the" }],
    ...over,
  };
}

test("planInlineEdit replaces exactly the flagged range once", () => {
  const text = "teh report is late";
  const plan = planInlineEdit("p0", text, issue({}), { kind: "replaceWith", text: "the" });
  assert.equal(plan.nextText, "the report is late");
  assert.equal(plan.watermarkStart, 0);
  assert.equal(plan.watermarkEnd, 3);
  assert.equal(plan.report.paragraph_text_hash.length, 8); // fnv1a hex
  assert.equal(plan.report.replacement_len, 3);
  assert.equal(plan.report.applied_start, 0);
  assert.equal(plan.report.applied_end, 3);
  // The report must never carry text.
  assert.ok(!JSON.stringify(plan.report).includes("teh"));
});

test("planInlineEdit remove-suggestion produces an empty splice", () => {
  const text = "very very bad";
  const plan = planInlineEdit(
    "p1",
    text,
    issue({ start_utf16: 5, end_utf16: 9, problem_text: "very" }),
    { kind: "remove" },
  );
  assert.equal(plan.nextText, "very  bad"); // range removed, spaces remain
  assert.equal(plan.watermarkEnd, plan.watermarkStart); // empty watermark range
});

test("planInlineEdit refuses stale offsets (wrong problem text)", () => {
  assert.throws(() =>
    planInlineEdit("p0", "totally different", issue({}), { kind: "replaceWith", text: "the" }),
  );
});

test("planInlineEdit refuses out-of-range offsets", () => {
  assert.throws(() =>
    planInlineEdit("p0", "abc", issue({ start_utf16: 1, end_utf16: 99 }), {
      kind: "replaceWith",
      text: "x",
    }),
  );
});

test("planInlineEdit refuses inverted ranges", () => {
  assert.throws(() =>
    planInlineEdit("p0", "abc", issue({ start_utf16: 2, end_utf16: 1, problem_text: "" }), {
      kind: "replaceWith",
      text: "x",
    }),
  );
});

test("planTier1Edits applies right-to-left over astral text", () => {
  // Paragraph with an emoji BEFORE the sentence so paragraph-UTF16 and
  // sentence-scalar offsets diverge: every edit must land on the right
  // character, proving the double offset conversion.
  const sentence = "\u{1F600} sayed hello"; // emoji + space + text
  const paragraph = "Intro " + sentence;
  const sentenceStartU16 = 6; // "Intro " is 6 UTF-16 units
  const map = new OffsetMap(sentence);
  const plan = planTier1Edits(
    "p2",
    paragraph,
    sentenceStartU16,
    sentence,
    {
      original: "\u{1F600} sayed hello",
      revised: "\u{1F600} said hello",
      edits: [
        { start: 2, end: 7, replacement: "said" }, // scalar offsets in sentence
      ],
    },
    map,
  );
  assert.equal(plan.nextText, "Intro \u{1F600} said hello");
  assert.ok(plan.report.issue_id.startsWith("tier1:"));
  // The report describes the whole sentence rewrite (original client
  // semantics): sentence start → sentence end (UTF-16), revised length.
  assert.equal(plan.report.applied_start, sentenceStartU16);
  assert.equal(plan.report.applied_end, sentenceStartU16 + 14); // emoji(2)+space+"sayed hello"
  assert.equal(plan.report.replacement_len, "\u{1F600} said hello".length);
});

test("planTier1Edits handles multiple edits without offset drift", () => {
  const sentence = "one two three";
  const plan = planTier1Edits(
    "p3",
    sentence,
    0,
    sentence,
    {
      original: sentence,
      revised: "ONE two THREE",
      edits: [
        { start: 0, end: 3, replacement: "ONE" },
        { start: 8, end: 13, replacement: "THREE" },
      ],
    },
    new OffsetMap(sentence),
  );
  assert.equal(plan.nextText, "ONE two THREE");
  assert.equal(plan.watermarkStart, 0);
  assert.equal(plan.watermarkEnd, 13);
});

test("planTier1Edits refuses a sentence that changed", () => {
  assert.throws(() =>
    planTier1Edits(
      "p4",
      "different text",
      0,
      "different text",
      { original: "original", revised: "r", edits: [] },
      new OffsetMap("different text"),
    ),
  );
});
