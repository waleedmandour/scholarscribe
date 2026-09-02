//! Google Docs Bridge (Phase 1.5) — pure logic.
//!
//! Pulls the *revision history* of a Google Doc through the Drive API and
//! feeds it through the exact same RawSession → SessionRecord pipeline the
//! .docx path uses, so a Google-sourced manifest is structurally identical
//! to a Word-sourced one.
//!
//! PRIVACY (this is the ONLY ScholarScribe feature that talks to the
//! network besides Ollama-on-localhost):
//! - Scope is `drive.readonly` — the app cannot modify anything.
//! - OAuth uses PKCE; the refresh token is stored in the OS keychain, never
//!   in a file, never in the webview.
//! - Revision text is downloaded into memory only for diffing; nothing
//!   textual is persisted or exported (the manifest carries hashes/counts).
//! - EVERY outbound call is recorded in the privacy audit log
//!   (google_docs_commands.rs). Remove that logging and the feature breaks
//!   its own ethics contract — don't.
//!
//! Mock-friendly: parsing/derivation lives here and is unit-tested against
//! recorded response shapes; the thin HTTP layer lives in
//! google_docs_net.rs.

use serde::{Deserialize, Serialize};

use crate::provenance::{RawRevision, RevisionKind};

/// OAuth scope — read-only access to Drive files (revisions included).
pub const GOOGLE_SCOPE: &str = "https://www.googleapis.com/auth/drive.readonly";

/// One revision of a Google Doc as Drive reports it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleRevision {
    pub id: String,
    /// Unix seconds, UTC.
    pub modified_time: i64,
    /// Full plain text of this revision (fetched separately).
    pub text: String,
    /// `lastModifyingUser.displayName` when Drive exposes one.
    pub author: Option<String>,
}

/// Metadata for one revision, straight from the revisions list response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionMeta {
    pub id: String,
    /// RFC 3339 as Drive returns it ("2026-01-02T09:00:00.000Z").
    pub modified_time: String,
    #[serde(default)]
    pub author: Option<String>,
}

/// OAuth token endpoint response (subset we consume).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub token_type: Option<String>,
}

/// Extract a Google Docs / Drive file ID from a URL or raw ID.
/// Handles: docs.google.com/document/d/{id}/..., drive.google.com/file/d/{id}/...,
/// drive.google.com/open?id={id}, and bare IDs.
pub fn parse_google_doc_id(input: &str) -> Option<String> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    // /d/{id} form (docs & drive)
    if let Some(pos) = input.find("/d/") {
        let rest = &input[pos + 3..];
        let id: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if !id.is_empty() {
            return Some(id);
        }
    }
    // ?id={id} form (drive open links)
    for param in input.split(['?', '&']) {
        if let Some(v) = param.strip_prefix("id=") {
            let id: String = v
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            if !id.is_empty() {
                return Some(id);
            }
        }
    }
    // Bare ID heuristic: Drive IDs are 25-60 chars of [A-Za-z0-9_-].
    if input.len() >= 20
        && input.len() <= 128
        && input
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Some(input.to_string());
    }
    None
}

/// Character-level diff of two revision snapshots (common-prefix/suffix).
/// Returns (deleted_from_prev, inserted_in_next). Good enough for session
/// statistics — this is not a general-purpose diff engine.
pub fn diff_consecutive(prev: &str, next: &str) -> (String, String) {
    let a: Vec<char> = prev.chars().collect();
    let b: Vec<char> = next.chars().collect();
    let min_len = a.len().min(b.len());
    let mut p = 0;
    while p < min_len && a[p] == b[p] {
        p += 1;
    }
    let mut s = 0;
    while s < min_len - p && a[a.len() - 1 - s] == b[b.len() - 1 - s] {
        s += 1;
    }
    let deleted: String = a[p..a.len() - s].iter().collect();
    let inserted: String = b[p..b.len() - s].iter().collect();
    (deleted, inserted)
}

