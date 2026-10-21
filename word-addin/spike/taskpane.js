// ScholarScribe Word Assist — Phase 0 spike task pane.
// Experiment code only; deliberately simple, no build step, strict-ish JS.
// Every action and event is appended to the on-screen log with a timestamp.
/* global Word */

"use strict";

const TEST_TEXT =
  'Café “smart quotes”, NBSP:\u00A0here, Arabic دومين, emoji \u{1F44D}\u{1F3FD} and \u{1F389}.';

let plantedParagraph = null; // the planted Word.Paragraph proxy (re-fetched per op)
let eventsRegistered = false;

// ---------- logging ----------

const logEl = () => document.getElementById("log");

function ts() {
  return new Date().toISOString().slice(11, 23);
}

function log(label, detail) {
  const line = `[${ts()}] ${label}${detail !== undefined ? " :: " + detail : ""}`;
  logEl().appendChild(document.createTextNode(line + "\n"));
  logEl().scrollTop = logEl().scrollHeight;
}

function logErr(label, err) {
  log(label, "ERROR " + (err && (err.message || err.debugInfo ? JSON.stringify(err.message) : String(err))));
}

// ---------- environment report ----------

function reportEnvironment() {
  const rows = [];
  try { rows.push("platform: " + Office.context.platform); } catch (e) { rows.push("platform: unavailable"); }
  try { rows.push("host: " + Office.context.host + " v" + Office.context.diagnostics.version); } catch (e) { rows.push("host: unavailable"); }
  try { rows.push("requirementsMode: " + JSON.stringify(Office.context.requirements)); } catch (e) { /* ignore */ }
  [1.5, 1.6, 1.7, 1.8].forEach((v) => {
    let s;
    try { s = Office.context.requirements.isSetSupported("WordApi", v); } catch (e) { s = "throw"; }
    rows.push(`isSetSupported("WordApi", ${v}): ${s}`);
  });
  rows.push("userAgent: " + navigator.userAgent);
  document.getElementById("status").textContent = rows.join("\n");
  log("ENV", rows.join(" | "));
}

// ---------- step 1: plant + read ----------

function plantParagraph() {
  Word.run(async (ctx) => {
    const body = ctx.document.body;
    const para = body.insertParagraph(TEST_TEXT, Word.InsertLocation.end);
    para.load("uniqueLocalId");
    await ctx.sync();
    plantedParagraph = para;
    const ruler =
      `planted. JS string.length (UTF-16 units) = ${TEST_TEXT.length}, ` +
      `[...str].length (code points) = ${[...TEST_TEXT].length}`;
    document.getElementById("planted").textContent = ruler +
      "\nindex map (UTF-16): " + [...TEST_TEXT].map((ch, i) => `${i}:"${ch}"`).join(" ");
    log("PLANT", ruler);
  }).catch((e) => logErr("PLANT", e));
}

function readParagraph() {
  Word.run(async (ctx) => {
    const para = await getPlanted(ctx);
    const plain = para.getText();
    await ctx.sync();
    log("getText()", JSON.stringify(plain.value));
    log("matches planted JS string?", String(plain.value === TEST_TEXT));

    // Options-object variant. NOT VERIFIED in docs which WordApi set carries
    // GetTextOptions {includeHiddenText, includeDeletedText}; the try/catch IS
    // the experiment. We log the raw rejection if the platform refuses it.
    try {
      const withOpts = para.getText({ includeHiddenText: true, includeDeletedText: true });
      await ctx.sync();
      log("getText({includeHiddenText,includeDeletedText})", JSON.stringify(withOpts.value));
    } catch (e) {
      log("getText(options)", "REJECTED (" + (e && e.message ? e.message : String(e)) + ")");
    }
  }).catch((e) => logErr("READ", e));
}

async function getPlanted(ctx) {
  if (plantedParagraph) {
    plantedParagraph.load("text");
    await ctx.sync();
    return plantedParagraph;
  }
  // Fallback: last paragraph of the body.
  const paras = ctx.document.body.paragraphs;
  const last = paras.getLast();
  last.load("text");
  await ctx.sync();
  return last;
}

// ---------- step 2: annotations + events ----------

function makeCritique(start, length) {
  // 1.7 shape: { start, length, colorScheme }. 1.8 adds popupOptions.
  const critique = { start: start, length: length, colorScheme: "Red" };
  if (Office.context.requirements.isSetSupported("WordApi", 1.8)) {
    critique.popupOptions = {
      suggestions: ["Spike: replace with «FIXED»"],
    };
  } else {
    log("popupOptions", "omitted (WordApi 1.8 not supported)");
  }
  return critique;
}

function insertAnnotation(start, length) {
  Word.run(async (ctx) => {
    const para = await getPlanted(ctx);
    const critique = makeCritique(start, length);
    log("insertAnnotations", JSON.stringify(critique));
    para.insertAnnotations([critique]);
    await ctx.sync();
    log("insertAnnotations", "sync OK");
    // Observation (j): correlation between our critique and Word's annotation ids.
    try {
      const ann = ctx.document.annotations;
      ann.load("items/id,items/state");
      await ctx.sync();
      ann.items.forEach((a) => log("annotations.item", `id=${a.id} state=${a.state}`));
    } catch (e) {
      log("annotations collection", "UNAVAILABLE (" + (e && e.message ? e.message : String(e)) + ")");
    }
  }).catch((e) => logErr("ANNOTATE", e));
}

