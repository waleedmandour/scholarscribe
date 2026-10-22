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

export type AssistMode =
  | "annotations" // 1.7 + 1.8 present
  | "annotations-nopopup" // 1.7 present, 1.8 missing
  | "list" // 1.6 present (live events), annotations missing
  | "manual-list"; // nothing present

export interface ProbedCapabilities {
  paragraphEvents: boolean; // WordApi 1.6
  annotations: boolean; // WordApi 1.7
  popupActions: boolean; // WordApi 1.8
}

/** Decide the client mode from the probed capability set. */
export function selectMode(caps: ProbedCapabilities): AssistMode {
  if (caps.annotations && caps.popupActions) return "annotations";
  if (caps.annotations) return "annotations-nopopup";
  if (caps.paragraphEvents) return "list";
  return "manual-list";
}

/** True when issue rendering should try Word annotations first. */
export function usesAnnotations(mode: AssistMode): boolean {
  return mode === "annotations" || mode === "annotations-nopopup";
}

/** True when live paragraph watching is available at all. */
export function hasLiveWatching(mode: AssistMode): boolean {
  return mode === "annotations" || mode === "annotations-nopopup" || mode === "list";
}

/** One-line capability string for the pane footer (same wording as the
 *  Phase 2 client so QA screenshots stay comparable). */
export function capsLine(caps: ProbedCapabilities): string {
  return (
    "WordApi 1.6 (paragraph events): " + (caps.paragraphEvents ? "yes" : "NO") +
    " · 1.7 (annotations): " + (caps.annotations ? "yes" : "NO — list mode") +
    " · 1.8 (popup actions): " + (caps.popupActions ? "yes" : "NO — list mode")
  );
}
