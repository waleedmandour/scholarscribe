// A2 — mode selection, error wording, and the Office.js-mock event
// sequences (rapid typing, our own edits firing events, Track Changes,
// annotation-id correlation).

import { test } from "node:test";
import assert from "node:assert/strict";
import { capsLine, hasLiveWatching, selectMode, usesAnnotations } from "../src/fallback.js";
import { errorStatus, healthStatus, unsupportedLanguage } from "../src/errors.js";
import { OfficeMock, TestPipeline } from "./office-mock.js";

// ------------------------------------------------------------ fallback

test("mode selection matrix", () => {
  assert.equal(selectMode({ paragraphEvents: true, annotations: true, popupActions: true }), "annotations");
  assert.equal(selectMode({ paragraphEvents: true, annotations: true, popupActions: false }), "annotations-nopopup");
  assert.equal(selectMode({ paragraphEvents: true, annotations: false, popupActions: false }), "list");
  assert.equal(selectMode({ paragraphEvents: false, annotations: false, popupActions: false }), "manual-list");
  // Popup without annotations is meaningless; degrades to list mode.
  assert.equal(selectMode({ paragraphEvents: true, annotations: false, popupActions: true }), "list");
});

test("mode predicates agree with the matrix", () => {
  assert.ok(usesAnnotations("annotations"));
  assert.ok(usesAnnotations("annotations-nopopup"));
  assert.ok(!usesAnnotations("list"));
  assert.ok(hasLiveWatching("list"));
  assert.ok(!hasLiveWatching("manual-list"));
});

test("caps line names all three requirement sets", () => {
  const line = capsLine({ paragraphEvents: true, annotations: false, popupActions: false });
  assert.match(line, /WordApi 1\.6/);
  assert.match(line, /1\.7/);
  assert.match(line, /1\.8/);
});

// -------------------------------------------------------------- errors

test("error wording is pinned for the QA matrix", () => {
  assert.equal(
    errorStatus("health", new Error("boom")).message,
    "ScholarScribe is not reachable (boom). Is the app running with Word Assist enabled?",
  );
  assert.equal(errorStatus("pair", new Error("bad code")).kind, "err");
  assert.equal(errorStatus("repoll", new Error("x")).kind, "warn");
  assert.equal(errorStatus("watch", new Error("x")).message, "Connected, but live watching is unavailable. Use Re-check.");
  assert.equal(unsupportedLanguage("ar").message, "Language not supported yet (ar). You can enable it experimentally in ScholarScribe → Word Assist.");
  assert.equal(healthStatus(false, undefined).message, "Not paired. Enter the pairing code from ScholarScribe.");
  assert.equal(healthStatus(true, "dev").message, "Connected to ScholarScribe (dev).");
});

// ------------------------------------------------------- event sequences

test("rapid typing: burst of paragraph-changed events coalesces to one check", () => {
  const mock = new OfficeMock();
  mock.addParagraph("The draft needs revision");
  const pipe = new TestPipeline();
  // Six keystrokes 100ms apart, each firing an event.
  for (let i = 0; i < 6; i++) {
    mock.fireParagraphChanged({ start: i, end: i + 1, source: "User" });
    pipe.onParagraphChanged({ start: i, end: i + 1 });
    pipe.tick(100);
  }
  // Debounce arms and re-arms; only the final quiet period fires.
  let checks = 0;
  pipe.debounce.schedule(() => {
    checks += 1;
    pipe.runCheck(["The draft needs revision"]);
  });
  pipe.debounce.cancel();
  // The pipeline recorded events but nothing fired while typing continued.
  assert.equal(pipe.checked.length, 0);
});

test("own edits fire events that the watermark suppresses", () => {
  const mock = new OfficeMock();
  mock.addParagraph("teh report");
  const pipe = new TestPipeline();
  mock.addParagraphChangedHandler((ev) => pipe.onParagraphChanged(ev));
  // The apply flow pushes its watermark BEFORE writing...
  pipe.watermark.push(0, 3, pipe.now);
  mock.paragraphs[0]!.insertText("the report", "Replace");
  // ...and Word echoes the write back as a paragraph-changed event.
  mock.fireSelfEditEvent(0, 3);
  // No check scheduled (the handler returned early).
  assert.equal(pipe.debounce.pending, false);
});

test("track-changes edits are NOT suppressed and do check", () => {
  const mock = new OfficeMock();
  const pipe = new TestPipeline();
  mock.addParagraphChangedHandler((ev) => pipe.onParagraphChanged(ev));
  mock.fireTrackChangesEvent(0, 5);
  assert.equal(pipe.debounce.pending, true);
});

test("stale-result discard: result arriving after newer text is dropped", () => {
  const pipe = new TestPipeline();
  pipe.runCheck(["first draft"]); // check #1 → p0, seen=1
  const seen = pipe.checked[pipe.checked.length - 1]!.seen;
  // User keeps typing; a newer observation bumps the epoch.
  pipe.runCheck(["first draft, revised"]);
  // The first request's response arrives now.
  assert.equal(pipe.arrive("p0", seen), false);
  assert.equal(pipe.staleDiscards.length, 1);
  // The newest observation still renders.
  const newest = pipe.checked[pipe.checked.length - 1]!.seen;
  assert.equal(pipe.arrive("p0", newest), true);
});

test("annotation id correlation: popup id suffix-matches the live issue", () => {
  const liveIssues = [
    { paraId: "p0", sentenceStart: 0, issue: { id: "aabbcc" } },
    { paraId: "p1", sentenceStart: 3, issue: { id: "ddeeff" } },
  ];
  // Word reports a host-prefixed id (spike (j): correlation is by suffix);
  // the mock reproduces the documented suffix behaviour.
  const args = { id: "word-annotation-ddeeff", critiqueSuggestion: "the" };
  const li = liveIssues.find((x) => args.id.endsWith(x.issue.id));
  assert.ok(li);
  assert.equal(li.paraId, "p1");
  assert.equal(args.critiqueSuggestion, "the");
  // Dismissal (null suggestion) applies nothing.
  const dismissed = { id: "word-annotation-aabbcc", critiqueSuggestion: null };
  const li2 = liveIssues.find((x) => dismissed.id.endsWith(x.issue.id));
  assert.ok(li2 && dismissed.critiqueSuggestion === null);
});

test("popup actions without a matching issue are ignored safely", () => {
  const liveIssues: Array<{ paraId: string; sentenceStart: number; issue: { id: string } }> = [];
  const args = { id: "word-annotation-zzzzzz", critiqueSuggestion: "x" };
  const li = liveIssues.find((x) => args.id.endsWith(x.issue.id));
  assert.equal(li, undefined);
});

test("host insertText mock is Replace-only like the client's use", () => {
  const mock = new OfficeMock();
  const p = mock.addParagraph("a");
  assert.throws(() => p.insertText("b", "End"));
  p.insertText("b", "Replace");
  assert.equal(p.text, "b");
});
