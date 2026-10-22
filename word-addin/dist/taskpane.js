// ScholarScribe Word Assist — production task pane client (Phase 2).
//
// Serve model: this file is served BY the ScholarScribe engine from the
// same origin as the /v1 API, so `window.location.origin` IS the engine
// origin and no configuration travels with the page. All /v1 calls are
// same-origin fetches; the only cross-origin resource is office.js itself
// (CDN, never carries document text — ruling 5 / addin-webview class).
//
// Runtime requirements are probed, never assumed (recon V2/V5, spike
// experiments (f)/(a)): WordApi 1.6 = onParagraphChanged, 1.7 =
// annotations, 1.8 = popup actions. Anything missing degrades to list
// mode; nothing hard-fails.
//
// Offset units: the engine speaks UTF-16 code units on the wire (JS string
// semantics). The ADAPTER below is the single place that builds Word
// annotation payloads, so if the owner's spike run (experiment (a)) shows
// Word.Critique measures differently, only that helper changes.
//
// TS users: this file is plain JavaScript with @ts-check + JSDoc (the
// repo has no build step for the add-in; adding a bundler for one file
// was rejected in favor of editor-time strictness — recorded in
// docs/DECISIONS.md).
// @ts-check
"use strict";

/** @typedef {{ id: string, category: "grammar"|"spelling"|"clarity", rule_id: string,
 *   start: number, end: number, start_utf16: number, end_utf16: number,
 *   problem_text: string, message: string,
 *   suggestions: Array<{kind: string, text?: string}> }} AssistIssue */
/** @typedef {{ start: number, end: number, text: string,
 *   issues: AssistIssue[] }} Tier0Sentence */
/** @typedef {{ sentences: Tier0Sentence[],
 *   language_status: {status: string, script?: string},
 *   tier1?: Array<any> }} CheckResponse */

const ENGINE_ORIGIN = window.location.origin; // served by the engine
const DEBOUNCE_MS = 1200;        // typing quiet period before a check
const REPOLL_MS = 1500;          // waiting for Tier 1 pending verdicts
const REPOLL_MAX = 3;            // Tier 1 re-poll budget per check
const SELF_EVENT_GRACE_MS = 2000;// suppression watermark window (C4)
const CHECK_COOLDOWN_MS = 400;   // min gap between full-document checks

const els = {
  status: document.getElementById("status"),
  pairingCard: document.getElementById("pairingCard"),
  pairCode: document.getElementById("pairCode"),
  pairBtn: document.getElementById("pairBtn"),
  fallbackCard: document.getElementById("fallbackCard"),
  fallbackList: document.getElementById("fallbackList"),
  caps: document.getElementById("caps"),
  log: document.getElementById("log"),
  recheckBtn: document.getElementById("recheckBtn"),
  unpairBtn: document.getElementById("unpairBtn"),
  engineVersion: document.getElementById("engineVersion"),
};

/** @type {string | null} */ let token = null;
/** @type {boolean} */ let annotationsOk = false;
/** @type {boolean} */ let popupOk = false;
/** @type {boolean} */ let paragraphEventsOk = false;
/** @type {Map<string, string>} */ let lastKnownText = new Map(); // paraKey -> text
/** @type {number | null} */ let debounceTimer = null;
/** @type {boolean} */ let checking = false;
/** @type {number} */ let lastCheckAt = 0;
/** @type {Array<{start:number, end:number, kind:"self"}>} */ let watermark = [];
/** @type {Array<{paraId:string, sentenceStart:number, issue:AssistIssue}>} */ let liveIssues = [];

// ---------------------------------------------------------------- utils

/** @param {string} msg */
function log(msg) {
  const li = document.createElement("li");
  li.textContent = new Date().toTimeString().slice(0, 8) + " " + msg;
  els.log.appendChild(li);
  while (els.log.childElementCount > 40) els.log.removeChild(els.log.firstChild);
}

/**
 * @param {"ok"|"warn"|"err"} kind
 * @param {string} msg
 */
function setStatus(kind, msg) {
  els.status.className = "status " + kind;
  els.status.textContent = msg;
}

