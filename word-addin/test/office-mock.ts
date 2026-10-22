// ScholarScribe Word Assist — hand-written Office.js mock (A2).
//
// Simulates the Word host surface the client uses, with an event queue so
// tests can replay real-world sequences: rapid typing bursts, the pane's
// OWN edits firing paragraph-changed events, Track Changes edits (source
// differs), and annotation popup actions. Shapes follow the spike-verified
// surface (word-addin/src/office.d.ts) — the mock implements THAT contract,
// not a guessed one.
//
// This file is a TEST fixture: it is never shipped in word-addin/dist.

import { Cooldown, Debouncer, Epoch } from "../src/timing.js";
import { Watermark } from "../src/watermark.js";
import { paragraphKeyByPosition, paragraphFingerprint } from "../src/keys.js";

export interface MockParagraph {
  text: string;
  annotations: Array<{ start: number; length: number; colorScheme?: string; popupOptions?: { suggestions: string[] } }>;
  insertText(text: string, location: string): void;
  insertAnnotations(critiques: Array<MockParagraph["annotations"][number]>): void;
}

export interface MockPopupAction {
  id: string;
  critiqueSuggestion: string | null;
}

export interface MockParagraphChangedEvent {
  start?: number;
  end?: number;
  source?: string; // "User" | "Machine" | ... — spike (e): NOT VERIFIED per platform
  uniqueLocalIds?: string[];
}

/** A fake Word host with the client's shape. */
export class OfficeMock {
  readonly paragraphs: MockParagraph[] = [];
  readonly popupActions: MockPopupAction[] = [];
  /** Annotation ids the host will hand back for popup events (the
   *  correlation suffix format is spike (j); the mock reproduces the
   *  documented behaviour: ids END WITH the client's issue id). */
  private paragraphHandlers: Array<(ev: MockParagraphChangedEvent) => void> = [];
  private popupHandlers: Array<(ev: MockPopupAction) => void> = [];
  /** Capability map, configurable per test. */
  private readonly supported = new Map<string, boolean>([
    ["WordApi", true], // the base set: always true in tests
  ]);

  addParagraph(text: string): MockParagraph {
    const p: MockParagraph = {
      text,
      annotations: [],
      insertText: (next, location) => {
        if (location !== "Replace") throw new Error("mock supports Replace only, got " + location);
        p.text = next;
      },
      insertAnnotations: (critiques) => {
        p.annotations.push(...critiques);
      },
    };
    this.paragraphs.push(p);
    return p;
  }

  setSupported(name: string, version: number, ok: boolean): void {
    this.supported.set(name + "@" + version, ok);
  }

  isSetSupported(name: string, version: number): boolean {
    return this.supported.get(name + "@" + version) ?? false;
  }

  /** Client calls Office.context.document.addHandlerAsync — the mock
   *  records the handler and immediately reports success. */
  addParagraphChangedHandler(handler: (ev: MockParagraphChangedEvent) => void): void {
    this.paragraphHandlers.push(handler);
  }

  addPopupHandler(handler: (ev: MockPopupAction) => void): void {
    this.popupHandlers.push(handler);
  }

  /** Host-side: fire one paragraph-changed event to all handlers. */
  fireParagraphChanged(ev: MockParagraphChangedEvent): void {
    for (const h of this.paragraphHandlers) h(ev);
  }

  /** Host-side: the user picked a suggestion in an annotation popup. */
  firePopupAction(ev: MockPopupAction): void {
    for (const h of this.popupHandlers) h(ev);
  }

  /** The event the pane's own insertText() triggers in real Word
   *  (spike (e): our edits come back as paragraph-changed events; the
   *  watermark must suppress them). */
  fireSelfEditEvent(rangeStart: number, rangeEnd: number): void {
    this.fireParagraphChanged({ start: rangeStart, end: rangeEnd, source: "Machine" });
  }

  /** Track Changes: another author (or the reviewer) edits; the event
   *  carries a source tag (WordApi 1.6 ParagraphChangedEventArgs.source). */
  fireTrackChangesEvent(rangeStart: number, rangeEnd: number): void {
    this.fireParagraphChanged({ start: rangeStart, end: rangeEnd, source: "User", uniqueLocalIds: [] });
  }
}

/**
 * The client-side pipeline the orchestrator wires, assembled from the same
 * modules with an injectable clock — the pure equivalent of taskpane.ts's
 * event path, so the event-sequence tests can drive it deterministically.
 */
export class TestPipeline {
  readonly debounce: Debouncer;
  readonly cooldown: Cooldown;
  readonly watermark: Watermark;
  readonly epochs = new Map<string, Epoch>();
  readonly lastText = new Map<string, string>();
  readonly checked: Array<{ key: string; text: string; at: number; seen: number }> = [];
  readonly staleDiscards: string[] = [];
  now = 0;

  constructor(
    debounceMs = 1200,
    cooldownMs = 400,
    graceMs = 2000,
  ) {
    this.debounce = new Debouncer(debounceMs);
    this.cooldown = new Cooldown(cooldownMs);
    this.watermark = new Watermark(graceMs);
  }

  tick(ms: number): void {
    this.now += ms;
  }

  onParagraphChanged(ev: MockParagraphChangedEvent): void {
    const start = ev.start ?? 0;
    const end = ev.end ?? 0;
    if (this.watermark.isSelfEvent(start, end, this.now)) return;
    this.debounce.schedule(() => {
      /* the orchestrator's checkDocument(false) runs here; tests call
         runCheck directly to drive the pipeline deterministically */
    });
  }

  runCheck(texts: string[]): void {
    for (let i = 0; i < texts.length; i++) {
      const text = texts[i] || "";
      const key = paragraphKeyByPosition(i);
      const fingerprint = paragraphFingerprint(key, text);
      if (this.lastText.get("k:" + key) === fingerprint) continue;
      this.lastText.set("k:" + key, fingerprint);
      let e = this.epochs.get(key);
      if (!e) {
        e = new Epoch();
        this.epochs.set(key, e);
      }
      const seen = e.bump();
      this.checked.push({ key, text, at: this.now, seen });
      this.lastText.set(fingerprint, text);
    }
  }

  /** Result arrival: discard when the epoch moved. */
  arrive(key: string, seen: number): boolean {
    const e = this.epochs.get(key);
    if (e && e.isCurrent(seen)) return true;
    this.staleDiscards.push(key);
    return false;
  }
}