/// Convert ordered revision snapshots into the same `RawRevision` stream the
/// .docx parser produces. The FIRST revision counts as an insertion of its
/// whole text (relative to an empty document); each later revision
/// contributes a deletion and/or insertion derived from the diff.
pub fn revisions_to_raw_revisions(revs: &[GoogleRevision]) -> Vec<RawRevision> {
    let mut sorted: Vec<&GoogleRevision> = revs.iter().collect();
    sorted.sort_by_key(|r| r.modified_time);

    let mut out: Vec<RawRevision> = Vec::new();
    for (i, rev) in sorted.iter().enumerate() {
        let author = rev
            .author
            .clone()
            .unwrap_or_else(|| "Unknown (Google Drive)".to_string());
        if i == 0 {
            if !rev.text.is_empty() {
                out.push(RawRevision {
                    author,
                    date: rev.modified_time,
                    kind: RevisionKind::Insertion,
                    text: rev.text.clone(),
                });
            }
            continue;
        }
        let prev_text = sorted[i - 1].text.clone();
        let (deleted, inserted) = diff_consecutive(&prev_text, &rev.text);
        // Replacements are recorded as a deletion followed by an insertion
        // at the same timestamp — order does not affect session aggregates.
        if !deleted.is_empty() {
            out.push(RawRevision {
                author: author.clone(),
                date: rev.modified_time,
                kind: RevisionKind::Deletion,
                text: deleted,
            });
        }
        if !inserted.is_empty() {
            out.push(RawRevision {
                author,
                date: rev.modified_time,
                kind: RevisionKind::Insertion,
                text: inserted,
            });
        }
    }
    out
}

/// Parse the Drive revisions-list JSON:
/// `{"revisions": [{"id": "1", "modifiedTime": "...", "lastModifyingUser": {"displayName": ...}}]}`
pub fn parse_revisions_list_json(json: &str) -> Result<Vec<RevisionMeta>, String> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("revisions response is not JSON: {e}"))?;
    let items = v
        .get("revisions")
        .and_then(|r| r.as_array())
        .ok_or_else(|| "revisions response missing 'revisions' array".to_string())?;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let id = item
            .get("id")
            .and_then(|x| x.as_str())
            .ok_or_else(|| "revision missing id".to_string())?
            .to_string();
        let modified_time = item
            .get("modifiedTime")
            .and_then(|x| x.as_str())
            .ok_or_else(|| format!("revision {id} missing modifiedTime"))?
            .to_string();
        let author = item
            .get("lastModifyingUser")
            .and_then(|u| u.get("displayName"))
            .and_then(|d| d.as_str())
            .map(|s| s.to_string());
        out.push(RevisionMeta {
            id,
            modified_time,
            author,
        });
    }
    Ok(out)
}

/// Parse the OAuth token endpoint response. Invalidates gracefully when
/// Google returns an error payload instead.
pub fn parse_token_response(json: &str) -> Result<TokenResponse, String> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("token response is not JSON: {e}"))?;
    if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
        let desc = v
            .get("error_description")
            .and_then(|d| d.as_str())
            .unwrap_or("");
        return Err(format!("Google OAuth error: {err} {desc}")
            .trim()
            .to_string());
    }
    let access_token = v
        .get("access_token")
        .and_then(|t| t.as_str())
        .ok_or_else(|| "token response missing access_token".to_string())?
        .to_string();
    Ok(TokenResponse {
        access_token,
        refresh_token: v
            .get("refresh_token")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string()),
        expires_in: v.get("expires_in").and_then(|t| t.as_u64()),
        token_type: v
            .get("token_type")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string()),
    })
}

