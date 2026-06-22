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

/// Generate section-by-section commentary — brief summaries of what each
/// major section contributes to the argument. Doubles as an outline tool.
pub async fn generate_section_commentary(
    client: &reqwest::Client,
    model: String,
    draft_text: String,
) -> Result<SectionCommentaryResult, AbstractError> {
    // Check Ollama is running
    let status = client
        .get(format!("{}/api/tags", OLLAMA_BASE))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|_| AbstractError {
            kind: "ollama_not_running".into(),
            message: format!("Ollama is not running on {}.", OLLAMA_BASE),
        })?;
    if !status.status().is_success() {
        return Err(AbstractError {
            kind: "ollama_not_running".into(),
            message: "Ollama returned an error. Please restart it.".into(),
        });
    }

    // Use structure_analyzer to identify sections
    let structure = crate::structure_analyzer::analyze_text(&draft_text);

    // If no headings detected, split by paragraphs into ~500-word sections
    let sections: Vec<(String, String)> = if structure.headings.is_empty() {
        let words: Vec<&str> = draft_text.split_whitespace().collect();
        let chunk_size = 500;
        let mut out = Vec::new();
        for (i, chunk) in words.chunks(chunk_size).enumerate() {
            out.push((format!("Section {}", i + 1), chunk.join(" ")));
        }
        out
    } else {
        // Extract text between headings
        let mut out = Vec::new();
        let text_bytes = draft_text.as_bytes();
        for (i, h) in structure.headings.iter().enumerate() {
            let start = if i == 0 {
                0
            } else {
                // Find the heading text in the draft
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

    let system_prompt = "You are an academic writing assistant. For each section of the manuscript provided, write a 1-2 sentence summary of what that section contributes to the overall argument. Output each summary on its own line, prefixed by the section name and a colon. Be concise and factual. Do not add commentary or suggestions.";

    let mut sections_text = String::new();
    for (name, text) in &sections {
        // Truncate each section to ~2000 chars to keep prompt manageable
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
        .post(format!("{}/api/chat", OLLAMA_BASE))
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

    // Parse the LLM output into section commentaries
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

    // If parsing didn't produce anything useful, return the raw output as a single entry
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
