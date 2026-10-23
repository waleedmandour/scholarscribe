// ScholarScribe Word Assist — production task pane client (A2 port).
//
// Serve model: this file is served BY the ScholarScribe engine from the
// same origin as the /v1 API, so `window.location.origin` IS the engine
// origin and no configuration travels with the page. All /v1 calls are
// same-origin fetches; the only cross-origin resource is office.js itself
// (CDN, never carries document text — ruling 5 / addin-webview class).
//
// A2 structure: every pure decision (offsets, keys, debounce/stale
// discard, suppression watermark, apply planning, mode selection, error
// wording) lives in a dedicated module under word-addin/src/ and is unit-
// tested with Node's test runner. This file is the only module that
// touches Office.js, Word.js and the DOM, so the mock-driven tests cover
// everything except the host calls themselves.
//
// Offset units: the engine speaks UTF-16 code units on the wire (JS
// string semantics). The toCritique adapter below is the single place
// that builds Word annotation payloads; its unit mode is the
// OFFSET_UNIT_MODE constant in core.ts (spike (a) flips a config, not a
// rewrite).
//
// Build: `npm run addin:build` (tsc, UMD, single file — word-addin/dist/
// taskpane.js, byte-identical output for the same inputs; CI compares the
// committed dist against a fresh build). TypeScript is already a repo
// devDependency; no bundler was added (recorded in docs/DECISIONS.md).
import { OffsetMap, OFFSET_UNIT_MODE } from "./core.js";
import { paragraphKeyByPosition, paragraphFingerprint } from "./keys.js";
import { Cooldown, Debouncer, Epoch, RepollBudget } from "./timing.js";
import { Watermark } from "./watermark.js";
import { planInlineEdit, planTier1Edits } from "./apply.js";
import { capsLine, hasLiveWatching, selectMode, usesAnnotations, } from "./fallback.js";
import { errorStatus, healthStatus, unsupportedLanguage } from "./errors.js";
import { ApiError, classifyCheckFailure, classifyPairFailure, parsePairOk, sanitizeForDisplay, } from "./pairing.js";
// ------------------------------------------------------------- constants
const ENGINE_ORIGIN = window.location.origin; // served by the engine
const DEBOUNCE_MS = 1200; // typing quiet period before a check
const REPOLL_MS = 1500; // waiting for Tier 1 pending verdicts
const REPOLL_MAX = 3; // Tier 1 re-poll budget per check
const SELF_EVENT_GRACE_MS = 2000; // suppression watermark window
const CHECK_COOLDOWN_MS = 400; // min gap between full-document checks
const HEALTH_POLL_MS = 30000;
/** Non-null typed element lookup; every id is guaranteed by
 *  taskpane.html and checked once at boot (a missing element is a build
 *  bug, not a runtime condition). */
function requireEl(id) {
    const el = document.getElementById(id);
    if (el === null)
        throw new Error("taskpane.html is missing element #" + id);
    return el;
}
// ------------------------------------------------------------------ state
let token = null;
let mode = "manual-list";
let checking = false;
/** paragraph key → fingerprint (key + text hash); fingerprints map to the
 *  text itself. Two entries per paragraph, one lookup each way. */
const lastKnownText = new Map();
const debounce = new Debouncer(DEBOUNCE_MS);
const cooldown = new Cooldown(CHECK_COOLDOWN_MS);
/** Epoch per paragraph: bumped whenever new text is observed, so a Tier 1
 *  re-poll for text that has since changed is discarded, never rendered. */
