//! Tier 1 — the local-model rewrite pass (Ollama), pure in Phase 1a.
//!
//! 1a ships the versioned prompt, the response schema, and the request
//! builder as PURE code: no socket, no daemon call, no async. 1b's
//! scheduler will put these pieces onto `ollama::chat` with the additive
//! ChatRequest fields (ruling D-3) and an injectable base URL.
//!
//! DESIGN (precision over recall, recon R7):
//! - One sentence per request. Short context, small blast radius, and the
//!   validators in `validate` are per-sentence by construction.
//! - Structured output via Ollama's `format` JSON schema; a schema
//!   violation or unparseable answer is "no suggestion", never an error
//!   shown to the user (the annotation simply does not appear).
//! - The prompt is a versioned constant; the active version travels in
//!   check responses (1b) and ledger entries (Phase 3).
//! - `think` is OFF: chain-of-thought output must never leak into the
//!   `revised` field, and thinking-mode semantics vary by Ollama version.

use serde::{Deserialize, Serialize};

use super::validate::rules;
use super::validate::Rejection;

/// Prompt v1 (2026-10, feature/word-assist). Deliberately short: long
/// prompts invite the model to "fix" protected content. The protected-
/// content rules repeat what `validate` enforces, so a compliant model
/// passes validation and a sloppy one gets rejected — the validator, not
/// the prompt, is the guarantee.
pub const TIER1_PROMPT_V1: &str = "\
You are an academic copy editor. Improve the grammar, clarity and flow of \
the sentence. Hard rules: keep the meaning; keep citations, numbers, URLs, \
DOIs, math and quoted text exactly as written; add no new content; do not \
split or merge sentences; write in the same language/script. Reply with \
JSON only.";

/// Structured-output schema sent as Ollama's `format` (additive ChatRequest
/// field, D-3). Minimal on purpose: every extra field is a way for a small
/// model to go off-script.
pub fn tier1_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "revised": { "type": "string" },
            "explanation": { "type": "string" }
        },
        "required": ["revised"],
        "additionalProperties": false
    })
}

/// The parsed Tier 1 model response (rule 1 of the six: schema).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Tier1Response {
    pub revised: String,
    #[serde(default)]
    pub explanation: Option<String>,
}

/// Parse and schema-validate a raw model completion. Tolerates the two
/// common small-model tics: markdown fences and leading/trailing prose.
/// Anything not recoverable as the schema is rejection rule 1.
pub fn parse_tier1_response(raw: &str) -> Result<Tier1Response, Rejection> {
    let candidate = strip_fences(raw.trim());
    let parsed: serde_json::Value =
        serde_json::from_str(candidate.trim()).map_err(|_| Rejection {
            rule: rules::SCHEMA,
            reason: "model reply is not JSON",
        })?;
    let revised = parsed
        .get("revised")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or(Rejection {
            rule: rules::SCHEMA,
            reason: "missing 'revised' string",
        })?;
    if revised.is_empty() {
        return Err(Rejection {
            rule: rules::SCHEMA,
            reason: "'revised' is empty",
        });
    }
    let explanation = parsed
        .get("explanation")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Ok(Tier1Response {
        revised,
        explanation,
    })
}

/// Strip ```json fences and a lone leading "JSON:" prefix.
fn strip_fences(s: &str) -> &str {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("```json") {
        return rest.strip_suffix("```").unwrap_or(rest).trim();
    }
    if let Some(rest) = s.strip_prefix("```") {
        return rest.strip_suffix("```").unwrap_or(rest).trim();
    }
    s
}

/// The pure Tier 1 request description. 1b maps this onto
/// `ollama::ChatRequest` (with `format` + `think` additive fields) and the
/// injectable base URL; keeping the shape here means the prompt/schema
/// contract is testable without a daemon.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tier1Request {
    pub model: String,
    pub system: String,
    pub user: String,
    /// JSON schema for Ollama's `format` field.
    pub format: serde_json::Value,
    /// Chain-of-thought stays off (see module docs).
    pub think: bool,
    /// Sampling: low temperature, deterministic-ish edits.
    pub temperature: f32,
}

/// Build the Tier 1 request for one original sentence. Pure; no I/O.
pub fn build_tier1_request(model: &str, original: &str) -> Tier1Request {
    Tier1Request {
        model: model.to_string(),
        system: TIER1_PROMPT_V1.to_string(),
        user: format!("Sentence: {original}\nJSON:"),
        format: tier1_schema(),
        think: false,
        temperature: 0.2,
    }
}

#[cfg(test)]
mod tests {
    use super::{build_tier1_request, parse_tier1_response, TIER1_PROMPT_V1};

    #[test]
    fn parses_clean_json() {
        let r = parse_tier1_response(
            r#"{"revised": "The results were clear.", "explanation": "agreement"}"#,
        )
        .expect("clean parse");
        assert_eq!(r.revised, "The results were clear.");
        assert_eq!(r.explanation.as_deref(), Some("agreement"));
    }

    #[test]
    fn parses_fenced_json() {
        let r = parse_tier1_response("```json\n{\"revised\": \"Ok sentence.\"}\n```")
            .expect("fenced parse");
        assert_eq!(r.revised, "Ok sentence.");
        assert!(r.explanation.is_none());
    }

    #[test]
    fn rejects_prose_reply() {
        let err = parse_tier1_response("Sure! The sentence looks fine to me.").unwrap_err();
        assert_eq!(err.rule, 1);
    }

    #[test]
    fn rejects_missing_field() {
        let err = parse_tier1_response(r#"{"explanation": "no revised field"}"#).unwrap_err();
        assert_eq!(err.rule, 1);
    }

    #[test]
    fn rejects_empty_revised() {
        let err = parse_tier1_response(r#"{"revised": ""}"#).unwrap_err();
        assert_eq!(err.rule, 1);
    }

    #[test]
    fn request_shape_is_stable() {
        let req = build_tier1_request("gemma3:4b", "The results was clear.");
        assert_eq!(req.model, "gemma3:4b");
        assert!(!req.think);
        assert!(req.temperature < 0.5);
        assert!(req.system.starts_with("You are an academic copy editor."));
        assert!(req.user.contains("The results was clear."));
        // The schema requires exactly the two known fields.
        let props = req.format["properties"].as_object().expect("schema props");
        assert!(props.contains_key("revised"));
        assert!(props.contains_key("explanation"));
    }

    #[test]
    fn prompt_pinpoints_protected_content() {
        // The prompt must name what the validator enforces, so doc drift
        // between prompt and validate.rs is caught here.
        let p = TIER1_PROMPT_V1;
        assert!(p.contains("citations"));
        assert!(p.contains("numbers"));
        assert!(p.contains("JSON only"));
        assert!(p.contains("same language/script"));
    }
}
