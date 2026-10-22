//! AI-use disclosure statement generator.
//!
//! Generates a disclosure statement that complies with the AI-use policies of
//! major academic venues. The user fills in: venue, AI tool(s) used, stage(s)
//! of assistance, and a brief description. The module returns a properly
//! formatted statement the user can paste into their manuscript or cover letter.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VenueTemplate {
    pub id: String,
    pub label: String,
    pub policy_url: String,
    pub requires_in_manuscript: bool,
    pub requires_in_cover_letter: bool,
    pub template: String,
    pub notes: String,
}

pub fn venue_templates() -> Vec<VenueTemplate> {
    vec![
        VenueTemplate {
            id: "icmje".into(),
            label: "ICMJE (medical journals)".into(),
            policy_url: "https://www.icmje.org/recommendations/".into(),
            requires_in_manuscript: true,
            requires_in_cover_letter: true,
            template: "During the preparation of this work the author(s) used [TOOL NAME AND VERSION] in order to [REASON FOR USE]. After using this tool, the author(s) reviewed and edited the content as needed and take(s) full responsibility for the content of the published article.".into(),
            notes: "Required by JAMA, NEJM, The Lancet, BMJ, Annals of Internal Medicine, and ~5000 ICMJE-following journals.".into(),
        },
        VenueTemplate {
            id: "nature".into(),
            label: "Nature Portfolio journals".into(),
            policy_url: "https://www.nature.com/editorial-policies/ai".into(),
            requires_in_manuscript: true,
            requires_in_cover_letter: false,
            template: "AI-assisted technologies were used in the preparation of this manuscript. Specifically, [TOOL] was used to [TASK]. The author(s) reviewed and edited all AI-generated content and take full responsibility for the content of this publication.".into(),
            notes: "Nature does not permit AI to be listed as an author. LLMs may be used for language refinement only, not for generating scientific content or data.".into(),
        },
        VenueTemplate {
            id: "ieee".into(),
            label: "IEEE".into(),
            policy_url: "https://journals.ieeeauthorcenter.ieee.org/".into(),
            requires_in_manuscript: true,
            requires_in_cover_letter: false,
            template: "This work used [TOOL] for [TASK]. The author reviewed and edited the output and takes full responsibility for the content.".into(),
            notes: "IEEE requires disclosure in the acknowledgements section. AI cannot be an author.".into(),
        },
        VenueTemplate {
            id: "elsevier".into(),
            label: "Elsevier".into(),
            policy_url: "https://www.elsevier.com/about/policies-and-standards/ai-author".into(),
            requires_in_manuscript: true,
            requires_in_cover_letter: true,
            template: "During the preparation of this work the author(s) used [TOOL] in order to [TASK]. After using this tool, the author(s) reviewed and edited the content as needed and take(s) full responsibility for the content of the publication.".into(),
            notes: "Elsevier does not permit AI to be listed as an author or to hold copyright. Disclosure required in both manuscript and cover letter.".into(),
        },
        VenueTemplate {
            id: "acl".into(),
            label: "ACL / EMNLP / NAACL".into(),
            policy_url: "https://www.aclweb.org/adminwiki/index.php?title=ACL_Policy_on_Publication_Ethics".into(),
            requires_in_manuscript: true,
            requires_in_cover_letter: false,
            template: "AI assistance: [TOOL] was used for [TASK]. All AI-assisted content was reviewed, edited, and verified by the author(s), who take full responsibility for the work.".into(),
            notes: "ACL requires disclosure of AI use for writing assistance. Using AI to generate core technical content or to fabricate results is prohibited.".into(),
        },
        VenueTemplate {
            id: "generic".into(),
            label: "Generic / custom venue".into(),
            policy_url: "".into(),
            requires_in_manuscript: true,
            requires_in_cover_letter: false,
            template: "During the preparation of this work the author(s) used [TOOL] for [TASK]. The author(s) reviewed and edited the output and take(s) full responsibility for the content of the publication.".into(),
            notes: "Use this template when your target venue does not have a specific AI-use policy. Always check with the editor if unsure.".into(),
        },
    ]
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DisclosureInput {
    pub venue_id: String,
    pub tool_name: String,
    pub task_description: String,
    pub model_used: Option<String>,
    pub author_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DisclosureOutput {
    pub venue: VenueTemplate,
    pub statement: String,
    pub where_to_include: String,
    pub warnings: Vec<String>,
    /// Word Assist pre-fill (feature word-assist): a counts-only sentence
    /// the user may paste into the statement. Absent unless the caller
    /// supplies Word Assist ledger aggregates (additive field; older
    /// consumers see JSON without the key).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub word_assist_note: Option<String>,
}

/// Build the counts-only Word Assist sentence for a disclosure statement
/// (Phase 3 pre-fill). Pure: the numbers come from the assist ledger,
/// which stores hashes and counts, never text. No claim is made about
/// quality or detector behavior — the wording mirrors ETHICS.md: the tool
/// assisted the author; the author remains responsible for the text.
/// (Called from the feature-gated assist command layer only, so the
/// default build sees it as dead code; kept pub for the Disclosure
/// Assistant contract and unit tested below.)
#[cfg_attr(not(feature = "word-assist"), allow(dead_code))]
pub fn word_assist_note(
    accepted: u64,
    grammar: u64,
    spelling: u64,
    clarity: u64,
    rewrites: u64,
) -> String {
    if accepted == 0 {
        return "No writing-assistance suggestions were accepted while drafting this manuscript."
            .to_string();
    }
    let mut parts: Vec<String> = Vec::new();
    if grammar > 0 {
        parts.push(format!("{grammar} grammar"));
    }
    if spelling > 0 {
        parts.push(format!("{spelling} spelling"));
    }
    if clarity > 0 {
        parts.push(format!("{clarity} clarity"));
    }
    let breakdown = if parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", parts.join(", "))
    };
    let rewrites = if rewrites > 0 {
        format!(
            ", including {rewrites} whole-sentence rewrites generated locally and reviewed by the author"
        )
    } else {
        String::new()
    };
    format!(
        "While drafting, the author used ScholarScribe's local Word Assist feature, accepting {accepted} suggestion{plural}{breakdown}{rewrites}; all text was reviewed and approved by the author, who takes full responsibility for the final manuscript.",
        plural = if accepted == 1 { "" } else { "s" },
    )
}

pub fn generate(input: &DisclosureInput) -> Result<DisclosureOutput, String> {
    let venues = venue_templates();
    let venue = venues
        .iter()
        .find(|v| v.id == input.venue_id)
        .cloned()
        .ok_or_else(|| format!("Unknown venue: {}", input.venue_id))?;

    if input.tool_name.trim().is_empty() {
        return Err("Tool name is required.".into());
    }
    if input.task_description.trim().is_empty() {
        return Err("Task description is required.".into());
    }

    let mut statement = venue.template.clone();
    statement = statement.replace("[TOOL NAME AND VERSION]", &input.tool_name);
    statement = statement.replace("[TOOL]", &input.tool_name);
    let task = if let Some(m) = &input.model_used {
        if !m.is_empty() {
            format!("{} (model: {})", input.task_description, m)
        } else {
            input.task_description.clone()
        }
    } else {
        input.task_description.clone()
    };
    statement = statement.replace("[REASON FOR USE]", &task);
    statement = statement.replace("[TASK]", &task);

    if let Some(author) = &input.author_name {
        if !author.is_empty() {
            statement = format!("{}\n\n,  {}", statement, author);
        }
    }

    let mut where_to_include = Vec::new();
    if venue.requires_in_manuscript {
        where_to_include
            .push("In the manuscript (Acknowledgements or Methods section).".to_string());
    }
    if venue.requires_in_cover_letter {
        where_to_include.push("In the cover letter to the editor.".to_string());
    }
    if where_to_include.is_empty() {
        where_to_include
            .push("Where required by the venue's policy (check the link below).".to_string());
    }

    let mut warnings = Vec::new();
    if input.tool_name.to_lowercase().contains("chatgpt")
        && (venue.id == "nature" || venue.id == "acl")
    {
        warnings.push(format!(
            "{}'s policy specifically cautions against using ChatGPT or similar general LLMs for generating scientific content. Verify that your use is limited to language refinement only.",
            venue.label
        ));
    }
    warnings.push(format!(
        "Always verify the current policy at: {}",
        venue.policy_url
    ));

    Ok(DisclosureOutput {
        venue,
        statement,
        where_to_include: where_to_include.join("\n"),
        warnings,
        word_assist_note: None,
    })
}

#[cfg(test)]
mod word_assist_note_tests {
    use super::word_assist_note;

    #[test]
    fn zero_accepted_reads_neutral() {
        let note = word_assist_note(0, 0, 0, 0, 0);
        assert!(note.contains("No writing-assistance suggestions"));
    }

    #[test]
    fn counts_are_listed_and_responsibility_stated() {
        let note = word_assist_note(12, 7, 0, 5, 2);
        assert!(note.contains("12 suggestions"));
        assert!(note.contains("(7 grammar, 5 clarity)"));
        assert!(note.contains("2 whole-sentence rewrites"));
        assert!(note.contains("full responsibility"));
        // E-1: no precision figures, no detector talk.
        assert!(!note.contains('%'));
        assert!(!note.to_lowercase().contains("detector"));
    }

    #[test]
    fn singular_grammar() {
        assert!(word_assist_note(1, 1, 0, 0, 0).contains("1 suggestion"));
    }
}