const paragraphEpoch = new Map();
const watermark = new Watermark(SELF_EVENT_GRACE_MS);
/** Issues still live in the pane, for popup-action correlation. */
let liveIssues = [];
// ----------------------------------------------------------------- utils
function log(msg) {
    const logEl = requireEl("log");
    const li = document.createElement("li");
    // sanitizeForDisplay: pairing codes and bearer tokens never reach the
    // pane log even if a future call site tries to log them (A2×H1/H2).
    li.textContent = new Date().toTimeString().slice(0, 8) + " " + sanitizeForDisplay(msg);
    logEl.appendChild(li);
    while (logEl.childElementCount > 40)
        logEl.removeChild(logEl.firstChild);
}
function setStatus(state) {
    const statusEl = requireEl("status");
    statusEl.className = "status " + state.kind;
    statusEl.textContent = state.message;
}
/** @returns parsed JSON or throws ApiError(status, server message) */
async function api(path, opts) {
    const res = await fetch(ENGINE_ORIGIN + path, opts);
    const body = await res.json().catch(() => ({}));
    if (!res.ok) {
        const msg = body && typeof body === "object" && "error" in body
            ? String(body.error)
            : "HTTP " + res.status;
        throw new ApiError(res.status, msg);
    }
    return body;
}
function authHeaders() {
    return {
        Authorization: "Bearer " + (token || ""),
        "Content-Type": "application/json",
    };
}
// --------------------------------------------------------------- pairing
async function loadToken() {
    try {
        token = window.localStorage.getItem("scholarscribe-assist-token");
    }
    catch {
        token = null; // private mode: pair per session
    }
}
async function pair(code) {
    const body = await api("/v1/pair", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ code }),
    });
    // parsePairOk: throws ApiError when the engine returned no token.
    token = parsePairOk(body);
    try {
        window.localStorage.setItem("scholarscribe-assist-token", token);
    }
    catch {
        /* private mode: session-only pairing */
    }
    log("paired");
    await refreshHealth();
    startWatching();
}
/** The token was revoked, rotated (H2) or expired (401 from a /v1 call):
 *  clear it everywhere and put the pane back into the re-pair state. */
function clearTokenAndRequirePairing(message) {
    token = null;
    try {
        window.localStorage.removeItem("scholarscribe-assist-token");
    }
    catch {
        /* ignore */
    }
    requireEl("pairingCard").classList.remove("hidden");
    requireEl("unpairBtn").classList.add("hidden");
    setStatus({ kind: "warn", message });
}
/** Single router for engine failures on /v1/check and /v1/accepted:
 *  401 -> re-pair UX; 429 -> back-off note; anything else -> log. */
