//! Proofread Phase 3 (v2.3.0). Tier 2 local-LLM minimal-edit suggestions.
//!
//! GATED: off by default. Uses existing Ollama client at 127.0.0.1:11434
//! (H1: no new host, no new HTTP crate). Dedicated system prompt NOT
//! inherited from Chat/WritingCoach (which forbid rewriting). H4 enforced
//! in code: caller drops fixes when was_clean=true. Pure core, no Tauri.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::grammar::DialectName;

pub const TIER2_ENGINE_NAME: &str = "Ollama (local)";

pub const TIER2_SYSTEM_PROMPT: &str = "\
You are a proofreading assistant. Given a single English sentence, return ONLY \
a JSON object: {\"fixes\": [{\"kind\": \"spelling\"|\"grammar\"|\"punctuation\", \
\"description\": \"<one short clause>\"}]}. If the sentence is already \
grammatical, return {\"fixes\": []}. Rules:\n\
1. Fix ONLY grammar, spelling, and punctuation errors. Never rewrite for style.\n\
2. Never invent citations, references, or facts.\n\
3. Never change the sentence's meaning.\n\
4. Never rewrite the sentence; only describe the fix in the description field.\n\
5. If you are unsure whether something is an error, return {\"fixes\": []}.\n\
6. Do not alter citations, numbers, quotations, or technical terms.";

#[derive(Debug, Error)]
pub enum Tier2Error {
    #[error("Tier 2 is not enabled. Turn it on in the Proofread tab first.")]
    NotEnabled,
    #[error("Ollama is not running at {base}. Start Ollama and try again.")]
    OllamaNotRunning { base: String },
    #[error("Ollama request failed: {0}")]
    RequestFailed(String),
    #[error("Could not parse Ollama response as JSON: {0}")]
    ParseFailed(String),
    #[error("No sentence provided. Tier 2 takes one sentence at a time.")]
    EmptyInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tier2Request {
    pub sentence: String,
    pub dialect: DialectName,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tier2Fix {
    pub kind: Tier2FixKind,
    pub description: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Tier2FixKind {
    Spelling,
    Grammar,
    Punctuation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tier2Result {
    pub fixes: Vec<Tier2Fix>,
    pub model: String,
    pub was_clean: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct LlmResponse {
    fixes: Vec<LlmFix>,
}

#[derive(Debug, Clone, Deserialize)]
struct LlmFix {
    kind: String,
    description: String,
}

pub async fn suggest_minimal_edit(
    client: &reqwest::Client,
    base_url: &str,
    request: &Tier2Request,
) -> Result<Tier2Result, Tier2Error> {
    if request.sentence.trim().is_empty() {
        return Err(Tier2Error::EmptyInput);
    }
    let chat_body = serde_json::json!({
        "model": request.model,
        "messages": [
            {"role": "system", "content": TIER2_SYSTEM_PROMPT},
            {"role": "user", "content": &request.sentence}
        ],
        "stream": false, "temperature": 0.0, "format": "json"
    });
    let url = format!("{}/api/chat", base_url);
    let resp = client
        .post(&url)
        .json(&chat_body)
        .send()
        .await
        .map_err(|e| Tier2Error::RequestFailed(format!("POST {} failed: {}", url, e)))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if status.as_u16() == 404 || body.contains("connection refused") {
            return Err(Tier2Error::OllamaNotRunning {
                base: base_url.to_string(),
            });
        }
        return Err(Tier2Error::RequestFailed(format!(
            "HTTP {} {}",
            status,
            body.chars().take(200).collect::<String>()
        )));
    }
    let resp_json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| Tier2Error::ParseFailed(format!("response body not JSON: {}", e)))?;
    let content = resp_json
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .ok_or_else(|| Tier2Error::ParseFailed("missing message.content".to_string()))?;
    let llm_resp: LlmResponse = serde_json::from_str(content)
        .map_err(|e| Tier2Error::ParseFailed(format!("content not valid JSON: {}", e)))?;
    let fixes: Vec<Tier2Fix> = llm_resp
        .fixes
        .into_iter()
        .filter_map(|f| {
            let kind = match f.kind.to_lowercase().as_str() {
                "spelling" => Tier2FixKind::Spelling,
                "grammar" => Tier2FixKind::Grammar,
                "punctuation" => Tier2FixKind::Punctuation,
                _ => return None,
            };
            Some(Tier2Fix {
                kind,
                description: f.description,
            })
        })
        .collect();
    let was_clean = fixes.is_empty();
    Ok(Tier2Result {
        fixes,
        model: request.model.clone(),
        was_clean,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_forbids_rewriting() {
        assert!(TIER2_SYSTEM_PROMPT.contains("Never rewrite"));
        assert!(TIER2_SYSTEM_PROMPT.contains("Fix ONLY grammar"));
    }
    #[test]
    fn tier2_result_clean_when_no_fixes() {
        let r = Tier2Result {
            fixes: vec![],
            model: "test".to_string(),
            was_clean: true,
        };
        assert!(r.was_clean);
    }
    #[test]
    fn tier2_result_not_clean_with_fixes() {
        let r = Tier2Result {
            fixes: vec![Tier2Fix {
                kind: Tier2FixKind::Spelling,
                description: "fix".to_string(),
            }],
            model: "test".to_string(),
            was_clean: false,
        };
        assert!(!r.was_clean);
    }
    #[test]
    fn empty_input_rejected() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = reqwest::Client::new();
        let req = Tier2Request {
            sentence: "   ".to_string(),
            dialect: DialectName::American,
            model: "test".to_string(),
        };
        let result = rt.block_on(suggest_minimal_edit(
            &client,
            "http://127.0.0.1:11434",
            &req,
        ));
        assert!(matches!(result, Err(Tier2Error::EmptyInput)));
    }
}