/** FNV-1a over a JS string (UTF-16 units) — cheap paragraph fingerprint. */
function fnv1a(str) {
  let h = 0x811c9dc5;
  for (let i = 0; i < str.length; i++) {
    h ^= str.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return (h >>> 0).toString(16);
}

/** @returns {Promise<any>} parsed JSON or throws Error(message from body) */
async function api(path, opts) {
  const res = await fetch(ENGINE_ORIGIN + path, opts);
  const body = await res.json().catch(() => ({}));
  if (!res.ok) {
    throw new Error(body && body.error ? body.error : "HTTP " + res.status);
  }
  return body;
}

function authHeaders() {
  return { Authorization: "Bearer " + (token || ""), "Content-Type": "application/json" };
}

// ---------------------------------------------------------------- pairing

async function loadToken() {
  try { token = window.localStorage.getItem("scholarscribe-assist-token"); } catch { token = null; }
}

async function pair(code) {
  const body = await api("/v1/pair", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ code }),
  });
  token = body.token;
  try { window.localStorage.setItem("scholarscribe-assist-token", token); } catch { /* private mode */ }
  log("paired");
  await refreshHealth();
  startWatching();
}

// ---------------------------------------------------------------- health

let healthTimer = null;
async function refreshHealth() {
  try {
    const h = await api("/v1/health");
    if (els.engineVersion) els.engineVersion.textContent = h.engine || "?";
    if (!token) {
      setStatus("warn", "Not paired. Enter the pairing code from ScholarScribe.");
      els.pairingCard.classList.remove("hidden");
      return;
    }
    setStatus("ok", h.paired
      ? "Connected to ScholarScribe (" + (h.strictness || "dev") + ")."
      : "Paired session expired — pair again.");
    els.pairingCard.classList.toggle("hidden", h.paired);
    els.unpairBtn.classList.toggle("hidden", !h.paired);
  } catch (e) {
    setStatus("err", "ScholarScribe is not reachable (" + e.message + "). Is the app running with Word Assist enabled?");
  }
}

// ---------------------------------------------------------------- caps

function probeCapabilities() {
  const supported = (v) => Office.context.requirements.isSetSupported("WordApi", v);
  paragraphEventsOk = supported(1.6);
  annotationsOk = supported(1.7);
  popupOk = supported(1.8);
  els.caps.textContent =
    "WordApi 1.6 (paragraph events): " + (paragraphEventsOk ? "yes" : "NO") +
    " · 1.7 (annotations): " + (annotationsOk ? "yes" : "NO — list mode") +
    " · 1.8 (popup actions): " + (popupOk ? "yes" : "NO — list mode");
  els.fallbackCard.classList.toggle("hidden", annotationsOk && popupOk);
  log("caps probed");
}

// ---------------------------------------------------------------- checks

/** Whole-document check: enumerate paragraphs, hash, diff, POST. */
async function checkDocument(force) {
  if (!token || checking) return;
  const now = Date.now();
  if (!force && now - lastCheckAt < CHECK_COOLDOWN_MS) return;
  lastCheckAt = now;
  checking = true;
  try {
    await Word.run(async (ctx) => {
      const paras = ctx.document.body.paragraphs;
      paras.load("items");
      await ctx.sync();
      /** @type {string[]} */
      const texts = [];
      for (const p of paras.items) {
        const t = p.insertText("", "End"); // no-op trick is fragile; use p.text
        texts.push(p.text || "");
      }
      await ctx.sync();
      // For each paragraph whose text changed since last known, check it.
      for (let i = 0; i < paras.items.length; i++) {
        const text = texts[i] || "";
        const key = "p" + i;
        const prev = lastKnownText.get(key);
        if (prev === text) continue;
        lastKnownText.set(key, text);
        if (text.trim().length === 0) continue;
        await checkParagraph(key, text);
      }
    });
  } catch (e) {
    log("check failed: " + e.message);
  } finally {
    checking = false;
  }
}

