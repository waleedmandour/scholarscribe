//! Abstract Generator — uses a locally-installed LLM (via Ollama) to produce
//! a structured abstract (Background/Methods/Results/Conclusions) from the
//! manuscript body. Also generates section-by-section commentary.

use serde::{Deserialize, Serialize};
use std::time::Duration;

const ABSTRACT_OLLAMA_BASE: &str = "http://127.0.0.1:11434";

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: Option<ChatMessage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AbstractRequest {
    pub model: String,
    pub draft_text: String,
    pub max_words: Option<usize>,
    pub venue: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AbstractResult {
    pub abstract_text: String,
    pub model_used: String,
    pub prompt_tokens: usize,
    pub draft_length_chars: usize,
}

#[derive(Debug, Serialize)]
pub struct AbstractError {
    pub kind: String,
    pub message: String,
}

pub async fn generate_abstract(
    client: &reqwest::Client,
    req: AbstractRequest,
) -> Result<AbstractResult, AbstractError> {
    let status_resp = client
        .get(format!("{}/api/tags", ABSTRACT_OLLAMA_BASE))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|_| AbstractError {
            kind: "ollama_not_running".into(),
            message: format!("Ollama is not running on {}.", ABSTRACT_OLLAMA_BASE),
        })?;
    if !status_resp.status().is_success() {
        return Err(AbstractError {
            kind: "ollama_not_running".into(),
            message: "Ollama returned an error status.".into(),
        });
    }

    let max_words = req.max_words.unwrap_or(250);
    let venue = req.venue.unwrap_or_else(|| "general academic".into());
    let draft_excerpt = if req.draft_text.len() > 12000 {
        format!("{}...[truncated]", &req.draft_text[..12000])
    } else {
        req.draft_text.clone()
    };

    let system_prompt = format!(
        "You are an academic writing assistant. Generate a structured abstract for the manuscript below. The abstract should be at most {} words and suitable for a {} venue.\n\nFormat the abstract with these four labeled sections, each as a single paragraph:\n\nBackground: <context and motivation>\nMethods: <what was done>\nResults: <key findings>\nConclusions: <interpretation and significance>\n\nDo NOT include citations, do NOT include the word 'Abstract' as a header, do NOT add commentary. Output only the four labeled paragraphs.",
        max_words, venue
    );

    let user_prompt = format!(
        "Manuscript:\n\n{}\n\nGenerate the structured abstract now.",
        draft_excerpt
    );

    let body = serde_json::json!({
        "model": req.model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_prompt }
        ],
        "stream": false,
        "options": { "temperature": 0.4 }
    });

    let resp = client
        .post(format!("{}/api/chat", ABSTRACT_OLLAMA_BASE))
        .json(&body)
        .timeout(Duration::from_secs(300))
        .send()
        .await
        .map_err(|e| AbstractError {
            kind: "generation_failed".into(),
            message: format!("LLM request failed: {}", e),
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AbstractError {
            kind: "generation_failed".into(),
            message: format!("Ollama returned HTTP {}: {}", status, body),
        });
    }

    let parsed: ChatResponse = resp.json().await.map_err(|e| AbstractError {
        kind: "generation_failed".into(),
        message: format!("Cannot parse LLM response: {}", e),
    })?;

    let abstract_text = parsed
        .message
        .ok_or_else(|| AbstractError {
            kind: "generation_failed".into(),
            message: "Ollama returned an empty response.".into(),
        })?
        .content;

    Ok(AbstractResult {
        abstract_text,
        model_used: req.model,
        prompt_tokens: system_prompt.len() / 4 + user_prompt.len() / 4,
        draft_length_chars: req.draft_text.len(),
    })
}

// ---------- v0.2.0: Section-Aware Abstract Generator ----------

#[derive(Debug, Serialize)]
pub struct SectionCommentary {
    pub section_name: String,
    pub summary: String,
}

#[derive(Debug, Serialize)]
pub struct SectionCommentaryResult {
    pub commentaries: Vec<SectionCommentary>,
    pub model_used: String,
    pub draft_length_chars: usize,
}

