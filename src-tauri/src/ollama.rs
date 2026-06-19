//! Ollama HTTP client.
//!
//! All requests go to http://127.0.0.1:11434 (Ollama's default bind address).
//! We never contact any other host. If Ollama is not installed or not running,
//! the user gets a clear, actionable error in the UI.

use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

const OLLAMA_BASE: &str = "http://127.0.0.1:11434";

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
/// or any other model Ollama supports. Catalog is grouped by use-case in the
/// UI via the `tags` field. Sizes and RAM requirements are approximate.
pub fn recommended_models() -> Vec<RecommendedModel> {
    vec![
        // ============ Latest generation (2025) ============
        RecommendedModel {
            name: "gemma3:4b".into(),
            label: "Gemma 3 4B (Google, 2025)".into(),
            size_gb: 2.5,
            min_ram_gb: 8,
            tags: vec!["lightweight".into(), "multimodal".into(), "academic".into()],
            description: "Google's latest Gemma 3 with strong multilingual support and improved reasoning. Multimodal (text + image). Excellent for academic drafting on modest hardware.".into(),
        },
        RecommendedModel {
            name: "gemma3:12b".into(),
            label: "Gemma 3 12B (Google, 2025)".into(),
            size_gb: 7.4,
            min_ram_gb: 16,
            tags: vec!["balanced".into(), "multimodal".into(), "academic".into()],
            description: "Mid-size Gemma 3. Strong on structured academic prose, citation-aware reasoning, and long-context work. Recommended for researchers with 16 GB+ RAM.".into(),
        },
        RecommendedModel {
            name: "gemma3:27b".into(),
            label: "Gemma 3 27B (Google, 2025)".into(),
            size_gb: 16.4,
            min_ram_gb: 32,
            tags: vec!["high-quality".into(), "multimodal".into(), "academic".into()],
            description: "Top-tier Gemma 3. Approaches proprietary-model quality on academic tasks. Requires 32 GB+ RAM. Best for final manuscript polishing and complex reasoning.".into(),
        },
        RecommendedModel {
            name: "qwen3:8b".into(),
            label: "Qwen 3 8B (Alibaba, 2025)".into(),
            size_gb: 4.9,
            min_ram_gb: 16,
            tags: vec!["multilingual".into(), "academic".into(), "reasoning".into()],
            description: "Latest Qwen with hybrid thinking-mode. Outstanding on multilingual academic text (English, Chinese, Arabic, European languages). Strong on math and logic.".into(),
        },
        RecommendedModel {
            name: "qwen3:14b".into(),
            label: "Qwen 3 14B (Alibaba, 2025)".into(),
            size_gb: 8.7,
            min_ram_gb: 24,
            tags: vec!["multilingual".into(), "academic".into(), "high-quality".into()],
            description: "Larger Qwen 3. Excellent for academic-style prose in 30+ languages. Includes thinking-mode toggle for deliberate reasoning.".into(),
        },
        RecommendedModel {
            name: "qwen3:32b".into(),
            label: "Qwen 3 32B (Alibaba, 2025)".into(),
            size_gb: 19.5,
            min_ram_gb: 32,
            tags: vec!["multilingual".into(), "academic".into(), "high-quality".into()],
            description: "Flagship Qwen 3. Competitive with GPT-4-class models on academic benchmarks. Requires 32 GB+ RAM.".into(),
        },
        RecommendedModel {
            name: "llama3.3:70b".into(),
            label: "Llama 3.3 70B (Meta, 2024)".into(),
            size_gb: 40.0,
            min_ram_gb: 64,
            tags: vec!["high-quality".into(), "academic".into(), "long-context".into()],
            description: "Meta's flagship open model. State-of-the-art on academic reasoning. Requires 64 GB+ RAM — only for high-end workstations.".into(),
        },
        RecommendedModel {
            name: "phi4:14b".into(),
            label: "Phi-4 14B (Microsoft, 2024)".into(),
            size_gb: 8.4,
            min_ram_gb: 16,
            tags: vec!["reasoning".into(), "academic".into(), "stem".into()],
            description: "Microsoft's latest Phi. Specifically trained on synthetic academic data — exceptional on STEM, math, and structured reasoning. Surprisingly strong for its size.".into(),
        },
        RecommendedModel {
            name: "deepseek-r1:14b".into(),
            label: "DeepSeek R1 14B (DeepSeek, 2025)".into(),
            size_gb: 8.4,
            min_ram_gb: 16,
            tags: vec!["reasoning".into(), "academic".into(), "research".into()],
            description: "Open reasoning model with explicit chain-of-thought. Excellent for literature review synthesis, argument structuring, and methodology critique.".into(),
        },
        RecommendedModel {
            name: "deepseek-r1:32b".into(),
            label: "DeepSeek R1 32B (DeepSeek, 2025)".into(),
            size_gb: 19.5,
            min_ram_gb: 32,
            tags: vec!["reasoning".into(), "academic".into(), "research".into(), "high-quality".into()],
            description: "Larger DeepSeek R1. Top-tier open reasoning model. Recommended for serious academic work where reasoning quality matters more than speed.".into(),
        },
        // ============ Lightweight / older-hardware friendly ============
        RecommendedModel {
            name: "gemma3:1b".into(),
            label: "Gemma 3 1B (Google, 2025)".into(),
            size_gb: 0.8,
            min_ram_gb: 4,
            tags: vec!["lightweight".into(), "fast".into()],
            description: "Smallest Gemma 3. Runs on 4 GB RAM devices. Good for quick edits and proofreading on the move.".into(),
        },
        RecommendedModel {
            name: "qwen3:4b".into(),
            label: "Qwen 3 4B (Alibaba, 2025)".into(),
            size_gb: 2.5,
            min_ram_gb: 8,
            tags: vec!["lightweight".into(), "multilingual".into()],
            description: "Compact Qwen 3 with thinking mode. Best small-model option for non-English academic text.".into(),
        },
        RecommendedModel {
            name: "phi4-mini:3.8b".into(),
            label: "Phi-4 Mini 3.8B (Microsoft, 2024)".into(),
            size_gb: 2.3,
            min_ram_gb: 8,
            tags: vec!["lightweight".into(), "stem".into()],
            description: "Compact Phi-4 with strong STEM reasoning. Best small model for technical/quantitative writing.".into(),
        },
        // ============ Specialized academic ============
        RecommendedModel {
            name: "hf.co/sciver/sciver-6b:Q4_K_M".into(),
            label: "SciGLM 6B (Academic-tuned, Tsinghua)".into(),
            size_gb: 3.5,
            min_ram_gb: 8,
            tags: vec!["academic".into(), "research".into()],
            description: "Academic-tuned model from Tsinghua/KEG. Trained on scientific papers — strong on literature review and technical writing. Uses HuggingFace direct pull syntax.".into(),
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
