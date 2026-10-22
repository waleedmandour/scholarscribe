// ScholarScribe Word Assist — error states (pure mapping).
//
// Every failure the client can observe maps to ONE user-facing status:
// kind (ok/warn/err) + a message that says what the user should do.
// Keeping the mapping pure lets the tests pin the exact wording that QA
// (docs/QA_v0.1.md) expects to see.
/**
 * Map a thrown value at a call site to the status the pane shows.
 * The messages are the Phase 2 client's wording, preserved verbatim where
 * it existed, so the QA matrix and the docs stay truthful.
 */
export function errorStatus(site, e) {
    const msg = e instanceof Error ? e.message : String(e);
    switch (site) {
        case "health":
            return {
                kind: "err",
                message: "ScholarScribe is not reachable (" + msg +
                    "). Is the app running with Word Assist enabled?",
            };
        case "pair":
            return { kind: "err", message: "Pairing failed: " + msg };
        case "check":
            return { kind: "warn", message: "check failed: " + msg };
        case "apply":
            return { kind: "err", message: "apply failed: " + msg };
        case "repoll":
            return { kind: "warn", message: "re-poll failed: " + msg };
        case "watch":
            return {
                kind: "warn",
                message: "Connected, but live watching is unavailable. Use Re-check.",
            };
        case "annotation":
            return { kind: "warn", message: msg };
    }
}
/** Status shown when the engine reports an unsupported language. */
export function unsupportedLanguage(script) {
    return {
        kind: "warn",
        message: "Language not supported yet (" + (script || "?") +
            "). You can enable it experimentally in ScholarScribe → Word Assist.",
    };
}
/** Statuses shown on a successful health poll (paired / not paired). */
export function healthStatus(paired, strictness) {
    if (!paired) {
        return { kind: "warn", message: "Not paired. Enter the pairing code from ScholarScribe." };
    }
    return { kind: "ok", message: "Connected to ScholarScribe (" + (strictness || "dev") + ")." };
}