/** POST one paragraph; render tier0 + tier1 verdicts. */
async function checkParagraph(paragraphId, text) {
  /** @type {CheckResponse} */
  const resp = await api("/v1/check", {
    method: "POST",
    headers: authHeaders(),
    body: JSON.stringify({ paragraph_id: paragraphId, text, has_focus: true }),
  });
  liveIssues = liveIssues.filter((li) => li.paraId !== paragraphId);

  if (resp.language_status && resp.language_status.status === "unsupported") {
    setStatus("warn", "Language not supported yet (" + (resp.language_status.script || "?") +
      "). You can enable it experimentally in ScholarScribe → Word Assist.");
    return;
  }

  /** @type {AssistIssue[]} */
  const issues = [];
  for (const s of resp.sentences || []) {
    for (const issue of s.issues) {
      issues.push(issue);
      liveIssues.push({ paraId: paragraphId, sentenceStart: s.start, issue });
    }
    const t1 = s.tier1;
    if (t1 && t1.state === "ready") renderTier1(paragraphId, s, t1);
  }

  if (annotationsOk && popupOk) {
    await renderAnnotations(paragraphId, issues);
  } else {
    renderFallback(paragraphId, issues);
  }

  const pending = (resp.sentences || []).some((s) => s.tier1 && s.tier1.state === "pending");
  if (pending) {
    setTimeout(() => repoll(paragraphId, text), REPOLL_MS);
  }
}

/** Re-post the same paragraph to pick up Tier 1 verdicts (cheap: tier0 is cached server-side). */
async function repoll(paragraphId, text) {
  if (!token) return;
  try {
    await checkParagraph(paragraphId, text);
  } catch (e) {
    log("re-poll failed: " + e.message);
  }
}

// ------------------------------------------------- annotations (1.7/1.8)

/**
 * ADAPTER: builds the Word.Critique payload for one issue (spike-verified
 * shape: { start, length, colorScheme, popupOptions? }). start/length are
 * UTF-16 code units relative to the PARAGRAPH start (JS semantics); if the
 * owner's spike report (experiment (a)) proves Word measures differently,
 * this is the only function that changes.
 */
function toCritique(issue, paragraphBase) {
  const start = paragraphBase + issue.start_utf16;
  /** @type {any} */
  const critique = {
    start,
    length: issue.end_utf16 - issue.start_utf16,
    colorScheme: issue.category === "grammar" ? "Red" : "Blue",
  };
  if (popupOk) {
    critique.popupOptions = {
      suggestions: (issue.suggestions || [])
        .filter((s) => s.kind !== "remove")
        .map((s) => s.text || "")
        .filter((t) => t.length > 0)
        .slice(0, 3),
    };
  }
  return critique;
}

/** Render issues as Word annotations (1.7). Any failure falls back to list mode. */
async function renderAnnotations(paragraphId, issues) {
  if (!issues.length) return;
  const paraText = lastKnownText.get(paragraphId) || "";
  try {
    await Word.run(async (ctx) => {
      const paras = ctx.document.body.paragraphs;
      paras.load("items");
      await ctx.sync();
      for (const p of paras.items) {
        if ((p.text || "") !== paraText) continue;
        const critiques = issues.map((i) => toCritique(i, 0));
        // Spike-verified call (experiment (c)/(j)): paragraph-scoped.
        p.insertAnnotations(critiques);
        await ctx.sync();
        log("annotations rendered: " + critiques.length);
        return;
      }
      log("paragraph not found for annotations; using list mode");
      renderFallback(paragraphId, issues);
    });
  } catch (e) {
    log("annotation render failed (" + e.message + "); using list mode");
    renderFallback(paragraphId, issues);
  }
}

/** Popup actions (1.8): apply the chosen suggestion, report, re-check. */
async function attachPopupHandler() {
  if (!popupOk) return;
  try {
    await Word.run(async (ctx) => {
      await ctx.document.onAnnotationPopupAction.add(async (args) => {
        log("popup action: " + JSON.stringify(args));
        const li = liveIssues.find((x) => args && String(args.id).endsWith(x.issue.id));
        const suggestion = args && args.critiqueSuggestion;
        if (li && suggestion) {
          await applyInlineEdit(li.paraId, li.issue, { kind: "replaceWith", text: String(suggestion) });
        }
      });
      await ctx.sync();
      log("popup handler attached");
    });
  } catch (e) {
    log("popup handler unavailable (" + e.message + "); list mode stays available");
  }
}

