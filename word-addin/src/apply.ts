// ScholarScribe Word Assist — apply flow, pure part.
//
// Everything about *computing* an edit lives here so tests can prove
// "applies exactly once" byte-for-byte. The orchestrator owns the Office.js
// side (finding the paragraph, insertText, then re-check) and calls these
// before/after the write.

import { fnv1a, type AcceptedReport, type AssistIssue } from "./core.js";

/** The exact single application of one suggestion to paragraph text. */
export interface PlannedEdit {
  /** New paragraph text after the replacement. */
  nextText: string;
  /** UTF-16 range the replacement occupies in the NEW text (for the
   *  suppression watermark). */
  watermarkStart: number;
  watermarkEnd: number;
  /** Report payload describing what was applied (counts + hash only). */
  report: AcceptedReport;
}

/**
 * Plan one inline edit (list-mode chip or popup suggestion). Pure:
 * throws RangeError when the issue no longer matches the text it is
 * applied to (the document moved on) — the orchestrator surfaces that as
 * a stale-result discard, never as a wrong edit.
 */
export function planInlineEdit(
  paragraphId: string,
  text: string,
  issue: AssistIssue,
  suggestion: { kind: string; text?: string },
): PlannedEdit {
  const startU16 = issue.start_utf16;
  const endU16 = issue.end_utf16;
  if (startU16 < 0 || endU16 > text.length || startU16 > endU16) {
    throw new RangeError("issue range no longer matches paragraph text");
  }
  const problem = text.slice(startU16, endU16);
  // The flagged text must still be what the engine saw; otherwise the
  // offsets are stale and applying would corrupt the paragraph.
  if (problem !== issue.problem_text) {
    throw new RangeError("flagged text changed since the check");
  }
  const replacement = suggestion.kind === "remove" ? "" : suggestion.text || "";
  const nextText = text.slice(0, startU16) + replacement + text.slice(endU16);
  return {
    nextText,
    watermarkStart: startU16,
    watermarkEnd: startU16 + replacement.length,
    report: {
      paragraph_id: paragraphId,
      issue_id: issue.id,
      rule_id: issue.rule_id,
      category: issue.category,
      applied_start: startU16,
      applied_end: endU16,
      replacement_len: replacement.length,
      paragraph_text_hash: fnv1a(text),
    },
  };
}

/**
 * Plan a Tier 1 rewrite. The engine's suggestion edits are Unicode
 * SCALAR offsets relative to the SENTENCE; paragraph text is UTF-16 and
 * the sentence starts at `sentenceStartU16`. Right-to-left application
 * keeps earlier offsets valid while later ones are spliced.
 */
export function planTier1Edits(
  paragraphId: string,
  paragraphText: string,
  sentenceStartU16: number,
  sentenceText: string,
  suggestion: { original: string; revised: string; edits: Array<{ start: number; end: number; replacement: string }> },
  offsetMap: { scalarToUtf16(scalar: number): number },
): PlannedEdit {
  if (sentenceText.slice(0, suggestion.original.length) !== suggestion.original) {
    throw new RangeError("sentence no longer starts with the Tier 1 original");
  }
  const edits = suggestion.edits.slice().sort((a, b) => b.start - a.start);
  let next = paragraphText;
  let minStart = Infinity;
  let maxEnd = 0;
  for (const e of edits) {
    const s = sentenceStartU16 + offsetMap.scalarToUtf16(e.start);
    const en = sentenceStartU16 + offsetMap.scalarToUtf16(e.end);
    if (s < 0 || en > next.length || s > en) {
      throw new RangeError("Tier 1 edit range outside paragraph text");
    }
    next = next.slice(0, s) + e.replacement + next.slice(en);
    minStart = Math.min(minStart, s);
    maxEnd = Math.max(maxEnd, s + e.replacement.length);
  }
  return {
    nextText: next,
    watermarkStart: minStart === Infinity ? sentenceStartU16 : minStart,
    watermarkEnd: maxEnd,
    report: {
      paragraph_id: paragraphId,
      issue_id: "tier1:" + fnv1a(suggestion.original),
      rule_id: "tier1",
      category: "clarity",
      applied_start: sentenceStartU16,
      applied_end: sentenceStartU16 + offsetMap.scalarToUtf16(suggestion.original.length),
      replacement_len: suggestion.revised.length,
      paragraph_text_hash: fnv1a(paragraphText),
    },
  };
}