function handleEngineFailure(site, e) {
    if (e instanceof ApiError) {
        const verdict = classifyCheckFailure(e.status, e.message);
        if (verdict.action === "repair") {
            clearTokenAndRequirePairing(verdict.message);
            return;
        }
        if (verdict.action === "backoff") {
            setStatus({ kind: "warn", message: verdict.message });
            return;
        }
    }
    log(errorStatus(site, e).message);
}
// ---------------------------------------------------------------- health
async function refreshHealth() {
    const engineVersionEl = document.getElementById("engineVersion");
    try {
        const h = (await api("/v1/health", {}));
        if (engineVersionEl)
            engineVersionEl.textContent = h.engine || "?";
        const pairingCard = requireEl("pairingCard");
        const unpairBtn = requireEl("unpairBtn");
        if (!token) {
            setStatus({ kind: "warn", message: "Not paired. Enter the pairing code from ScholarScribe." });
            pairingCard.classList.remove("hidden");
            return;
        }
        setStatus(healthStatus(h.paired === true, h.strictness));
        pairingCard.classList.toggle("hidden", h.paired === true);
        unpairBtn.classList.toggle("hidden", h.paired !== true);
    }
    catch (e) {
        setStatus(errorStatus("health", e));
    }
}
// ---------------------------------------------------------------- checks
function epochFor(paragraphId) {
    let e = paragraphEpoch.get(paragraphId);
    if (!e) {
        e = new Epoch();
        paragraphEpoch.set(paragraphId, e);
    }
    return e;
}
/** Whole-document check: enumerate paragraphs, fingerprint, POST changed. */
async function checkDocument(force) {
    if (!token || checking)
        return;
    const now = Date.now();
    if (!force && !cooldown.admit(now))
        return;
    checking = true;
    try {
        await Word.run(async (ctx) => {
            const paras = ctx.document.body.paragraphs;
            paras.load("items");
            await ctx.sync();
            for (let i = 0; i < paras.items.length; i++) {
                const text = paras.items[i]?.text || "";
                const key = paragraphKeyByPosition(i);
                const fingerprint = paragraphFingerprint(key, text);
                // Skip unchanged paragraphs: the fingerprint carries key + text
                // hash, so a paragraph reordering is observed, not silently
                // matched by position.
                if (lastKnownText.get("k:" + key) === fingerprint)
                    continue;
                lastKnownText.set("k:" + key, fingerprint);
                lastKnownText.set(fingerprint, text);
                if (text.trim().length === 0)
                    continue;
                await checkParagraph(key, text);
            }
        });
    }
    catch (e) {
        handleEngineFailure("check", e);
    }
    finally {
        checking = false;
    }
}
/** Remember post-edit text so the next fingerprint diff sees it. */
function rememberText(paragraphId, text) {
    const fingerprint = paragraphFingerprint(paragraphId, text);
    lastKnownText.set("k:" + paragraphId, fingerprint);
    lastKnownText.set(fingerprint, text);
}
/** The paragraph text behind the most recent observation. */
function lastKnownTextFor(paragraphId) {
    const fingerprint = lastKnownText.get("k:" + paragraphId);
    return fingerprint ? lastKnownText.get(fingerprint) || "" : "";
}
/** POST one paragraph; render tier0 + tier1 verdicts. */
async function checkParagraph(paragraphId, text) {
    const seen = epochFor(paragraphId).bump();
    const resp = (await api("/v1/check", {
        method: "POST",
        headers: authHeaders(),
        body: JSON.stringify({ paragraph_id: paragraphId, text, has_focus: true }),
    }));
    // Stale discard: if the paragraph changed (or another check for it
    // started) while this request was in flight, drop the result.
    if (!epochFor(paragraphId).isCurrent(seen)) {
        log("stale result discarded for " + paragraphId);
        return;
    }
    liveIssues = liveIssues.filter((li) => li.paraId !== paragraphId);
    if (resp.language_status && resp.language_status.status === "unsupported") {
        setStatus(unsupportedLanguage(resp.language_status.script));
        return;
    }
    const issues = [];
    const budget = new RepollBudget(REPOLL_MAX, REPOLL_MS);
    let pending = false;
    for (const s of resp.sentences || []) {
        for (const issue of s.issues) {
            issues.push(issue);
            liveIssues.push({ paraId: paragraphId, sentenceStart: s.start, issue });
        }
        const t1 = s.tier1;
        if (t1 && t1.state === "ready" && t1.suggestion) {
            renderTier1(paragraphId, s.start, { ...t1, state: "ready" });
        }
        if (t1 && t1.state === "pending")
            pending = true;
    }
    if (usesAnnotations(mode)) {
        await renderAnnotations(paragraphId, issues);
    }
    else {
        renderFallback(paragraphId, issues);
    }
    if (pending) {
        const delay = budget.admit();
        if (delay !== null) {
            setTimeout(() => {
                repoll(paragraphId, text).catch(() => {
                    /* logged inside */
                });
            }, delay);
        }
        else {
            log("Tier 1 still pending after " + REPOLL_MAX + " re-polls; surfaces on the next check");
        }
    }
}
/** Re-post the same paragraph to pick up Tier 1 verdicts (tier0 is cached
 *  server-side). The re-poll re-bumps the epoch through checkParagraph,
 *  so only the newest observation can render. */
async function repoll(paragraphId, text) {
    if (!token)
        return;
    try {
        await checkParagraph(paragraphId, text);
    }
    catch (e) {
        handleEngineFailure("repoll", e);
    }
}
// ------------------------------------------------- annotations (1.7/1.8)
/**
 * ADAPTER: builds the Word.Critique payload for one issue (spike-verified
 * shape: { start, length, colorScheme, popupOptions? }). Under the default
 * OFFSET_UNIT_MODE="utf16", start/length are UTF-16 code units relative to
 * the PARAGRAPH start (JS semantics); if the owner's spike report
 * (experiment (a)) proves Word measures scalar values instead, flipping
 * OFFSET_UNIT_MODE is the whole change.
 */
