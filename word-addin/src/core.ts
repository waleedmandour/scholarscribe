// ScholarScribe Word Assist — pure core types and helpers.
//
// This module carries NO Office.js references: everything here runs under
// Node's test runner unchanged (A2). The orchestrator (taskpane.ts) is the
// only module allowed to touch Office/Word/DOM.
//
// Offset units (spike experiment (a), still NOT VERIFIED): the engine
// speaks UTF-16 code units on the wire — JS string semantics. Where Word's
// Critique payload units are unknown we keep the single adapter
// (toCritique in the orchestrator) as the only place that changes; the
// mode lives in OFFSET_UNIT_MODE so Stage B flips a config, not a rewrite.

/** Category labels shared with the engine wire format. */
export type IssueCategory = "grammar" | "spelling" | "clarity";

/** One assistant finding, exactly as /v1/check reports it. */
export interface AssistIssue {
  id: string;
  category: IssueCategory;
  rule_id: string;
  start: number;
  end: number;
  start_utf16: number;
  end_utf16: number;
  problem_text: string;
  message: string;
  suggestions: Array<{ kind: string; text?: string }>;
}

/** One checked sentence with its issues and optional Tier 1 verdict. */
export interface Tier0Sentence {
  start: number;
  end: number;
  text: string;
  issues: AssistIssue[];
  /** Present once the engine's Tier 1 scheduler has a verdict for this
   *  sentence; "pending" means re-poll (see timing.RepollBudget). */
  tier1?: Tier1Verdict;
}

/** Tier 1 verdict shape (state machine: pending → ready/rejected). */
export interface Tier1Verdict {
  state: "pending" | "ready" | "rejected";
  suggestion?: {
    original: string;
    revised: string;
    explanation?: string;
    edits: Array<{ start: number; end: number; replacement: string }>;
  };
}

/** /v1/check response body. */
export interface CheckResponse {
  sentences: Tier0Sentence[];
  language_status: { status: string; script?: string };
  tier1?: Tier1Verdict[];
}

/** /v1/accepted request body (counts and hashes only — never text). */
export interface AcceptedReport {
  paragraph_id: string;
  issue_id: string;
  rule_id: string;
  category: string;
  applied_start: number;
  applied_end: number;
  replacement_len: number;
  paragraph_text_hash: string;
}

/** Offset-unit mode selected for Word annotation payloads (spike (a)).
 *  "utf16": Word.Critique.start/length count UTF-16 code units relative
 *           to the paragraph (JS string semantics). Current default.
 *  "scalar": Word counts Unicode scalar values — Stage B flips this one
 *            constant after the owner's spike run proves it. */
export type OffsetUnitMode = "utf16" | "scalar";

/** The one configuration Stage B is expected to change (spike (a)). */
export const OFFSET_UNIT_MODE: OffsetUnitMode = "utf16";

/** FNV-1a over a JS string (UTF-16 units) — cheap paragraph fingerprint.
 *  Mirrors the wire contract used for paragraph_text_hash; NOT a security
 *  hash (the engine never treats it as one). */
export function fnv1a(str: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < str.length; i++) {
    h ^= str.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return (h >>> 0).toString(16);
}

/**
 * Scalar-offset → UTF-16-offset map for one fixed string (mirror of
 * assist/offsets.rs). Tier 1 edits arrive in Unicode scalar offsets
 * (engine semantics); DOM/Word string indices are UTF-16. Out-of-range
 * scalar offsets clamp; a lone surrogate boundary snaps to its owner.
 */
export class OffsetMap {
  private readonly prefix: number[];

  constructor(text: string) {
    this.prefix = [0];
    let acc = 0;
    for (const ch of text) {
      const cp = ch.codePointAt(0) || 0;
      acc += cp > 0xffff ? 2 : 1;
      this.prefix.push(acc);
    }
  }

  /** Map a scalar (code point) offset to a UTF-16 offset. Clamps. */
  scalarToUtf16(scalar: number): number {
    if (scalar <= 0) return 0;
    const last = this.prefix.length - 1;
    if (scalar >= last) return this.prefix[last] ?? 0;
    return this.prefix[scalar] ?? 0;
  }

  /** Total UTF-16 length of the mapped text. */
  utf16Length(): number {
    return this.prefix[this.prefix.length - 1] ?? 0;
  }
}