/// Convert Drive RFC3339 ("2026-01-02T09:00:00.000Z") to unix seconds.
pub fn parse_drive_time(s: &str) -> Option<i64> {
    // Drive appends ".000" milliseconds that time's Rfc3339 accepts; trim
    // sub-second precision defensively for producers that deviate.
    use time::format_description::well_known::Rfc3339;
    use time::OffsetDateTime;
    if let Ok(dt) = OffsetDateTime::parse(s, &Rfc3339) {
        return Some(dt.unix_timestamp());
    }
    if let Some(dot) = s.find('.') {
        if let Ok(dt) = OffsetDateTime::parse(&format!("{}Z", &s[..dot]), &Rfc3339) {
            return Some(dt.unix_timestamp());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_doc_urls() {
        assert_eq!(
            parse_google_doc_id("https://docs.google.com/document/d/1AbC_dEf-123/edit#heading=h.x"),
            Some("1AbC_dEf-123".to_string())
        );
        assert_eq!(
            parse_google_doc_id("https://drive.google.com/file/d/BASE64_ID_9/view?usp=sharing"),
            Some("BASE64_ID_9".to_string())
        );
        assert_eq!(
            parse_google_doc_id("https://drive.google.com/open?id=FILEID1234567890abc"),
            Some("FILEID1234567890abc".to_string())
        );
        assert_eq!(
            parse_google_doc_id("1AbC_dEf-123XYZ45678901"),
            Some("1AbC_dEf-123XYZ45678901".to_string())
        );
        assert_eq!(parse_google_doc_id(""), None);
        assert_eq!(parse_google_doc_id("not a doc id!"), None);
    }

    #[test]
    fn diff_prefix_suffix() {
        let (del, ins) = diff_consecutive("hello world", "hello brave world");
        assert_eq!(del, "");
        assert_eq!(ins, "brave ");
        let (del, ins) = diff_consecutive("the quick fox", "the slow fox");
        assert_eq!(del, "quick");
        assert_eq!(ins, "slow");
        let (del, ins) = diff_consecutive("", "new text");
        assert_eq!(del, "");
        assert_eq!(ins, "new text");
        let (del, ins) = diff_consecutive("same", "same");
        assert_eq!(del, "");
        assert_eq!(ins, "");
    }

    #[test]
    fn revisions_to_raw_revisions_insertions() {
        let revs = vec![
            GoogleRevision {
                id: "1".into(),
                modified_time: 1000,
                text: "first draft".into(),
                author: Some("Ada".into()),
            },
            GoogleRevision {
                id: "2".into(),
                modified_time: 2000,
                text: "first draft, expanded".into(),
                author: Some("Ada".into()),
            },
        ];
        let raw = revisions_to_raw_revisions(&revs);
        assert_eq!(raw.len(), 2);
        assert_eq!(raw[0].kind, RevisionKind::Insertion);
        assert_eq!(raw[0].text, "first draft");
        assert_eq!(raw[1].kind, RevisionKind::Insertion);
        assert_eq!(raw[1].text, ", expanded");
        assert_eq!(raw[1].author, "Ada");
    }

    #[test]
    fn revisions_replacement_yields_delete_then_insert() {
        let revs = vec![
            GoogleRevision {
                id: "1".into(),
                modified_time: 1000,
                text: "the quick fox".into(),
                author: Some("Ada".into()),
            },
            GoogleRevision {
                id: "2".into(),
                modified_time: 2000,
                text: "the slow fox".into(),
                author: Some("Bob".into()),
            },
        ];
        let raw = revisions_to_raw_revisions(&revs);
        // Revision 1 counts as a whole-document insertion; revision 2 as a
        // replacement (delete "quick", insert "slow").
        assert_eq!(raw.len(), 3);
        assert_eq!(raw[0].kind, RevisionKind::Insertion);
        assert_eq!(raw[0].text, "the quick fox");
        assert_eq!(raw[0].author, "Ada");
        assert_eq!(raw[1].kind, RevisionKind::Deletion);
        assert_eq!(raw[1].text, "quick");
        assert_eq!(raw[1].author, "Bob"); // the editor of revision 2 made the change
        assert_eq!(raw[2].kind, RevisionKind::Insertion);
        assert_eq!(raw[2].text, "slow");
    }

    #[test]
    fn revisions_missing_author_gets_honest_label() {
        let revs = vec![GoogleRevision {
            id: "1".into(),
            modified_time: 1000,
            text: "x".into(),
            author: None,
        }];
        let raw = revisions_to_raw_revisions(&revs);
        assert_eq!(raw[0].author, "Unknown (Google Drive)");
    }

    #[test]
    fn parses_revisions_list_fixture() {
        let fixture = r#"{
            "revisions": [
                {"id": "101", "modifiedTime": "2026-01-02T09:00:00.000Z",
                 "lastModifyingUser": {"displayName": "Ada Lovelace", "kind": "drive#user"}},
                {"id": "102", "modifiedTime": "2026-01-02T09:30:00.000Z"}
            ]
        }"#;
        let metas = parse_revisions_list_json(fixture).unwrap();
        assert_eq!(metas.len(), 2);
        assert_eq!(metas[0].id, "101");
        assert_eq!(metas[0].author.as_deref(), Some("Ada Lovelace"));
        assert_eq!(metas[1].author, None);
        assert_eq!(parse_drive_time(&metas[0].modified_time), Some(1767344400));
    }

    #[test]
    fn parses_revisions_list_error_shapes() {
        assert!(parse_revisions_list_json("{}").is_err());
        assert!(parse_revisions_list_json("not json").is_err());
        assert!(parse_revisions_list_json(r#"{"revisions": [{"id": "x"}]}"#).is_err());
    }

    #[test]
    fn parses_token_response_fixture() {
        let fixture = r#"{
            "access_token": "ya29.a0AfB_byAbc",
            "refresh_token": "1//0gRefReshToken",
            "expires_in": 3599,
            "token_type": "Bearer",
            "scope": "https://www.googleapis.com/auth/drive.readonly"
        }"#;
        let t = parse_token_response(fixture).unwrap();
        assert_eq!(t.access_token, "ya29.a0AfB_byAbc");
        assert_eq!(t.refresh_token.as_deref(), Some("1//0gRefReshToken"));
        assert_eq!(t.expires_in, Some(3599));
    }

    #[test]
    fn token_response_error_shape() {
        let fixture =
            r#"{"error": "invalid_grant", "error_description": "Token has been expired."}"#;
        let err = parse_token_response(fixture).unwrap_err();
        assert!(err.contains("invalid_grant"));
        assert!(err.contains("Token has been expired."));
    }

    #[test]
    fn end_to_end_sessions_match_docx_pipeline() {
        // The Google path must feed group_into_sessions identically.
        let revs = vec![
            GoogleRevision {
                id: "1".into(),
                modified_time: 1000,
                text: "alpha".into(),
                author: Some("Ada".into()),
            },
            GoogleRevision {
                id: "2".into(),
                modified_time: 2000,
                text: "alpha beta".into(),
                author: Some("Ada".into()),
            },
            GoogleRevision {
                id: "3".into(),
                // 45 minutes AFTER revision 2 → new session.
                modified_time: 2000 + 45 * 60,
                text: "alpha beta gamma".into(),
                author: Some("Ada".into()),
            },
        ];
        let raw = revisions_to_raw_revisions(&revs);
        let grouping = crate::provenance::group_into_sessions(&raw);
        // 45-minute gap > 30 min → two sessions.
        assert_eq!(grouping.sessions.len(), 2);
        // Session 1: "alpha" (5) + " beta" (5).
        assert_eq!(grouping.sessions[0].chars_added, 10);
        let records = crate::provenance::build_chain(&grouping.sessions);
        let (ok, anomalies) = crate::provenance::verify_chain(&records);
        assert!(ok, "{anomalies:?}");
    }
}
