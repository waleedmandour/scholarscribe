//! .docx file reading — extracts plain text from Word documents.
//!
//! A .docx file is a ZIP archive containing `word/document.xml` (an OOXML
//! document). We unzip in memory and walk the XML to pull text out of `<w:t>`
//! elements, insert paragraph breaks on `</w:p>`, and tabs on `<w:tab/>`.
//!
//! This is more robust than depending on a third-party docx parser crate
//! because the OOXML spec is stable and our needs are simple (plain text
//! extraction — not formatting, styles, or revision tracking).
//!
//! Uses the `zip` crate for unzip (pure Rust, no system deps).

use std::io::Read;
use std::path::Path;

/// Extract plain text from a .docx file. Returns the text with paragraphs
/// separated by `\n\n`.
pub fn extract_text_from_docx(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {}", path.display(), e))?;

    // .docx is a ZIP. Find word/document.xml inside it.
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("unzip .docx: {}", e))?;

    let mut document_xml = String::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("zip entry {}: {}", i, e))?;
        if entry.name() == "word/document.xml" {
            entry
                .read_to_string(&mut document_xml)
                .map_err(|e| format!("read document.xml: {}", e))?;
            break;
        }
    }
    if document_xml.is_empty() {
        return Err(
            "word/document.xml not found in .docx — file may be corrupt or not a real .docx".into(),
        );
    }

    Ok(extract_text_from_ooxml(&document_xml))
}

/// Pull text out of an OOXML document.xml string. Walks the XML and
/// accumulates text from `<w:t>` elements. Inserts `\n\n` on `</w:p>`
/// (paragraph end), `\t` on `<w:tab/>`, `\n` on `<w:br/>`.
///
/// We use a hand-rolled state machine rather than a full XML parser
/// because OOXML is well-formed by construction and we only need to
/// identify a handful of tag types. This avoids pulling in a heavyweight
/// XML dependency.
fn extract_text_from_ooxml(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len() / 4);
    let mut in_text = false;
    let mut current_text = String::new();
    let mut chars = xml.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '<' {
            // Read the tag name
            let mut tag = String::new();
            let mut closing = false;
            if chars.peek() == Some(&'/') {
                chars.next();
                closing = true;
            }
            while let Some(&ch) = chars.peek() {
                if ch == '>' || ch == ' ' || ch == '/' {
                    break;
                }
                tag.push(ch);
                chars.next();
            }
            // Skip attributes and consume until '>'
            while let Some(&ch) = chars.peek() {
                chars.next();
                if ch == '>' {
                    break;
                }
            }

            if tag == "w:t" {
                if !closing {
                    in_text = true;
                    current_text.clear();
                } else {
                    if in_text {
                        out.push_str(&current_text);
                        current_text.clear();
                    }
                    in_text = false;
                }
            } else if tag == "w:p" && closing {
                // End of paragraph — add blank line
                if !out.is_empty() {
                    out.push_str("\n\n");
                }
            } else if tag == "w:tab" {
                if !in_text {
                    out.push('\t');
                }
            } else if tag == "w:br" {
                if !in_text {
                    out.push('\n');
                }
            }
        } else if in_text {
            current_text.push(c);
        }
    }

    // Normalize trailing whitespace
    while out.ends_with('\n') {
        out.pop();
    }
    out
}
