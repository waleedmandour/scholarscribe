// ScholarScribe Word Assist — paragraph and sentence keys.
//
// A paragraph key identifies one paragraph across checks inside one pane
// session. The current strategy is positional ("p" + body index), which is
// what the Phase 2 client shipped and what the engine's ledger aggregates
// on. Positional keys are stable only between edits that do not insert or
// delete paragraphs; a real insertion shifts every following key. That is
// ACCEPTED for v0.1 (checks are whole-document and re-derived every pass;
// the worst case is one extra check per shifted paragraph), and the
// strategy is isolated here so spike items (A8 paragraph identity /
// uniqueLocalId) can replace it without touching the client flow.
//
// Sentence keys combine the paragraph key with the sentence's UTF-16
// start offset, which the engine guarantees are relative to the
// paragraph text it was sent.
import { fnv1a } from "./core.js";
/** Key for the paragraph at body position `index` (0-based). */
export function paragraphKeyByPosition(index) {
    return "p" + index;
}
/** Key for one sentence inside a paragraph (paragraph key + UTF-16 start). */
export function sentenceKey(paragraph, sentenceStartU16) {
    return paragraph + ":" + sentenceStartU16;
}
/**
 * Identity of a paragraph for the "did this text change" decision: the
 * FNV-1a of its text plus the positional key. Two calls with the same
 * result mean "skip" — the pure predicate behind checkDocument's diff.
 */
export function paragraphFingerprint(key, text) {
    return key + "@" + fnv1a(text);
}
