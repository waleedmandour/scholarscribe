//! .docx file reading — extracts plain text from Word documents.
//!
//! Uses the `docx-rs` crate (pure Rust, no system deps, no LibreOffice).
//! Extracts paragraphs and tables in reading order. Preserves paragraph
//! breaks (one blank line between paragraphs) so downstream text-cleaning
//! and style-analysis modules can operate on a sensible text representation.

use std::path::Path;

/// Extract plain text from a .docx file. Returns the text with paragraphs
/// separated by `\n\n`. Tables are flattened in reading order with cells
/// joined by ` | ` and rows separated by newlines.
pub fn extract_text_from_docx(path: &Path) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("open {}: {}", path.display(), e))?;
    let reader = std::io::BufReader::new(file);
    let doc = docx_rs::read_docx(reader).map_err(|e| format!("parse .docx: {}", e.to_string()))?;

    let mut out = String::with_capacity(8192);

    // Walk every direct child of the document body. docx-rs exposes
    // paragraphs and tables as `Paragraph` and `Table` variants of
    // `DocumentBodyContent`. We iterate the body's children vec.
    for item in doc.document.body.content.iter() {
        match item {
            docx_rs::DocumentBodyContent::Paragraph(p) => {
                append_paragraph(&mut out, p);
            }
            docx_rs::DocumentBodyContent::Table(t) => {
                append_table(&mut out, t);
            }
        }
    }

    // Normalize trailing whitespace and double-newlines.
    while out.ends_with('\n') {
        out.pop();
    }
    Ok(out)
}

fn append_paragraph(out: &mut String, p: &docx_rs::Paragraph) {
    let mut para_text = String::new();
    for child in &p.children {
        match child {
            docx_rs::ParagraphChild::Text(t) => {
                para_text.push_str(&t.text);
            }
            docx_rs::ParagraphChild::Hyperlink(h) => {
                for hc in &h.children {
                    if let docx_rs::HyperlinkChild::Text(t) = hc {
                        para_text.push_str(&t.text);
                    }
                }
            }
            docx_rs::ParagraphChild::Run(r) => {
                // Some text is wrapped in a Run with text children.
                for rc in &r.children {
                    if let docx_rs::RunChild::Text(t) = rc {
                        para_text.push_str(&t.text);
                    }
                }
            }
            _ => {} // skip images, breaks, etc.
        }
    }
    if !para_text.is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&para_text);
    } else if !out.is_empty() {
        // Empty paragraph — represent as a blank line if we already have content.
        out.push('\n');
    }
}

fn append_table(out: &mut String, t: &docx_rs::Table) {
    for row in &t.rows {
        let mut cells: Vec<String> = Vec::with_capacity(row.cells.len());
        for cell in &row.cells {
            let mut cell_text = String::new();
            for p in &cell.content {
                if let docx_rs::DocumentBodyContent::Paragraph(para) = p {
                    let mut para_text = String::new();
                    for child in &para.children {
                        match child {
                            docx_rs::ParagraphChild::Text(t) => para_text.push_str(&t.text),
                            docx_rs::ParagraphChild::Run(r) => {
                                for rc in &r.children {
                                    if let docx_rs::RunChild::Text(t) = rc {
                                        para_text.push_str(&t.text);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    if !cell_text.is_empty() {
                        cell_text.push(' ');
                    }
                    cell_text.push_str(&para_text);
                }
            }
            cells.push(cell_text);
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&cells.join(" | "));
    }
}
