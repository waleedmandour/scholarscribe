//! Word Assist — the as-you-type writing assist for Microsoft Word (feature
//! `word-assist`).
//!
//! PHASE MAP (docs/WORD_ASSIST_RECON.md §8, revised per the owner's rulings):
//! - 1a (this commit): the pure engine. `offsets`, `sentences`, `mask`,
//!   `diff`, `validate`, `cache`, `tier0`, `engine`. No socket, no Tauri
//!   import in this module tree (ruling D-1: pure logic), no user-visible
//!   surface. Everything below compiles only with
//!   `--features word-assist`; the default build is byte-for-byte the
//!   pre-feature build.
//! - 1b adds `server`, `scheduler`, `settings` and the Tauri command layer.
//! - 2 adds the add-in client under `word-addin/`.
//! - 3 adds `ledger` and the registration flow.
//! - 4 finalizes defaults and docs.
//!
//! PRIVACY DESIGN (module level):
//! - 1a performs no I/O: no files, no sockets, no clocks read for anything
//!   but test timing. Text lives in memory only, and the sentence cache
//!   (`cache`) keys by hash and lives for the process lifetime at most.
//! - Tier 0 runs entirely offline through the existing `language_check`
//!   engine. Tier 1 sends one sentence at a time to the local Ollama daemon
//!   and only after the user enabled the feature (1b wiring).
//!
//! STRICTNESS DEFAULTS (owner ruling E-1, PROVISIONAL until Phase 4):
//! the shipped default profile enables Grammar only. Spelling stays OFF
//! pending a re-evaluation against the FP-share criterion, and Clarity
//! (harper style) stays OFF. Final defaults are a Phase 4 decision. No
//! golden-set precision numbers appear in the UI or README (E-1).

pub mod cache;
pub mod diff;
pub mod engine;
pub mod ledger;
pub mod mask;
pub mod offsets;
pub mod scheduler;
pub mod sentences;
pub mod server;
pub mod settings;
pub mod tier0;
pub mod validate;

/// The canonical offset space used inside the engine: Unicode scalar values
/// (harper's unit and Rust `str`'s unit).
pub type ScalarOffset = usize;
/// The wire offset space used on the add-in boundary: UTF-16 code units
/// (JavaScript string semantics, Word object model semantics).
pub type Utf16Offset = usize;

/// Version of the Word Assist engine, bumped with every behavioral change to
/// prompts, validation or filtering. Reported by `/v1/health` (1b) and
/// written into ledger entries (Phase 3) so any verdict can be traced back
/// to the exact engine behavior that produced it.
pub const ENGINE_VERSION: &str = "word-assist/0.1.0";

#[cfg(test)]
mod tests {
    #[test]
    fn engine_version_is_calibrated() {
        // Cheap guard against accidental bumping; format is "word-assist/x.y.z".
        let v = super::ENGINE_VERSION;
        assert!(v.starts_with("word-assist/"));
        assert_eq!(v.matches('.').count(), 2);
    }
}