// ------------------------------------------------------- fallback (list)

function renderFallback(paragraphId, issues) {
  els.fallbackList.textContent = "";
  if (!issues.length) {
    els.fallbackList.textContent = "No issues in the changed paragraph.";
    return;
  }
  for (const issue of issues) {
    const div = document.createElement("div");
    div.className = "issue";
    const cat = document.createElement("div");
    cat.className = "cat " + issue.category;
    cat.textContent = issue.category;
    const problem = document.createElement("div");
    problem.className = "problem";
    problem.textContent = "“" + issue.problem_text + "”";
    const msg = document.createElement("div");
    msg.className = "message";
    msg.textContent = issue.message;
    div.appendChild(cat); div.appendChild(problem); div.appendChild(msg);
    for (const sug of issue.suggestions || []) {
      const chip = document.createElement("span");
      chip.className = "chip";
      chip.textContent = sug.kind === "remove" ? "Remove" : (sug.text || "");
      chip.onclick = () => applyInlineEdit(paragraphId, issue, sug);
      div.appendChild(chip);
    }
    els.fallbackList.appendChild(div);
  }
  els.fallbackCard.classList.remove("hidden");
}

/** List-mode apply: replace the flagged range inside the paragraph text. */
async function applyInlineEdit(paragraphId, issue, suggestion) {
  const text = lastKnownText.get(paragraphId) || "";
  const startU16 = issue.start_utf16;
  const endU16 = issue.end_utf16;
  const replacement = suggestion.kind === "remove" ? "" : (suggestion.text || "");
  const next = text.slice(0, startU16) + replacement + text.slice(endU16);
  watermark.push({ start: startU16, end: startU16 + replacement.length, at: Date.now(), kind: "self" });
  try {
    await Word.run(async (ctx) => {
      // Find the paragraph by matching its current text, then rewrite it.
      const paras = ctx.document.body.paragraphs;
      paras.load("items");
      await ctx.sync();
      for (const p of paras.items) {
        if ((p.text || "") === text) {
          p.insertText(next, "Replace");
          break;
        }
      }
      await ctx.sync();
    });
    lastKnownText.set(paragraphId, next);
    await api("/v1/accepted", {
      method: "POST",
      headers: authHeaders(),
      body: JSON.stringify({
        paragraph_id: paragraphId,
        issue_id: issue.id,
        rule_id: issue.rule_id,
        category: issue.category,
        applied_start: startU16,
        applied_end: endU16,
        replacement_len: replacement.length,
        paragraph_text_hash: fnv1a(text),
      }),
    });
    log("accepted: " + issue.id);
    checkDocument(true);
  } catch (e) {
    log("apply failed: " + e.message);
  }
}

// ------------------------------------------------------------- tier 1

function renderTier1(paragraphId, sentence, t1) {
  if (!t1.suggestion) return;
  const sug = t1.suggestion;
  const box = document.createElement("div");
  box.className = "tier1";
  const label = document.createElement("div");
  label.textContent = "Rewrite available" + (sug.explanation ? " — " + sug.explanation : "");
  const chip = document.createElement("span");
  chip.className = "chip rewrite";
  chip.textContent = "Apply rewrite";
  chip.onclick = () => applyTier1Edits(paragraphId, sentence, sug);
  box.appendChild(label);
  box.appendChild(chip);
  els.fallbackCard.classList.remove("hidden");
  els.fallbackList.appendChild(box);
}

