// ScholarScribe Word Assist — self-edit suppression watermark.
//
// When the add-in applies an edit, the paragraph-changed event that OUR
// OWN write fires must not trigger another check (feedback loop) — that
// is the suppression watermark, from the Phase 1b stale-discard ruling.
// The watermark stores the UTF-16 ranges we recently wrote plus the wall
// time; an incoming changed event is "self" when its range overlaps a
// fresh watermark entry.
//
// Pure and clock-injectable: `now` is always a parameter.
export class Watermark {
    constructor(graceMs) {
        this.graceMs = graceMs;
        this.entries = [];
    }
    /** Record a self-write covering [start, end) at time `now`. */
    push(start, end, now) {
        this.entries.push({ start, end, at: now });
    }
    /** Drop entries older than the grace window (measured from `now`). */
    prune(now) {
        this.entries = this.entries.filter((w) => now - w.at < this.graceMs);
    }
    /** True when [start, end) overlaps any fresh self-write. */
    isSelfEvent(start, end, now) {
        this.prune(now);
        return this.entries.some((w) => start < w.end && end > w.start);
    }
    /** Test/teardown helper: forget everything. */
    clear() {
        this.entries = [];
    }
    get size() {
        return this.entries.length;
    }
}
