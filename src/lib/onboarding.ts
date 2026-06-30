/**
 * Onboarding store — controls the welcome tour visibility.
 *
 * Persistence model:
 *   localStorage["scholarscribe-onboarded"] = "v1"  -> user has seen/completed the tour
 *
 * The tour is shown on first launch (no key) and can be re-opened manually
 * from the sidebar footer or About tab via the `showTour` store.
 */
import { writable } from "svelte/store";

const ONBOARDED_KEY = "scholarscribe-onboarded";
const ONBOARDED_VERSION = "v1";

function readOnboarded(): boolean {
  try {
    return localStorage.getItem(ONBOARDED_KEY) === ONBOARDED_VERSION;
  } catch {
    return false;
  }
}

function writeOnboarded() {
  try {
    localStorage.setItem(ONBOARDED_KEY, ONBOARDED_VERSION);
  } catch {
    // localStorage may be unavailable in some embedded contexts; non-fatal.
  }
}

// Initial state: show tour if user has NOT been onboarded yet.
export const showTour = writable<boolean>(!readOnboarded());

/** Open the tour manually (from sidebar / About tab). Does not touch the persisted flag. */
export function openTour() {
  showTour.set(true);
}

/** Close the tour without marking the user as onboarded (e.g. user clicked Skip). */
export function closeTour() {
  showTour.set(false);
}

/**
 * Close the tour AND mark the user as onboarded.
 * Called when the user clicks "Finish" or ticks "Don't show again at start"
 * and then closes the tour.
 */
export function completeTour() {
  writeOnboarded();
  showTour.set(false);
}

/** Reset onboarding (useful for testing / "show again" debugging). */
export function resetOnboarding() {
  try {
    localStorage.removeItem(ONBOARDED_KEY);
  } catch {
    // ignore
  }
  showTour.set(true);
}
