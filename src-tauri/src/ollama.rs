//! Ollama HTTP client.
//!
//! All requests go to http://127.0.0.1:11434 (Ollama's default bind address).
//! We never contact any other host. If Ollama is not installed or not running,
//! the user gets a clear, actionable error in the UI.

use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

const OLLAMA_BASE: &str = "http://127.0.0.1:11434";

#[derive(Default)]
pub struct OllamaState {
    pub client: reqwest::Client,
}

#[derive(Debug, Error)]
pub enum OllamaError {
    #[error("Ollama is not running on {base}. Please start the Ollama app or run `ollama serve` in a terminal, then try again.")]
    NotRunning { base: String },
    #[error("Ollama returned HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Invalid response: {0}")]
    Parse(String),
}

impl serde::Serialize for OllamaError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub size: u64,
    pub digest: String,
    pub modified_at: String,
}

#[derive(Debug, Deserialize)]
struct ListResponse {
    models: Option<Vec<ModelInfo>>,
}

pub async fn status(state: &OllamaState) -> Result<bool, OllamaError> {
    let resp = state
        .client
        .get(format!("{}/api/tags", OLLAMA_BASE))
        .timeout(Duration::from_secs(3))
        .send()
        .await;
    Ok(resp.map(|r| r.status().is_success()).unwrap_or(false))
}

pub async fn list_models(state: &OllamaState) -> Result<Vec<ModelInfo>, OllamaError> {
    let resp = state
        .client
        .get(format!("{}/api/tags", OLLAMA_BASE))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|_| OllamaError::NotRunning {
            base: OLLAMA_BASE.to_string(),
        })?;

    if !resp.status().is_success() {
        return Err(OllamaError::Http {
            status: resp.status().as_u16(),
            body: resp.text().await.unwrap_or_default(),
        });
    }

    let parsed: ListResponse = resp
        .json()
        .await
        .map_err(|e| OllamaError::Parse(e.to_string()))?;
    Ok(parsed.models.unwrap_or_default())
}

/// Streams `ollama pull` progress events back to the frontend via Tauri events.
pub async fn pull_model(
    app: tauri::AppHandle,
    state: &OllamaState,
    name: String,
) -> Result<(), OllamaError> {
    let body = serde_json::json!({ "name": name, "stream": true });
    let resp = state
        .client
        .post(format!("{}/api/pull", OLLAMA_BASE))
        .json(&body)
        .timeout(Duration::from_secs(3600))
        .send()
        .await
        .map_err(|_| OllamaError::NotRunning {
            base: OLLAMA_BASE.to_string(),
        })?;

    if !resp.status().is_success() {
        return Err(OllamaError::Http {
            status: resp.status().as_u16(),
            body: resp.text().await.unwrap_or_default(),
        });
    }

    use futures_util::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(OllamaError::Network)?;
        buf.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(idx) = buf.find('\n') {
            let line: String = buf.drain(..=idx).collect();
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
                let _ = tauri::Emitter::emit(
                    &app,
                    "ollama://pull-progress",
                    PullProgress {
                        model: name.clone(),
                        status: v
                            .get("status")
                            .and_then(|s| s.as_str())
                            .unwrap_or("")
                            .to_string(),
                        completed: v.get("completed").and_then(|c| c.as_u64()).unwrap_or(0),
                        total: v.get("total").and_then(|t| t.as_u64()).unwrap_or(0),
                    },
                );
            }
        }
    }
    Ok(())
}

#[derive(Clone, Serialize)]
pub struct PullProgress {
    pub model: String,
    pub status: String,
    pub completed: u64,
    pub total: u64,
}

