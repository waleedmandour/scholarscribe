// ScholarScribe Word Assist — pairing/session failure classification (A2
// integration adaptation to the H1/H2 security model).
//
// The hardened engine answers with precise status codes and QA-pinned
// human messages (server.rs):
//   - /v1/pair wrong code          -> 401 "wrong or expired pairing code; …"
//   - /v1/pair lockout (H1 burn)   -> 401 "pairing is disabled for now; …"
//   - /v1/pair expired / no code    -> 401 ("pairing code expired…", "no pairing code is active…")
//   - /v1/pair route limiter (H1)  -> 429 "too many pairing attempts; wait a minute"
//   - global limiter               -> 429 "rate limited"
//   - /v1/check,/v1/accepted with a revoked, rotated (H2) or 30-day-expired
//     token, or a missing/garbled bearer -> 401
//
// This module maps those to ONE decision each: what the pane shows and
// what the client does (retry vs. clear the token and re-pair). Pure and
// dependency-free so the simulated-response tests pin every wording.
/** Error type thrown by the API helper: carries the HTTP status and the
 *  raw body text (never logged) so the classifiers below can decide. */
export class ApiError extends Error {
    constructor(status, message) {
        super(message);
        this.name = "ApiError";
        this.status = status;
    }
}
/** Classify a /v1/pair failure. The server message is treated as
 *  untrusted formatting input: the user sees OUR wording, not theirs. */
export function classifyPairFailure(status, serverMessage) {
    const m = serverMessage.toLowerCase();
    if (status === 429) {
        return {
            kind: "rate-limited",
            message: "Too many attempts. Wait one minute, then try again (the code stays valid if not burned).",
            canRetrySameCode: true,
        };
    }
    if (status === 401) {
        if (m.includes("disabled")) {
            return {
                kind: "locked",
                message: "Pairing is locked after too many wrong codes. Wait for the cooldown in the Word Assist tab, request a NEW code, and pair with that.",
                canRetrySameCode: false,
            };
        }
        if (m.includes("no pairing code")) {
            return {
                kind: "no-code",
                message: "There is no active pairing code. Open the Word Assist tab and request one.",
                canRetrySameCode: false,
            };
        }
        // NB: the wrong-code message is "wrong or expired pairing code; …" —
        // it CONTAINS "expired", so the expiry anchor must exclude it.
        if (m.includes("expired") && !m.includes("wrong")) {
            return {
                kind: "expired",
                message: "The pairing code expired (codes last 5 minutes). Request a fresh code.",
                canRetrySameCode: false,
            };
        }
        return {
            kind: "wrong-code",
            message: "That code is wrong. Check the 6-character code in the Word Assist tab and try again (5 wrong attempts burn the code).",
            canRetrySameCode: true,
        };
    }
    if (status >= 500) {
        return {
            kind: "server",
            message: "ScholarScribe had an internal error while pairing. Try again; if it persists, restart the app.",
            canRetrySameCode: true,
        };
    }
    return {
        kind: "network",
        message: "Pairing request failed (" + status + "). Is the app running?",
        canRetrySameCode: true,
    };
}
/** Classify a /v1/check or /v1/accepted failure.
 *  - 401 => the token was revoked, rotated (H2) or expired: clear it and
 *    re-pair (action "repair").
 *  - 429 => the global limiter: back off and retry later.
 *  - anything else => report and continue. */
export function classifyCheckFailure(status, serverMessage) {
    if (status === 401) {
        return {
            action: "repair",
            message: "Your Word Assist session ended (the token was revoked, rotated or expired). Enter a fresh pairing code to connect again.",
        };
    }
    if (status === 429) {
        return {
            action: "backoff",
            message: "The engine is busy — waiting before the next check.",
        };
    }
    return { action: "report", message: serverMessage || "request failed (" + status + ")" };
}
/** Parse a successful /v1/pair body into the bearer token. */
export function parsePairOk(body) {
    if (body && typeof body === "object" && "token" in body) {
        const t = body.token;
        if (typeof t === "string" && t.length > 0)
            return t;
    }
    throw new ApiError(0, "engine returned no token");
}
/** A bearer token is 64 hex chars; a pairing code is 6 A-Z0-9 symbols
 *  (optionally dashed). Neither must ever reach the pane log or an error
 *  message — this strips secret-looking substrings, keeping normal words
 *  intact. Defence in depth: nothing should ever try to log them. */
export function sanitizeForDisplay(s) {
    return s
        .replace(/\b[0-9a-f]{64}\b/gi, "<redacted>")
        .replace(/\b[A-Z0-9]{2,4}-?[A-Z0-9]{2,4}\b/g, (m, off, whole) => {
        // Only treat dash-free 6-symbol ALL-CAPS runs as code-shaped when
        // they stand alone (not inside a longer word).
        const before = off > 0 ? whole[off - 1] ?? "" : "";
        const after = off + m.length < whole.length ? whole[off + m.length] ?? "" : "";
        const isStandalone = !/[A-Za-z0-9]/.test(before) && !/[A-Za-z0-9]/.test(after);
        return isStandalone && m.replace(/-/g, "").length === 6 ? "<redacted>" : m;
    });
}