/** Apply a Tier 1 rewrite: right-to-left word edits, then re-check. */
async function applyTier1Edits(paragraphId, sentence, sug) {
  const paraText = lastKnownText.get(paragraphId) || "";
  // Sentence-relative scalar edits → paragraph-relative UTF-16 edits.
  const sentenceU16Start = paraText.slice(0, sentence.start).length;
  const edits = (sug.edits || []).slice().sort((a, b) => b.start - a.start);
  const map = new OffsetMap(paraText);
  let next = paraText;
  /** @type {Array<[number, number]>} */ let applied = [];
  for (const e of edits) {
    const s = sentenceU16Start + map.scalarToUtf16(e.start);
    const en = sentenceU16Start + map.scalarToUtf16(e.end);
    next = next.slice(0, s) + e.replacement + next.slice(en);
    applied.push([s, s + e.replacement.length]);
  }
  watermark.push({ start: Math.min(...applied.map((a) => a[0]), Infinity), end: Math.max(...applied.map((a) => a[1]), 0), at: Date.now(), kind: "self" });
  try {
    await Word.run(async (ctx) => {
      const paras = ctx.document.body.paragraphs;
      paras.load("items");
      await ctx.sync();
      for (const p of paras.items) {
        if ((p.text || "") === paraText) {
          p.insertText(next, "Replace");
          break;
        }
      }
      await ctx.sync();
    });
    lastKnownText.set(paragraphId, next);
    await api("/v1/accepted", {
      method: "POST",
      headers: authHeaders(),
      body: JSON.stringify({
        paragraph_id: paragraphId,
        issue_id: "tier1:" + fnv1a(sug.original),
        rule_id: "tier1",
        category: "clarity",
        applied_start: sentenceU16Start,
        applied_end: sentenceU16Start + paraText.slice(sentence.start).length,
        replacement_len: sug.revised.length,
        paragraph_text_hash: fnv1a(paraText),
      }),
    });
    checkDocument(true);
  } catch (e) {
    log("rewrite failed: " + e.message);
  }
}

/** Minimal scalar→UTF-16 map (mirror of assist/offsets.rs). */
class OffsetMap {
  /** @param {string} text */
  constructor(text) {
    /** @type {number[]} */ this.prefix = [0];
    let acc = 0;
    for (const ch of text) {
      const cp = ch.codePointAt(0) || 0;
      acc += cp > 0xffff ? 2 : 1;
      this.prefix.push(acc);
    }
  }
  /** @param {number} scalar */
  scalarToUtf16(scalar) {
    return this.prefix[Math.min(scalar, this.prefix.length - 1)];
  }
}

// ------------------------------------------------------------- watching

function scheduleCheck() {
  if (debounceTimer) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => checkDocument(false), DEBOUNCE_MS);
}

function isSelfEvent(rangeStart, rangeEnd) {
  const now = Date.now();
  watermark = watermark.filter((w) => now - w.at < SELF_EVENT_GRACE_MS);
  return watermark.some((w) => rangeStart < w.end && rangeEnd > w.start);
}

async function startWatching() {
  if (!paragraphEventsOk) {
    log("paragraph events unavailable (WordApi 1.6); manual re-check only");
    setStatus("ok", "Connected (manual mode: use Re-check).");
    return;
  }
  Office.context.document.addHandlerAsync(
    Office.EventType.DocumentParagraphChanged ?? "documentParagraphChanged",
    () => scheduleCheck(),
    (asyncResult) => {
      if (asyncResult.status === Office.AsyncResultStatus.Succeeded) {
        log("watching paragraph changes");
        checkDocument(true);
        attachPopupHandler();
      } else {
        log("could not attach paragraph listener: " + asyncResult.error.message);
        setStatus("warn", "Connected, but live watching is unavailable. Use Re-check.");
      }
    }
  );
}

// ---------------------------------------------------------------- boot

Office.onReady(() => {
  probeCapabilities();
  loadToken().then(refreshHealth);
  els.pairBtn.addEventListener("click", () => {
    const code = (els.pairCode.value || "").trim().toUpperCase();
    if (code.length !== 6) { setStatus("warn", "The pairing code is 6 characters."); return; }
    els.pairBtn.disabled = true;
    pair(code).catch((e) => {
      setStatus("err", "Pairing failed: " + e.message);
      els.pairBtn.disabled = false;
    });
  });
  els.recheckBtn.addEventListener("click", () => checkDocument(true));
  els.unpairBtn.addEventListener("click", async () => {
    try { window.localStorage.removeItem("scholarscribe-assist-token"); } catch { /* ignore */ }
    token = null;
    els.unpairBtn.classList.add("hidden");
    await refreshHealth();
  });
  setInterval(refreshHealth, 30000);
});
