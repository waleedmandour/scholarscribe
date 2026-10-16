//! Manuscript references extraction (v2.2.1 r7).
//!
//! When a user uploads their full .docx manuscript for inspection, the
//! Citation Manager needs to:
//! 1. Extract the body text (the prose with in-text citations).
//! 2. Identify the "References" / "Bibliography" / "Works Cited" section
//!    heading at the end of the document and split the references section
//!    off from the body.
//! 3. Hand the body text to the citation validator and the references
//!    section to the plain-text BibTeX parser.
//!
//! This module does step 2 (and reuses `docx_reading::extract_text_from_docx`
//! for step 1). The split is heuristic: we walk paragraphs (separated by
//! `\n\n` per `extract_text_from_ooxml`'s output format) and look for the
//! first paragraph whose trimmed text matches one of the canonical
//! references-section headings (case-insensitive, with optional trailing
//! punctuation). Everything before that paragraph is the body; that
//! paragraph and everything after it is the references section.
//!
//! The split is conservative: if no references-section heading is found,
//! the entire text is treated as the body and the references section is
//! empty. The caller (the Tauri command) records a warning so the user
//! knows the manuscript didn't have a detectable references section.

use std::path::Path;

use crate::docx_reading;

/// The result of extracting the references section from a .docx manuscript.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ManuscriptExtraction {
    /// The source .docx file path.
    pub source_path: String,
    /// The full text of the manuscript body (everything before the
    /// references section). Paragraphs are separated by `\n\n`.
    pub body_text: String,
    /// The raw text of the references section (everything from the
    /// "References" / "Bibliography" / "Works Cited" heading onward).
    /// Paragraphs are separated by `\n\n`. This is what the plain-text
    /// BibTeX parser consumes.
    pub references_text: String,
    /// The actual heading text that was matched (e.g., "References",
    /// "Bibliography"). Empty when no heading was found.
    pub references_heading: String,
    /// True when the extractor found a references-section heading and
    /// successfully split body from references. False when no heading
    /// was detected (the body_text contains everything and references_text
    /// is empty).
    pub split_succeeded: bool,
}

/// The canonical references-section headings, lowercased. The splitter
/// matches any paragraph whose trimmed text (lowercased, trailing
/// punctuation stripped) is in this list. Single-word matches like
/// "references" are exact-only to avoid false positives on body prose
/// like "the references cited above".
const REFERENCES_HEADINGS: &[&str] = &[
    "references",
    "bibliography",
    "works cited",
    "works referenced",
    "literature cited",
    "reference list",
    "list of references",
];

