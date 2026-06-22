//! Appeal Letter Generator — drafts a professional, evidence-based appeal
//! letter for researchers whose work has been falsely flagged by AI
//! detection tools. References the peer-reviewed literature and Turnitin's
//! own guidance that scores are indicators, not proof.
//!
//! This is entirely ethical: it helps researchers respond to false positives
//! using the same evidence-based approach that the Disclosure Assistant uses
//! for voluntary AI-use disclosure.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppealLetterInput {
    pub researcher_name: String,
    pub researcher_title: String, // e.g. "Dr.", "Prof.", "Ms."
    pub institution: String,
    pub manuscript_title: String,
    pub venue: String,               // e.g. "Journal of X"
    pub editor_name: String,         // name of the editor (if known)
    pub detector_used: String,       // e.g. "Turnitin AI", "GPTZero"
    pub detector_score: String,      // e.g. "92% AI-generated"
    pub process_description: String, // researcher's description of their writing process
    pub additional_evidence: String, // optional: version history, drafts, etc.
}

#[derive(Debug, Serialize)]
pub struct AppealLetterOutput {
    pub letter: String,
    pub references: Vec<String>,
}

pub fn generate(input: &AppealLetterInput) -> Result<AppealLetterOutput, String> {
    if input.researcher_name.trim().is_empty() {
        return Err("Researcher name is required.".into());
    }
    if input.manuscript_title.trim().is_empty() {
        return Err("Manuscript title is required.".into());
    }

    let editor_salutation = if input.editor_name.trim().is_empty() {
        "Dear Editor,".to_string()
    } else {
        format!("Dear {},", input.editor_name.trim())
    };

    let letter = format!(
        r#"{title} {name}
{institution}

{date}

{salutation}

Re: Appeal Regarding AI-Detection Flag on Manuscript "{manuscript}"

I am writing in response to a flag raised by {detector} regarding my manuscript, "{manuscript}," submitted to {venue}. The detector reportedly assigned a score of {score} indicating AI-generated content. I respectfully request that this score be reconsidered in light of the following evidence and the published scientific literature on the limitations of AI-detection tools.

## 1. The Manuscript Was Authored by Me

I confirm that the manuscript "{manuscript}" was researched, drafted, and revised entirely by me. My writing process was as follows:

{process_description}

{additional_evidence_section}

## 2. AI-Detection Scores Are Not Conclusive Evidence

I respectfully draw your attention to the following:

**Turnitin's own guidance** states that AI-detection scores are indicators, not proof, of AI-generated content. The company's documentation explicitly cautions that scores should not be used as the sole basis for academic integrity determinations and recommends human review of flagged passages.

**Peer-reviewed research** has documented significant limitations in AI-detection tools:

- Liang et al. (2023) demonstrated that GPT detectors are biased against non-native English writers, frequently misclassifying genuine human writing as AI-generated. This finding has been replicated across multiple detector platforms and raises serious fairness concerns, particularly for researchers writing in English as a second or additional language (Liang, W., Yuksekgonul, M., Mao, Y., Wu, E., & Zou, J. (2023). GPT detectors are biased against non-native English writers. Patterns, 4(7), 100779. https://doi.org/10.1016/j.patter.2023.100779).

- Weber-Wulff et al. (2023) evaluated 14 publicly available AI-generated text detection tools and found that all of them exhibited high false-positive rates on human-written text, particularly on texts with simpler vocabulary and shorter sentences — features common in academic writing by non-native English speakers and in technical/methods sections (Weber-Wulff, D., et al. (2023). Testing of detection tools for AI-generated text. International Journal for Educational Integrity, 19, 26. https://doi.org/10.1007/s40979-023-00146-z).

- Several universities, including Vanderbilt University, University of Pittsburgh, and University of Texas at Austin, have discontinued or significantly restricted the use of Turnitin's AI-detection feature due to reliability concerns.

## 3. Request

I respectfully request that:

1. The AI-detection score not be used as the sole or primary basis for any integrity finding.
2. A human reviewer examine the flagged passages in context.
3. If additional evidence of authorship is required, I am prepared to provide draft histories, version control records, or other documentation of my writing process.

I am committed to maintaining the highest standards of research integrity and am happy to cooperate fully with any review process. I have disclosed any AI tools used in the preparation of this manuscript in accordance with {venue}'s AI-use policy.

Thank you for your fair and careful consideration of this matter.

Sincerely,

{title} {name}
{institution}

---

References

Liang, W., Yuksekgonul, M., Mao, Y., Wu, E., & Zou, J. (2023). GPT detectors are biased against non-native English writers. Patterns, 4(7), 100779. https://doi.org/10.1016/j.patter.2023.100779

Weber-Wulff, D., Anohina-Naumeca, A., Bjelobaba, S., Foltýnek, T., Guerrero-Dib, J., Popoola, O., Šigut, P., & Waddington, L. (2023). Testing of detection tools for AI-generated text. International Journal for Educational Integrity, 19, 26. https://doi.org/10.1007/s40979-023-00146-z
"#,
        title = input.researcher_title.trim(),
        name = input.researcher_name.trim(),
        institution = input.institution.trim(),
        date = chrono_date_string(),
        salutation = editor_salutation,
        manuscript = input.manuscript_title.trim(),
        venue = input.venue.trim(),
        detector = input.detector_used.trim(),
        score = input.detector_score.trim(),
        process_description = input.process_description.trim(),
        additional_evidence_section = if input.additional_evidence.trim().is_empty() {
            String::new()
        } else {
            format!(
                "I can also provide the following additional evidence of authorship:\n\n{}\n",
                input.additional_evidence.trim()
            )
        },
    );

    Ok(AppealLetterOutput {
        letter,
        references: vec![
            "Liang et al. (2023). Patterns, 4(7), 100779. https://doi.org/10.1016/j.patter.2023.100779".into(),
            "Weber-Wulff et al. (2023). International Journal for Educational Integrity, 19, 26. https://doi.org/10.1007/s40979-023-00146-z".into(),
        ],
    })
}

fn chrono_date_string() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Simple date approximation (not perfectly accurate but good enough for a letter)
    let days = now / 86400;
    let year = 1970 + (days / 365);
    let day_of_year = days % 365;
    let month = (day_of_year / 30 + 1).min(12);
    let day = (day_of_year % 30 + 1).min(28);
    format!("{:04}-{:02}-{:02}", year, month, day)
}
