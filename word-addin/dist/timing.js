// ScholarScribe Word Assist — timing gates: debounce, cooldown, and
// stale-result discard.
//
// These are the pure control-flow pieces of the check pipeline, isolated
// so Node tests can drive them with a fake clock instead of sleeping
// (A2: "hand-written mock simulating event sequences... rapid typing").
//
// Stale discard semantics (Phase 1b ruling, still evidenced): a check
// result may only be rendered if no newer paragraph text was observed for
// that paragraph between request start and result arrival. Our own edits
// bump the epoch too (they change the text), so a verdict for pre-edit
// text never lands on post-edit text.
/** One debounced trigger. `fire` runs once after `quietMs` of silence. */
export class Debouncer {
    constructor(quietMs) {
        this.quietMs = quietMs;
        this.timer = null;
    }
    /** (Re)arm the debounce. Returns true when this call armed a new wait. */
    schedule(fire) {
        if (this.timer !== null)
            clearTimeout(this.timer);
        this.timer = setTimeout(() => {
            this.timer = null;
            fire();
        }, this.quietMs);
        return true;
    }
    /** Cancel without firing. Returns true when something was cancelled. */
    cancel() {
        if (this.timer === null)
            return false;
        clearTimeout(this.timer);
        this.timer = null;
        return true;
    }
    get pending() {
        return this.timer !== null;
    }
}
/** Monotonic epoch for stale-result discard. */
export class Epoch {
    constructor() {
        this.current = 0;
    }
    /** Record an observation of new state; invalidate everything before. */
    bump() {
        this.current += 1;
        return this.current;
    }
    /** True when `seen` is still the newest observation. */
    isCurrent(seen) {
        return seen === this.current;
    }
    get value() {
        return this.current;
    }
}
/** Minimum-gap gate between expensive full-document checks. */
export class Cooldown {
    constructor(minGapMs) {
        this.minGapMs = minGapMs;
        /** -Infinity so the first admit at t=0 is allowed. */
        this.lastAt = Number.NEGATIVE_INFINITY;
    }
    /** True (and records `now`) when a check may run. */
    admit(now) {
        if (now - this.lastAt < this.minGapMs)
            return false;
        this.lastAt = now;
        return true;
    }
    /** Force-admit the next call regardless of the gap (manual re-check). */
    reset() {
        this.lastAt = 0;
    }
}
/**
 * Re-poll budget for Tier 1 pending verdicts: at most `max` delayed
 * re-checks, each after `gapMs`. `admit` returns the delay for the next
 * re-poll, or null when the budget is spent.
 */
export class RepollBudget {
    constructor(max, gapMs) {
        this.max = max;
        this.gapMs = gapMs;
        this.attempts = 0;
    }
    admit() {
        if (this.attempts >= this.max)
            return null;
        this.attempts += 1;
        return this.gapMs;
    }
    reset() {
        this.attempts = 0;
    }
    get spent() {
        return this.attempts >= this.max;
    }
}
