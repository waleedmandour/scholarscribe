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
            statement = format!("{}\n\n— {}", statement, author);
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
    })
}