function toCritique(issue, paragraphText, popupOk) {
    let start = issue.start_utf16;
    let length = issue.end_utf16 - issue.start_utf16;
    if (OFFSET_UNIT_MODE === "scalar") {
        const map = new OffsetMap(paragraphText);
        start = map.scalarToUtf16(issue.start);
        length = map.scalarToUtf16(issue.end) - map.scalarToUtf16(issue.start);
    }
    const critique = {
        start,
        length,
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
    if (!issues.length)
        return;
    const paraText = lastKnownTextFor(paragraphId);
    try {
        await Word.run(async (ctx) => {
            const paras = ctx.document.body.paragraphs;
            paras.load("items");
            await ctx.sync();
            for (const p of paras.items) {
                if ((p.text || "") !== paraText)
                    continue;
                const critiques = issues.map((i) => toCritique(i, paraText, mode === "annotations"));
                // Spike-verified call (experiments (c)/(j)): paragraph-scoped.
                p.insertAnnotations(critiques);
                await ctx.sync();
                log("annotations rendered: " + critiques.length);
                return;
            }
            log("paragraph not found for annotations; using list mode");
            renderFallback(paragraphId, issues);
        });
    }
    catch (e) {
        log(errorStatus("annotation", e).message + "; using list mode");
        renderFallback(paragraphId, issues);
    }
}
/** Popup actions (1.8): apply the chosen suggestion, report, re-check. */
async function attachPopupHandler() {
    if (mode !== "annotations")
        return;
    try {
        await Word.run(async (ctx) => {
            ctx.document.onAnnotationPopupAction.add(async (args) => {
                log("popup action: " + JSON.stringify(args));
                const li = liveIssues.find((x) => args && String(args.id).endsWith(x.issue.id));
                const suggestion = args && args.critiqueSuggestion;
                if (li && suggestion) {
                    await applyInlineEdit(li.paraId, li.issue, { kind: "replaceWith", text: suggestion });
                }
            });
            await ctx.sync();
            log("popup handler attached");
        });
    }
    catch (e) {
        log(errorStatus("annotation", e).message + "; list mode stays available");
    }
}
// ------------------------------------------------------- fallback (list)
function renderFallback(paragraphId, issues) {
    const listEl = requireEl("fallbackList");
    listEl.textContent = "";
    if (!issues.length) {
        listEl.textContent = "No issues in the changed paragraph.";
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
        div.appendChild(cat);
        div.appendChild(problem);
        div.appendChild(msg);
        for (const sug of issue.suggestions || []) {
            const chip = document.createElement("span");
            chip.className = "chip";
            chip.textContent = sug.kind === "remove" ? "Remove" : sug.text || "";
            chip.onclick = () => {
                applyInlineEdit(paragraphId, issue, sug).catch((e) => log(errorStatus("apply", e).message));
            };
            div.appendChild(chip);
        }
        listEl.appendChild(div);
    }
    requireEl("fallbackCard").classList.remove("hidden");
}
/** List-mode apply: plan → write → watermark → report → re-check. */
async function applyInlineEdit(paragraphId, issue, suggestion) {
    const text = lastKnownTextFor(paragraphId);
    let planned;
    try {
        planned = planInlineEdit(paragraphId, text, issue, suggestion);
    }
    catch (e) {
        // Stale offsets: never apply a wrong edit; surface and stop.
        log(errorStatus("apply", e).message);
        return;
    }
    watermark.push(planned.watermarkStart, planned.watermarkEnd, Date.now());
    try {
        await Word.run(async (ctx) => {
            const paras = ctx.document.body.paragraphs;
            paras.load("items");
            await ctx.sync();
            for (const p of paras.items) {
                if ((p.text || "") === text) {
                    p.insertText(planned.nextText, "Replace");
                    break;
                }
            }
            await ctx.sync();
        });
        rememberText(paragraphId, planned.nextText);
        await api("/v1/accepted", {
            method: "POST",
            headers: authHeaders(),
            body: JSON.stringify(planned.report),
        });
        log("accepted: " + issue.id);
        checkDocument(true);
    }
    catch (e) {
        handleEngineFailure("apply", e);
    }
}
function renderTier1(paragraphId, sentenceStart, t1) {
    if (!t1.suggestion)
        return;
    const sug = t1.suggestion;
    const box = document.createElement("div");
    box.className = "tier1";
    const label = document.createElement("div");
    label.textContent = "Rewrite available" + (sug.explanation ? " — " + sug.explanation : "");
    const chip = document.createElement("span");
    chip.className = "chip rewrite";
    chip.textContent = "Apply rewrite";
    chip.onclick = () => {
        applyTier1Edits(paragraphId, sentenceStart, sug).catch((e) => log(errorStatus("apply", e).message));
    };
    box.appendChild(label);
    box.appendChild(chip);
    requireEl("fallbackCard").classList.remove("hidden");
    requireEl("fallbackList").appendChild(box);
}
/** Apply a Tier 1 rewrite: planned scalar→UTF-16 edits, then re-check. */
async function applyTier1Edits(paragraphId, sentenceStartU16, sug) {
    const paraText = lastKnownTextFor(paragraphId);
    // Sentence-relative scalar edits → paragraph-relative UTF-16 edits.
    // The sentence start arrives as a UTF-16 offset already (engine wire
    // format), so the sentence text is the slice from there.
    const sentenceText = paraText.slice(sentenceStartU16);
    let planned;
    try {
        planned = planTier1Edits(paragraphId, paraText, sentenceStartU16, sentenceText, sug, new OffsetMap(sentenceText));
    }
    catch (e) {
        log(errorStatus("apply", e).message);
        return;
    }
    watermark.push(planned.watermarkStart, planned.watermarkEnd, Date.now());
    try {
        await Word.run(async (ctx) => {
            const paras = ctx.document.body.paragraphs;
            paras.load("items");
            await ctx.sync();
            for (const p of paras.items) {
                if ((p.text || "") === paraText) {
                    p.insertText(planned.nextText, "Replace");
                    break;
                }
            }
            await ctx.sync();
        });
        rememberText(paragraphId, planned.nextText);
        await api("/v1/accepted", {
            method: "POST",
            headers: authHeaders(),
            body: JSON.stringify(planned.report),
        });
        checkDocument(true);
    }
    catch (e) {
        handleEngineFailure("apply", e);
    }
}
// ------------------------------------------------------------- watching
function isSelfEvent(rangeStart, rangeEnd) {
    return watermark.isSelfEvent(rangeStart, rangeEnd, Date.now());
}
async function startWatching() {
    if (!hasLiveWatching(mode)) {
        log("paragraph events unavailable (WordApi 1.6); manual re-check only");
        setStatus({ kind: "ok", message: "Connected (manual mode: use Re-check)." });
        return;
    }
    Office.context.document.addHandlerAsync(Office.EventType.DocumentParagraphChanged ?? "documentParagraphChanged", (args) => {
        // Self-suppression: our own writes fire paragraph-changed too; a
        // watermark overlap schedules nothing (the apply flow already
        // re-checked with force=true).
        const ev = args;
        if (ev &&
            typeof ev.start === "number" &&
            typeof ev.end === "number" &&
            isSelfEvent(ev.start, ev.end)) {
            log("self event suppressed");
            return;
        }
        debounce.schedule(() => {
            checkDocument(false);
        });
    }, (asyncResult) => {
        if (asyncResult.status === Office.AsyncResultStatus.Succeeded) {
            log("watching paragraph changes");
            checkDocument(true);
            attachPopupHandler();
        }
        else {
            log("could not attach paragraph listener: " + asyncResult.error.message);
            setStatus(errorStatus("watch", new Error(asyncResult.error.message)));
        }
    });
}
// ------------------------------------------------------------------ boot
Office.onReady(() => {
    const supported = (v) => Office.context.requirements.isSetSupported("WordApi", v);
    const caps = {
        paragraphEvents: supported(1.6),
        annotations: supported(1.7),
        popupActions: supported(1.8),
    };
    mode = selectMode(caps);
    requireEl("caps").textContent = capsLine(caps);
    requireEl("fallbackCard").classList.toggle("hidden", mode === "annotations");
    log("mode: " + mode);
    loadToken().then(refreshHealth);
    const pairBtn = requireEl("pairBtn");
    const pairCode = requireEl("pairCode");
    pairBtn.addEventListener("click", () => {
        const code = (pairCode.value || "").trim().toUpperCase();
        if (code.length !== 6) {
            setStatus({ kind: "warn", message: "The pairing code is 6 characters." });
            return;
        }
        pairBtn.disabled = true;
        pair(code)
            .then(() => {
            pairBtn.disabled = false;
            pairCode.value = "";
        })
            .catch((e) => {
            // A2×H1/H2: the failure kind decides the wording — wrong code,
            // lockout (cooldown + NEW code required), rate limit, expiry.
            // The typed code and any token are never shown or logged.
            if (e instanceof ApiError) {
                const v = classifyPairFailure(e.status, e.message);
                setStatus({ kind: v.kind === "rate-limited" || v.kind === "locked" ? "warn" : "err", message: v.message });
            }
            else {
                setStatus(errorStatus("pair", e));
            }
            pairBtn.disabled = false;
        });
    });
    requireEl("recheckBtn").addEventListener("click", () => checkDocument(true));
    requireEl("unpairBtn").addEventListener("click", async () => {
        try {
            window.localStorage.removeItem("scholarscribe-assist-token");
        }
        catch {
            /* ignore */
        }
        token = null;
        requireEl("unpairBtn").classList.add("hidden");
        await refreshHealth();
    });
    setInterval(() => {
        refreshHealth();
    }, HEALTH_POLL_MS);
});