function registerEvents() {
  if (eventsRegistered) { log("events", "already registered"); return; }
  Word.run(async (ctx) => {
    // Paragraph change (WordApi 1.6). The core of the pipeline; also the
    // feedback-loop probe (e): our own inserts/edits must show up here.
    try {
      const h = ctx.document.onParagraphChanged.add((args) => {
        log("onParagraphChanged", JSON.stringify({
          changeType: args.changeType,
          uniqueLocalIds: args.uniqueLocalIds,
        }));
      });
      await ctx.sync();
      log("events", "onParagraphChanged registered");
      window.__spikeHandlers = window.__spikeHandlers || [];
      window.__spikeHandlers.push(h);
    } catch (e) {
      log("onParagraphChanged", "UNAVAILABLE (" + (e && e.message ? e.message : String(e)) + ")");
    }

    // Annotation lifecycle events (WordApi 1.7).
    const annEvents = [
      ["onAnnotationClicked", (args) => log("onAnnotationClicked", JSON.stringify(args))],
      ["onAnnotationHovered", (args) => log("onAnnotationHovered", JSON.stringify(args))],
      ["onAnnotationInserted", (args) => log("onAnnotationInserted", JSON.stringify(args))],
      ["onAnnotationRemoved", (args) => log("onAnnotationRemoved", JSON.stringify(args))],
    ];
    for (const [name, handler] of annEvents) {
      try {
        const h = ctx.document[name].add(handler);
        await ctx.sync();
        log("events", name + " registered");
        window.__spikeHandlers.push(h);
      } catch (e) {
        log(name, "UNAVAILABLE (" + (e && e.message ? e.message : String(e)) + ")");
      }
    }

    // Popup action (WordApi 1.8) — observation (b).
    try {
      const h = ctx.document.onAnnotationPopupAction.add(async (args) => {
        log("onAnnotationPopupAction", JSON.stringify(args));
        if (document.getElementById("applyMode").checked) {
          await applyReplacementManually(args);
        }
      });
      await ctx.sync();
      log("events", "onAnnotationPopupAction registered");
      window.__spikeHandlers.push(h);
    } catch (e) {
      log("onAnnotationPopupAction", "UNAVAILABLE (" + (e && e.message ? e.message : String(e)) + ")");
    }

    eventsRegistered = true;
  }).catch((e) => logErr("EVENTS", e));
}

// Observation (b): with Apply-on-action ON, the pane replaces the annotated
// range itself. If Word already applied the suggestion before this runs, the
// paragraph text will no longer match — we log that difference explicitly.
async function applyReplacementManually(args) {
  Word.run(async (ctx) => {
    const para = await getPlanted(ctx);
    const before = para.text;
    // args carries id + critiqueSuggestion (exact shape = observation b).
    const suggestion = args.critiqueSuggestion || "«FIXED»";
    // We only know start/length for the LAST annotation we inserted; for the
    // spike we use the current inputs — good enough to observe behaviour.
    const start = Number(document.getElementById("inStart").value) || 0;
    const length = Number(document.getElementById("inLength").value) || 0;
    const range = para.getRange("Whole");
    log("manualApply", `paragraph before: ${JSON.stringify(before)}; replacing [${start},${length}) with ${JSON.stringify(suggestion)}`);
    // NOTE: range-level replacement at offsets requires Range via paragraph;
    // the spike uses insertParagraph-free single-range write to keep it simple:
    // we do NOT attempt surgical text surgery here — we only log and delete
    // the annotation, because WHO applies is exactly what we are testing.
    try {
      const anns = ctx.document.annotations;
      anns.load("items/id");
      await ctx.sync();
      const target = anns.items.find((a) => a.id === args.id);
      if (target) {
        target.delete();
        await ctx.sync();
        log("manualApply", "annotation deleted by pane; text NOT changed by pane (deliberate)");
      } else {
        log("manualApply", "annotation id not found: " + args.id);
      }
    } catch (e) {
      log("manualApply", "annotation handling failed (" + (e && e.message ? e.message : String(e)) + ")");
    }
  }).catch((e) => logErr("manualApply", e));
}

// ---------- wiring ----------

Office.onReady(function () {
  reportEnvironment();
  document.getElementById("btnPlant").onclick = plantParagraph;
  document.getElementById("btnRead").onclick = readParagraph;
  document.getElementById("btnAnnHardcoded").onclick = () => insertAnnotation(0, 12);
  document.getElementById("btnAnnCustom").onclick = () =>
    insertAnnotation(Number(document.getElementById("inStart").value) || 0,
                     Number(document.getElementById("inLength").value) || 0);
  document.getElementById("btnEvents").onclick = registerEvents;
  document.getElementById("btnClear").onclick = () => { logEl().textContent = ""; };
  document.getElementById("btnCopy").onclick = () => {
    navigator.clipboard.writeText(logEl().textContent).then(
      () => log("log", "copied to clipboard"),
      (e) => logErr("log", e)
    );
  };
  log("READY", "spike task pane loaded");
});
