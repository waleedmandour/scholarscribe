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
//!
//! v2.1.0 — Writing Provenance: also extracts Word's real Track Changes
//! history (`w:ins` / `w:del` elements) as `RawRevision`s for the hash
//! chain. See `provenance.rs` and docs/PROVENANCE_SPEC.md. ETHICAL NOTE:
//! this reads the revision history the document *actually carries* — we
//! never fabricate or synthesize history (docs/ETHICS.md §2.3).

use std::io::Read;
use std::path::Path;

use quick_xml::events::Event;

use crate::provenance::{ProvenanceError, RawRevision, RevisionKind};

/// Extract plain text from a .docx file. Returns the text with paragraphs
/// separated by `\n\n`.
pub fn extract_text_from_docx(path: &Path) -> Result<String, String> {
    let (_, document_xml) = read_document_xml(path).map_err(|e| e.to_string())?;
    Ok(extract_text_from_ooxml(&document_xml))
}

/// Read `word/document.xml` out of a .docx. Returns (raw bytes, lossy text).
fn read_document_xml(path: &Path) -> Result<(Vec<u8>, String), ProvenanceError> {
    let bytes = std::fs::read(path)
        .map_err(|e| ProvenanceError::Io(format!("read {}: {}", path.display(), e)))?;

    // .docx is a ZIP. Find word/document.xml inside it.
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| ProvenanceError::CorruptOoxml(format!("not a readable .docx ({e})")))?;

    let mut raw: Option<Vec<u8>> = None;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| ProvenanceError::CorruptOoxml(format!("zip entry {}: {}", i, e)))?;
        if entry.name() == "word/document.xml" {
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .map_err(|e| ProvenanceError::CorruptOoxml(format!("read document.xml: {}", e)))?;
            raw = Some(buf);
            break;
        }
    }
    let raw = raw.ok_or_else(|| {
        ProvenanceError::CorruptOoxml(
            "word/document.xml not found in .docx — file may be corrupt or not a real .docx"
                .to_string(),
        )
    })?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    Ok((raw, text))
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

// ---------------------------------------------------------------------------
// Writing Provenance — Track Changes extraction (v2.1.0)
// ---------------------------------------------------------------------------

/// Result of extracting Track Changes from a document.
#[derive(Debug)]
pub struct TrackChangesExtraction {
    /// Revisions with usable (non-empty) text, in markup order.
    pub revisions: Vec<RawRevision>,
    /// Revisions skipped because their `w:date` could not be parsed.
    pub skipped_unparsable_dates: usize,
    /// Whether ANY `w:ins`/`w:del` markup was present (even empty).
    pub has_track_changes_markup: bool,
    /// SHA-256 of the raw `word/document.xml` bytes — the manifest's
    /// `document_hash`. Lets a verifier bind the manifest to the exact
    /// document file it was produced from.
    pub document_xml_sha256: String,
}

/// Extract Track Changes revisions from a .docx file on disk.
pub fn extract_track_changes(path: &Path) -> Result<TrackChangesExtraction, ProvenanceError> {
    let (raw, document_xml) = read_document_xml(path)?;
    let document_xml_sha256 = format!("sha256:{}", crate::provenance::sha256_hex(&raw));
    parse_track_changes_ooxml(&document_xml, document_xml_sha256)
}

/// Spec-required pipeline entry: .docx path → grouped sessions.
/// Errors when the document carries no usable revision history.
/// (Part of the documented public API; the command layer uses
/// `extract_track_changes` directly because it needs extraction details
/// such as skipped-revision counts for anomaly reporting.)
#[allow(dead_code)]
pub fn extract_track_changes_sessions(
    path: &Path,
) -> Result<Vec<crate::provenance::RawSession>, ProvenanceError> {
    let extraction = extract_track_changes(path)?;
    if extraction.revisions.is_empty() {
        return Err(if extraction.has_track_changes_markup {
            ProvenanceError::NoRevisions
        } else {
            ProvenanceError::NoTrackChanges
        });
    }
    Ok(crate::provenance::group_into_sessions(&extraction.revisions).sessions)
}

struct OpenRevision {
    kind: RevisionKind,
    author: String,
    date: i64,
    text: String,
}

