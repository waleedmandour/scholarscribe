//! Google Docs Bridge — thin HTTP layer (Phase 1.5).
//!
//! ALL functions here perform outbound HTTPS. Every call site MUST log to
//! the privacy audit log — that is enforced by convention in
//! google_docs_commands.rs, which is the only module allowed to call these.
//!
//! Endpoints (all Google-owned, documented):
//! - accounts.google.com  — authorization (browser, PKCE + loopback redirect)
//! - oauth2.googleapis.com/token — code/token exchange + refresh
//! - www.googleapis.com/drive/v3 — revision list + per-revision download

use crate::google_docs::{RevisionMeta, TokenResponse};

pub const OAUTH_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const OAUTH_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const DRIVE_API_BASE: &str = "https://www.googleapis.com/drive/v3";
/// Legacy per-revision plain-text export endpoint for native Google Docs
/// (Drive's alt=media does not serve text for native formats).
pub const DOCS_EXPORT_URL: &str = "https://docs.google.com/feeds/download/documents/export/Export";

pub fn consent_url(
    client_id: &str,
    redirect_uri: &str,
    code_challenge: &str,
    state: &str,
) -> String {
    format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&code_challenge={}&code_challenge_method=S256&state={}",
        OAUTH_AUTH_URL,
        urlencode(client_id),
        urlencode(redirect_uri),
        urlencode(crate::google_docs::GOOGLE_SCOPE),
        urlencode(code_challenge),
        urlencode(state),
    )
}

async fn post_form(url: &str, form: &[(&str, &str)]) -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(url)
        .form(form)
        .send()
        .await
        .map_err(|e| format!("network error talking to Google: {e}"))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("could not read Google response: {e}"))?;
    if !status.is_success() {
        return Err(format!("Google returned HTTP {status}: {body}"));
    }
    Ok(body)
}

/// Exchange an authorization code for tokens (PKCE verifier included).
pub async fn exchange_code(
    client_id: &str,
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
) -> Result<TokenResponse, String> {
    let body = post_form(
        OAUTH_TOKEN_URL,
        &[
            ("client_id", client_id),
            ("code", code),
            ("code_verifier", code_verifier),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
        ],
    )
    .await?;
    crate::google_docs::parse_token_response(&body)
}

/// Refresh an access token from the stored refresh token.
pub async fn refresh_access_token(
    client_id: &str,
    refresh_token: &str,
) -> Result<TokenResponse, String> {
    let body = post_form(
        OAUTH_TOKEN_URL,
        &[
            ("client_id", client_id),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ],
    )
    .await?;
    crate::google_docs::parse_token_response(&body)
}

/// List a document's revisions (metadata only).
pub async fn list_revisions(
    access_token: &str,
    file_id: &str,
) -> Result<Vec<RevisionMeta>, String> {
    let url = format!(
        "{}/files/{}/revisions?fields=revisions(id,modifiedTime,lastModifyingUser)&pageSize=1000",
        DRIVE_API_BASE, file_id
    );
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| format!("network error listing revisions: {e}"))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("could not read revisions response: {e}"))?;
    if !status.is_success() {
        return Err(format!("Google returned HTTP {status}: {body}"));
    }
    crate::google_docs::parse_revisions_list_json(&body)
}

/// Download the plain text of one revision. Tries Drive's alt=media first,
/// then the legacy docs export endpoint (native Google Docs formats).
pub async fn fetch_revision_text(
    access_token: &str,
    file_id: &str,
    revision_id: &str,
) -> Result<String, String> {
    let client = reqwest::Client::new();

    let media_url = format!(
        "{}/files/{}/revisions/{}?alt=media",
        DRIVE_API_BASE, file_id, revision_id
    );
    let resp = client
        .get(&media_url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| format!("network error fetching revision: {e}"))?;
    if resp.status().is_success() {
        let ctype = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = resp
            .text()
            .await
            .map_err(|e| format!("could not read revision body: {e}"))?;
        // JSON metadata means the native-format doc needs the export path.
        if !ctype.contains("json") {
            return Ok(body);
        }
    }

    let export_url = format!(
        "{}?id={}&revision={}&exportFormat=txt",
        DOCS_EXPORT_URL, file_id, revision_id
    );
    let resp = client
        .get(&export_url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| format!("network error exporting revision: {e}"))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("could not read export body: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "could not fetch text for revision {revision_id} (HTTP {status})"
        ));
    }
    Ok(body)
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencode_matches_rfc3986_unreserved() {
        assert_eq!(urlencode("abc DEF-1_2.3~x"), "abc%20DEF-1_2.3~x");
        assert_eq!(urlencode("a&b=c/d"), "a%26b%3Dc%2Fd");
    }

    #[test]
    fn consent_url_contains_pkce_and_scope() {
        let url = consent_url(
            "my-client",
            "http://127.0.0.1:9999",
            "challenge123",
            "state456",
        );
        assert!(url.starts_with(OAUTH_AUTH_URL));
        assert!(url.contains("client_id=my-client"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A9999"));
        assert!(url.contains("code_challenge=challenge123"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("access_type=offline"));
        assert!(url.contains("drive.readonly"));
        assert!(url.contains("state=state456"));
    }
}