pub async fn delete_model(state: &OllamaState, name: &str) -> Result<(), OllamaError> {
    let resp = state
        .client
        .delete(format!("{}/api/delete", OLLAMA_BASE))
        .json(&serde_json::json!({ "name": name }))
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|_| OllamaError::NotRunning {
            base: OLLAMA_BASE.to_string(),
        })?;

    if !resp.status().is_success() {
        return Err(OllamaError::Http {
            status: resp.status().as_u16(),
            body: resp.text().await.unwrap_or_default(),
        });
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

fn default_temperature() -> f32 {
    0.7
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: Option<ChatMessage>,
}

pub async fn chat(state: &OllamaState, req: ChatRequest) -> Result<ChatMessage, OllamaError> {
    let body = serde_json::json!({
        "model": req.model,
        "messages": req.messages,
        "stream": false,
        "options": { "temperature": req.temperature }
    });
    let resp = state
        .client
        .post(format!("{}/api/chat", OLLAMA_BASE))
        .json(&body)
        .timeout(Duration::from_secs(300))
        .send()
        .await
        .map_err(|_| OllamaError::NotRunning {
            base: OLLAMA_BASE.to_string(),
        })?;

    if !resp.status().is_success() {
        return Err(OllamaError::Http {
            status: resp.status().as_u16(),
            body: resp.text().await.unwrap_or_default(),
        });
    }

    let parsed: ChatResponse = resp
        .json()
        .await
        .map_err(|e| OllamaError::Parse(e.to_string()))?;
    parsed
        .message
        .ok_or_else(|| OllamaError::Parse("empty message in Ollama response".into()))
}

/// Curated catalog of recommended models. The user can install any of these
/// or any other model Ollama supports.
pub fn recommended_models() -> Vec<RecommendedModel> {
    vec![
        RecommendedModel {
            name: "gemma2:9b".into(),
            label: "Gemma 2 9B (Google)".into(),
            size_gb: 5.4,
            min_ram_gb: 16,
            tags: vec!["balanced".into(), "general".into()],
            description: "Google's open model. Strong general writing and reasoning quality for its size.".into(),
        },
        RecommendedModel {
            name: "gemma2:2b".into(),
            label: "Gemma 2 2B (Google)".into(),
            size_gb: 1.6,
            min_ram_gb: 8,
            tags: vec!["lightweight".into(), "fast".into()],
            description: "Compact version. Runs on laptops with 8 GB RAM. Good for quick edits.".into(),
        },
        RecommendedModel {
            name: "qwen2.5:7b".into(),
            label: "Qwen 2.5 7B (Alibaba)".into(),
            size_gb: 4.7,
            min_ram_gb: 16,
            tags: vec!["multilingual".into(), "academic".into()],
            description: "Excellent multilingual support, strong on academic-style prose. Good citation-aware.".into(),
        },
        RecommendedModel {
            name: "qwen2.5:3b".into(),
            label: "Qwen 2.5 3B (Alibaba)".into(),
            size_gb: 2.0,
            min_ram_gb: 8,
            tags: vec!["lightweight".into(), "multilingual".into()],
            description: "Lightweight multilingual option. Good for older hardware.".into(),
        },
        RecommendedModel {
            name: "llama3.1:8b".into(),
            label: "Llama 3.1 8B (Meta)".into(),
            size_gb: 4.9,
            min_ram_gb: 16,
            tags: vec!["general".into(), "strong-reasoning".into()],
            description: "Meta's open model. Solid all-rounder with good instruction-following.".into(),
        },
        RecommendedModel {
            name: "phi3:mini".into(),
            label: "Phi-3 Mini 3.8B (Microsoft)".into(),
            size_gb: 2.3,
            min_ram_gb: 8,
            tags: vec!["lightweight".into(), "fast".into()],
            description: "Very small model with surprisingly strong reasoning. Best for low-RAM devices.".into(),
        },
    ]
}

#[derive(Debug, Serialize, Clone)]
pub struct RecommendedModel {
    pub name: String,
    pub label: String,
    pub size_gb: f64,
    pub min_ram_gb: u32,
    pub tags: Vec<String>,
    pub description: String,
}

#[allow(dead_code)]
pub fn base_url() -> &'static str {
    OLLAMA_BASE
}

impl OllamaState {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent(concat!("ScholarScribe/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("failed to build reqwest client"),
        }
    }
}

impl Default for OllamaState {
    fn default() -> Self {
        Self::new()
    }
}