pub async fn generate_section_commentary(
    client: &reqwest::Client,
    model: String,
    draft_text: String,
) -> Result<SectionCommentaryResult, AbstractError> {
    let status = client
        .get(format!("{}/api/tags", ABSTRACT_OLLAMA_BASE))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|_| AbstractError {
            kind: "ollama_not_running".into(),
            message: format!("Ollama is not running on {}.", ABSTRACT_OLLAMA_BASE),
        })?;
    if !status.status().is_success() {
        return Err(AbstractError {
            kind: "ollama_not_running".into(),
            message: "Ollama returned an error.".into(),
        });
    }

    let structure = crate::structure_analyzer::analyze_text(&draft_text);

    let sections: Vec<(String, String)> = if structure.headings.is_empty() {
        let words: Vec<&str> = draft_text.split_whitespace().collect();
        let mut out = Vec::new();
        for (i, chunk) in words.chunks(500).enumerate() {
            out.push((format!("Section {}", i + 1), chunk.join(" ")));
        }
        out
    } else {
        let mut out = Vec::new();
        for (i, h) in structure.headings.iter().enumerate() {
            let start = if i == 0 {
                0
            } else {
                draft_text.find(&h.text).unwrap_or(0)
            };
            let end = if i + 1 < structure.headings.len() {
                draft_text
                    .find(&structure.headings[i + 1].text)
                    .unwrap_or(draft_text.len())
            } else {
                draft_text.len()
            };
            let section_text = &draft_text[start.min(end)..end];
            out.push((h.text.clone(), section_text.to_string()));
        }
        out
    };

    if sections.is_empty() {
        return Err(AbstractError {
            kind: "generation_failed".into(),
            message: "No sections could be identified in the draft.".into(),
        });
    }

    let system_prompt = "You are an academic writing assistant. For each section of the manuscript provided, write a 1-2 sentence summary of what that section contributes to the overall argument. Output each summary on its own line, prefixed by the section name and a colon. Be concise and factual.";

    let mut sections_text = String::new();
    for (name, text) in &sections {
        let excerpt = if text.len() > 2000 {
            format!("{}...", &text[..2000])
        } else {
            text.clone()
        };
        sections_text.push_str(&format!("\n\n## {}\n{}", name, excerpt));
    }

    let user_prompt = format!(
        "Manuscript sections:\n{}\n\nProvide a 1-2 sentence summary for each section.",
        sections_text
    );

    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_prompt }
        ],
        "stream": false,
        "options": { "temperature": 0.3 }
    });

    let resp = client
        .post(format!("{}/api/chat", ABSTRACT_OLLAMA_BASE))
        .json(&body)
        .timeout(Duration::from_secs(300))
        .send()
        .await
        .map_err(|e| AbstractError {
            kind: "generation_failed".into(),
            message: format!("LLM request failed: {}", e),
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AbstractError {
            kind: "generation_failed".into(),
            message: format!("Ollama returned HTTP {}: {}", status, body),
        });
    }

    let parsed: ChatResponse = resp.json().await.map_err(|e| AbstractError {
        kind: "generation_failed".into(),
        message: format!("Cannot parse LLM response: {}", e),
    })?;

    let raw_output = parsed
        .message
        .ok_or_else(|| AbstractError {
            kind: "generation_failed".into(),
            message: "Ollama returned an empty response.".into(),
        })?
        .content;

    let mut commentaries = Vec::new();
    for line in raw_output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(colon_idx) = line.find(':') {
            let name = line[..colon_idx]
                .trim()
                .trim_start_matches('#')
                .trim()
                .to_string();
            let summary = line[colon_idx + 1..].trim().to_string();
            if !summary.is_empty() {
                commentaries.push(SectionCommentary {
                    section_name: name,
                    summary,
                });
            }
        }
    }

    if commentaries.is_empty() {
        commentaries.push(SectionCommentary {
            section_name: "Overall".into(),
            summary: raw_output,
        });
    }

    Ok(SectionCommentaryResult {
        commentaries,
        model_used: model,
        draft_length_chars: draft_text.len(),
    })
}