/// Parse `w:ins` / `w:del` elements out of an OOXML document.xml string.
/// Handled per the ECMA-376 layout: `<w:ins w:author w:date>` contains runs
/// whose `<w:t>` holds inserted text; `<w:del ...>` contains runs whose
/// `<w:delText>` holds deleted text. Formatting-only changes (`w:rPrChange`,
/// `w:pPrChange`) are not content revisions and are ignored.
fn parse_track_changes_ooxml(
    xml: &str,
    document_xml_sha256: String,
) -> Result<TrackChangesExtraction, ProvenanceError> {
    let mut revisions: Vec<RawRevision> = Vec::new();
    let mut skipped_dates = 0usize;
    let mut has_markup = false;
    let mut stack: Vec<OpenRevision> = Vec::new();
    // Whether text events should be appended to the innermost open revision.
    let mut collecting = false;

    let mut reader = quick_xml::Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"w:ins" | b"w:del" => {
                    has_markup = true;
                    let kind = if e.name().as_ref() == b"w:ins" {
                        RevisionKind::Insertion
                    } else {
                        RevisionKind::Deletion
                    };
                    let author = attr_value(&e, b"w:author").unwrap_or_default();
                    let date = attr_value(&e, b"w:date").and_then(|d| parse_ooxml_date(&d));
                    match (author.is_empty(), date) {
                        (false, Some(ts)) => stack.push(OpenRevision {
                            kind,
                            author,
                            date: ts,
                            text: String::new(),
                        }),
                        (false, None) => {
                            // Marked as skipped; still count the markup.
                            skipped_dates += 1;
                        }
                        _ => { /* no author recorded — ignore this element */ }
                    }
                }
                b"w:t" | b"w:delText" => collecting = true,
                _ => {}
            },
            Ok(Event::Empty(e)) => {
                if !stack.is_empty() && collecting {
                    match e.name().as_ref() {
                        b"w:tab" => stack.last_mut().unwrap().text.push('\t'),
                        b"w:br" => stack.last_mut().unwrap().text.push('\n'),
                        _ => {}
                    }
                }
            }
            Ok(Event::Text(t)) => {
                if collecting && !stack.is_empty() {
                    if let Ok(txt) = t.unescape() {
                        stack.last_mut().unwrap().text.push_str(&txt);
                    }
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"w:t" | b"w:delText" => collecting = false,
                b"w:ins" | b"w:del" => {
                    if let Some(open) = stack.pop() {
                        if !open.text.is_empty() {
                            revisions.push(RawRevision {
                                author: open.author,
                                date: open.date,
                                kind: open.kind,
                                text: open.text,
                            });
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(ProvenanceError::CorruptOoxml(format!(
                    "XML parse failure: {err}"
                )))
            }
            _ => {}
        }
    }

    if !stack.is_empty() {
        // Unclosed tracked-change element — malformed document.
        return Err(ProvenanceError::CorruptOoxml(
            "unclosed tracked-change element (w:ins/w:del) — document.xml is malformed".into(),
        ));
    }

    Ok(TrackChangesExtraction {
        revisions,
        skipped_unparsable_dates: skipped_dates,
        has_track_changes_markup: has_markup,
        document_xml_sha256,
    })
}

/// Read an attribute value off a start tag, XML-unescaped.
fn attr_value(e: &quick_xml::events::BytesStart<'_>, name: &[u8]) -> Option<String> {
    for attr in e.attributes() {
        let attr = attr.ok()?;
        if attr.key.as_ref() == name {
            let raw = String::from_utf8_lossy(&attr.value).into_owned();
            return Some(unescape_xml_entities(&raw));
        }
    }
    None
}

/// Minimal XML entity unescape (order matters: `&amp;` last).
fn unescape_xml_entities(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// Parse an OOXML `xsd:dateTime` ("2026-01-31T10:20:00Z", offsets also
/// accepted). Returns unix seconds. Falls back to treating a zoneless
/// timestamp as UTC.
fn parse_ooxml_date(s: &str) -> Option<i64> {
    use time::format_description::well_known::Rfc3339;
    use time::OffsetDateTime;
    if let Ok(dt) = OffsetDateTime::parse(s, &Rfc3339) {
        return Some(dt.unix_timestamp());
    }
    if let Ok(dt) = OffsetDateTime::parse(&format!("{s}Z"), &Rfc3339) {
        return Some(dt.unix_timestamp());
    }
    None
}

// ---------------------------------------------------------------------------
// Tests — build real .docx files in memory and parse them back
// ---------------------------------------------------------------------------

#[cfg(test)]
mod provenance_tests {
    use super::*;
    use crate::provenance::RevisionKind;

    const XMLNS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;

    fn make_docx(document_xml: &str) -> Vec<u8> {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
        zip.start_file("word/document.xml", options).unwrap();
        use std::io::Write as _;
        zip.write_all(document_xml.as_bytes()).unwrap();
        zip.finish().unwrap().into_inner()
    }

    fn write_temp(document_xml: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "scholarscribe-test-{}-{}.docx",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&path, make_docx(document_xml)).unwrap();
        path
    }

    fn tracked_xml(body: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document {XMLNS}><w:body>{body}</w:body></w:document>"#
        )
    }

    #[test]
    fn extracts_insertion_and_deletion() {
        let xml = tracked_xml(
            r#"<w:p><w:r><w:t>Keep </w:t></w:r>
               <w:ins w:id="1" w:author="Dr. Ada" w:date="2026-01-02T09:00:00Z">
                 <w:r><w:t>inserted text</w:t></w:r>
               </w:ins>
               <w:del w:id="2" w:author="Dr. Ada" w:date="2026-01-02T09:05:00Z">
                 <w:r><w:delText>removed text</w:delText></w:r>
               </w:del></w:p>"#,
        );
        let path = write_temp(&xml);
        let ex = extract_track_changes(&path).unwrap();
        std::fs::remove_file(&path).ok();

        assert!(ex.has_track_changes_markup);
        assert_eq!(ex.revisions.len(), 2);
        assert_eq!(ex.revisions[0].author, "Dr. Ada");
        assert_eq!(ex.revisions[0].kind, RevisionKind::Insertion);
        assert_eq!(ex.revisions[0].text, "inserted text");
        assert_eq!(ex.revisions[1].kind, RevisionKind::Deletion);
        assert_eq!(ex.revisions[1].text, "removed text");
        // 2026-01-02T09:00:00Z
        assert_eq!(ex.revisions[0].date, 1767344400);
        assert!(ex.document_xml_sha256.starts_with("sha256:"));
    }

    #[test]
    fn sessions_through_file_path() {
        let xml = tracked_xml(
            r#"<w:ins w:id="1" w:author="A" w:date="2026-01-02T09:00:00Z"><w:r><w:t>one</w:t></w:r></w:ins>
               <w:ins w:id="2" w:author="A" w:date="2026-01-02T09:10:00Z"><w:r><w:t>two</w:t></w:r></w:ins>"#,
        );
        let path = write_temp(&xml);
        let sessions = extract_track_changes_sessions(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(sessions.len(), 1, "10-minute gap joins one session");
        assert_eq!(sessions[0].chars_added, 6);
    }

    #[test]
    fn no_track_changes_errors() {
        let xml = tracked_xml(r#"<w:p><w:r><w:t>plain text, no revisions</w:t></w:r></w:p>"#);
        let path = write_temp(&xml);
        let err = extract_track_changes_sessions(&path).unwrap_err();
        std::fs::remove_file(&path).ok();
        assert!(matches!(err, ProvenanceError::NoTrackChanges));
        assert!(err.to_string().contains("No tracked changes found"));
    }

    #[test]
    fn corrupt_ooxml_errors() {
        // Mismatched end tag — a real XML syntax error quick-xml reports.
        let path = write_temp("<w:document><w:p></w:document>");
        let err = extract_track_changes_sessions(&path).unwrap_err();
        std::fs::remove_file(&path).ok();
        assert!(matches!(err, ProvenanceError::CorruptOoxml(_)));
    }

    #[test]
    fn unclosed_tracked_change_errors() {
        // A w:ins that never closes is malformed even though quick-xml
        // itself is a lenient tokenizer.
        let path = write_temp(&tracked_xml(
            r#"<w:ins w:id="1" w:author="A" w:date="2026-01-02T09:00:00Z"><w:r><w:t>x"#,
        ));
        let err = extract_track_changes_sessions(&path).unwrap_err();
        std::fs::remove_file(&path).ok();
        assert!(matches!(err, ProvenanceError::CorruptOoxml(_)));
    }

    #[test]
    fn not_a_docx_errors() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("not-a-docx-{}.bin", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"this is not a zip file").unwrap();
        let err = extract_track_changes_sessions(&path).unwrap_err();
        std::fs::remove_file(&path).ok();
        assert!(matches!(err, ProvenanceError::CorruptOoxml(_)));
    }

    #[test]
    fn entities_and_special_chars() {
        let xml = tracked_xml(
            r#"<w:ins w:id="1" w:author="Ann &amp; Bob" w:date="2026-01-02T09:00:00Z">
                 <w:r><w:t>5 &lt; 6 &amp; 7 &gt; 3</w:t></w:r>
               </w:ins>"#,
        );
        let path = write_temp(&xml);
        let ex = extract_track_changes(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(ex.revisions[0].author, "Ann & Bob");
        assert_eq!(ex.revisions[0].text, "5 < 6 & 7 > 3");
    }

    #[test]
    fn document_hash_is_stable() {
        let xml = tracked_xml(
            r#"<w:ins w:id="1" w:author="A" w:date="2026-01-02T09:00:00Z"><w:r><w:t>x</w:t></w:r></w:ins>"#,
        );
        let path = write_temp(&xml);
        let h1 = extract_track_changes(&path).unwrap().document_xml_sha256;
        let h2 = extract_track_changes(&path).unwrap().document_xml_sha256;
        std::fs::remove_file(&path).ok();
        assert_eq!(h1, h2);
        assert_ne!(h1, crate::provenance::GENESIS_HASH);
    }

    #[test]
    fn plain_text_extraction_still_works() {
        // Regression guard: the provenance refactor must not break the
        // original use case (extract_text_from_docx).
        let xml = tracked_xml(
            r#"<w:p><w:r><w:t>Hello </w:t></w:r>
               <w:ins w:id="1" w:author="A" w:date="2026-01-02T09:00:00Z"><w:r><w:t>world</w:t></w:r></w:ins>
               <w:del w:id="2" w:author="A" w:date="2026-01-02T09:00:00Z"><w:r><w:delText>gone</w:delText></w:r></w:del></w:p>"#,
        );
        let path = write_temp(&xml);
        let text = extract_text_from_docx(&path).unwrap();
        std::fs::remove_file(&path).ok();
        // Existing parser reads w:t inside w:ins (inserted text is real text)
        // and skips w:delText (it only matches "w:t").
        assert!(text.contains("Hello"));
        assert!(text.contains("world"));
    }
}
