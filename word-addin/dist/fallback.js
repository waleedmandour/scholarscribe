// ScholarScribe Word Assist — fallback-mode selection (pure).
//
// The add-in probes Word's requirement sets at startup (spike experiments
// (f)/(k) verified the PROBE, not every platform's answer). The probe
// result alone decides the mode; nothing is guessed per platform.
//
//  annotations+popup → annotation mode (underlines + popup suggestions)
//  annotations only  → annotation mode, suggestions via list fallback
//  paragraph events  → list mode with live re-checking
//  nothing           → manual list mode (Re-check button only)
//
// The ENGINE's language gate is independent of the mode: an unsupported
// script degrades every mode to its "language unsupported" state; that
// decision belongs to the response handling, not the capability probe.
/** Decide the client mode from the probed capability set. */
export function selectMode(caps) {
    if (caps.annotations && caps.popupActions)
        return "annotations";
    if (caps.annotations)
        return "annotations-nopopup";
    if (caps.paragraphEvents)
        return "list";
    return "manual-list";
}
/** True when issue rendering should try Word annotations first. */
export function usesAnnotations(mode) {
    return mode === "annotations" || mode === "annotations-nopopup";
}
/** True when live paragraph watching is available at all. */
export function hasLiveWatching(mode) {
    return mode === "annotations" || mode === "annotations-nopopup" || mode === "list";
}
/** One-line capability string for the pane footer (same wording as the
 *  Phase 2 client so QA screenshots stay comparable). */
export function capsLine(caps) {
    return ("WordApi 1.6 (paragraph events): " + (caps.paragraphEvents ? "yes" : "NO") +
        " · 1.7 (annotations): " + (caps.annotations ? "yes" : "NO — list mode") +
        " · 1.8 (popup actions): " + (caps.popupActions ? "yes" : "NO — list mode"));
}