/// Extract the body and references sections from a .docx manuscript.
///
/// The split is heuristic: walks paragraphs (separated by `\n\n`) and
/// looks for the FIRST paragraph whose trimmed text — after lowercasing
/// and stripping trailing punctuation — matches one of the canonical
/// references-section headings. Everything before is the body; that
/// paragraph and everything after is the references section.
///
/// Returns an extraction with `split_succeeded = false` when no
/// references heading is found. The caller should treat that as a
/// warning (the user's manuscript may not have a references section, or
/// the heading may be phrased unusually).
pub fn extract_manuscript_references(path: &Path) -> Result<ManuscriptExtraction, String> {
    let text = docx_reading::extract_text_from_docx(path)?;
    let source_path = path.to_string_lossy().into_owned();

    // Split into paragraphs. `extract_text_from_ooxml` separates
    // paragraphs with `\n\n`, so splitting on `\n\n` is the canonical way
    // to recover them.
    let paragraphs: Vec<&str> = text.split("\n\n").collect();

    let mut split_idx: Option<usize> = None;
    let mut matched_heading: String = String::new();
    for (i, p) in paragraphs.iter().enumerate() {
        let trimmed = p.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Strip trailing punctuation: "References.", "References:",
        // "References,". Lowercase for case-insensitive match.
        let normalized = trimmed
            .trim_end_matches(['.', ':', ';', ',', '!', '?'])
            .to_lowercase();
        if REFERENCES_HEADINGS.contains(&normalized.as_str()) {
            split_idx = Some(i);
            matched_heading = trimmed.to_string();
            break;
        }
    }

    let (body_text, references_text, split_succeeded) = match split_idx {
        Some(idx) => {
            // Body = everything before the references heading.
            let body = paragraphs[..idx].join("\n\n");
            // References = the heading paragraph + everything after.
            // We include the heading in the references_text for context
            // (the plain-text BibTeX parser will ignore it as a
            // non-reference line, but the user can see what was matched).
            let refs = paragraphs[idx..].join("\n\n");
            (body, refs, true)
        }
        None => (text.clone(), String::new(), false),
    };

    Ok(ManuscriptExtraction {
        source_path,
        body_text,
        references_text,
        references_heading: matched_heading,
        split_succeeded,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal in-memory .docx with the given document.xml body.
    /// Mirrors the helper in `docx_to_latex.rs::tests`.
    fn write_test_docx(dir: &Path, name: &str, document_xml: &str) -> std::path::PathBuf {
        use std::io::Write;
        let file_path = dir.join(name);
        let f = std::fs::File::create(&file_path).unwrap();
        let mut zip = zip::ZipWriter::new(f);
        let opts: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file("word/document.xml", opts).unwrap();
        zip.write_all(document_xml.as_bytes()).unwrap();
        zip.finish().unwrap();
        file_path
    }

    /// Build a document.xml with body paragraphs separated by `\n\n`
    /// (the format `extract_text_from_ooxml` produces).
    fn build_manuscript_xml(body_paragraphs: &[&str], references_heading: &str, reference_paragraphs: &[&str]) -> String {
        let mut s = String::new();
        s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
        s.push_str("<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\n");
        s.push_str("<w:body>\n");
        for p in body_paragraphs {
            s.push_str(&format!("<w:p><w:r><w:t>{}</w:t></w:r></w:p>\n", p));
        }
        // The references heading paragraph.
        s.push_str(&format!(
            "<w:p><w:pPr><w:pStyle w:val=\"Heading1\"/></w:pPr><w:r><w:t>{}</w:t></w:r></w:p>\n",
            references_heading
        ));
        for p in reference_paragraphs {
            s.push_str(&format!("<w:p><w:r><w:t>{}</w:t></w:r></w:p>\n", p));
        }
        s.push_str("</w:body>\n</w:document>\n");
        s
    }

    #[test]
    fn splits_body_and_references_at_references_heading() {
        let xml = build_manuscript_xml(
            &[
                "This is the introduction paragraph.",
                "Smith (2020) showed that the effect is significant (Smith, 2020).",
                "The conclusion follows from these results.",
            ],
            "References",
            &[
                "Smith, J. (2020). A great paper. Journal of Things, 12(3), 45-67.",
                "Jones, K. (2021). Another paper. Other Journal, 5, 1-10.",
            ],
        );
        let dir = std::env::temp_dir().join(format!(
            "scholarscribe-manuscript-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let src = write_test_docx(&dir, "manuscript.docx", &xml);
        let out = dir.join("ignored.tex"); // not used by extractor
        let _ = out;
        let extraction = extract_manuscript_references(&src).expect("extraction should succeed");
        assert!(extraction.split_succeeded, "split should succeed");
        assert_eq!(extraction.references_heading, "References");
        // Body should contain the intro paragraphs but NOT the references.
        assert!(extraction.body_text.contains("introduction paragraph"));
        assert!(extraction.body_text.contains("Smith (2020) showed"));
        assert!(!extraction.body_text.contains("Journal of Things"));
        // References section should contain the heading and the references.
        assert!(extraction.references_text.contains("References"));
        assert!(extraction.references_text.contains("Smith, J. (2020)"));
        assert!(extraction.references_text.contains("Jones, K. (2021)"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn matches_bibliography_and_works_cited_headings() {
        for heading in &["Bibliography", "Works Cited", "Literature Cited", "Reference List"] {
            let xml = build_manuscript_xml(
                &["Some body text."],
                heading,
                &["Author, A. (2020). Title. Journal, 1, 1-2."],
            );
            let dir = std::env::temp_dir().join(format!(
                "scholarscribe-manuscript-test-{}-{}",
                std::process::id(),
                heading.replace(' ', "_")
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let src = write_test_docx(&dir, "manuscript.docx", &xml);
            let extraction = extract_manuscript_references(&src).expect("extraction should succeed");
            assert!(
                extraction.split_succeeded,
                "split should succeed for heading: {}",
                heading
            );
            assert_eq!(extraction.references_heading, *heading);
            std::fs::remove_dir_all(&dir).ok();
        }
    }

    #[test]
    fn handles_trailing_punctuation_in_heading() {
        // "References." and "References:" should both match.
        for heading in &["References.", "References:", "References,"] {
            let xml = build_manuscript_xml(
                &["Some body text."],
                heading,
                &["Author, A. (2020). Title. Journal, 1, 1-2."],
            );
            let dir = std::env::temp_dir().join(format!(
                "scholarscribe-manuscript-test-punct-{}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let src = write_test_docx(&dir, "manuscript.docx", &xml);
            let extraction = extract_manuscript_references(&src).expect("extraction should succeed");
            assert!(
                extraction.split_succeeded,
                "split should succeed for heading with trailing punctuation: {}",
                heading
            );
            std::fs::remove_dir_all(&dir).ok();
        }
    }

    #[test]
    fn no_split_when_references_heading_absent() {
        let xml = build_manuscript_xml(
            &[
                "Body paragraph 1.",
                "Body paragraph 2 with citations (Smith, 2020).",
                "Body paragraph 3.",
            ],
            "Conclusion", // not a references heading
            &[],
        );
        let dir = std::env::temp_dir().join(format!(
            "scholarscribe-manuscript-test-no-refs-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let src = write_test_docx(&dir, "no-refs.docx", &xml);
        let extraction = extract_manuscript_references(&src).expect("extraction should succeed");
        assert!(
            !extraction.split_succeeded,
            "split should NOT succeed when there is no references heading"
        );
        assert!(extraction.references_heading.is_empty());
        assert!(extraction.references_text.is_empty());
        // Body should contain everything.
        assert!(extraction.body_text.contains("Body paragraph 1."));
        assert!(extraction.body_text.contains("Body paragraph 3."));
        std::fs::remove_dir_all(&dir).ok();
    }
}
