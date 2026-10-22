//! The Word Assist loopback listener (1b).
//!
//! Option A stack, exactly as proposed in recon §7 and approved by the
//! owner (ruling D-2's "reuse the in-tree hyper/rustls stack"): hyper 1.x
//! HTTP/1.1 per connection over tokio-rustls, ring provider, one self-
//! signed leaf from rcgen. The only new dependency is `rcgen` (D-2
//! approved, MIT OR Apache-2.0); hyper/hyper-util/http-body-util/
//! tokio-rustls/rustls were already in Cargo.lock as reqwest transitive
//! deps at these exact versions — declaring them adds no crate.
//!
//! SECURITY MODEL (recon §6 Q1/Q2, rulings 4c + 5):
//! - Binds `127.0.0.1:<port>` only. Never `0.0.0.0`.
//! - Host must be `localhost:<port>`; Origin must be
//!   `https://localhost:<port>` for every /v1 route except /v1/health.
//!   No CORS headers are ever emitted: the add-in page and the API share
//!   one origin, so any cross-origin browser request is rejected, not
//!   facilitated.
//! - Bearer token issued by /v1/pair against a 6-character one-time
//!   pairing code with a 5-minute TTL; constant-time compares throughout.
//! - 64 KiB body cap, sliding-window rate limit, no request logging of
//!   TEXT (the audit log records counts and hashes only — PROVENANCE_SPEC
//!   §0.3 pattern).
//! - TLS is mandatory; there is no plain-HTTP fallback in the shipping
//!   server (the http experiment lives in the Phase 0 spike only).

use std::sync::Arc;
use std::time::Duration;

use http_body_util::{BodyExt, Limited};
use hyper::body::Incoming;
use hyper::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE, HOST, ORIGIN};
use hyper::http::{Request, Response, StatusCode};
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use serde::{Deserialize, Serialize};
use tokio_rustls::TlsAcceptor;

use super::cache::{SentenceVerdict, Tier1Status};
use super::diff::word_diff;
use super::engine::{build_tier1_request, parse_tier1_response};
use super::scheduler::rand_compat::random_token;
use super::scheduler::Tier1Caller;
use super::settings::{AssistSettings, AssistState};
use super::tier0::check_paragraph;
use super::validate::validate_revision;

/// The response body type: `Full<Bytes>` is the hyper-1 equivalent of the
/// old `Response<Vec<u8>>` idiom.
type RespBody = http_body_util::Full<hyper::body::Bytes>;

fn body_response(body: hyper::body::Bytes) -> Response<RespBody> {
    Response::new(RespBody::from(body))
}

/// 64 KiB body cap (recon §8). Paragraphs are sentence-sized; even a
/// pathological paste is far below this.
const BODY_CAP: usize = 64 * 1024;
/// Tier 1 model-call timeout.
const TIER1_TIMEOUT: Duration = Duration::from_secs(120);

pub const ADDIN_CSP: &str = "default-src 'none'; script-src 'self' https://appsforoffice.microsoft.com; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; font-src 'self'";

fn json_response(status: StatusCode, value: &impl Serialize) -> Response<RespBody> {
    let body = serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec());
    let mut resp = body_response(hyper::body::Bytes::from(body));
    *resp.status_mut() = status;
    resp.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    resp.headers_mut()
        .insert("Cache-Control", HeaderValue::from_static("no-store"));
    resp
}

fn error_response(status: StatusCode, msg: &str) -> Response<RespBody> {
    json_response(status, &serde_json::json!({ "error": msg }))
}

fn static_response(content_type: &str, bytes: Vec<u8>, csp: Option<&str>) -> Response<RespBody> {
    let mut resp = body_response(hyper::body::Bytes::from(bytes));
    resp.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_str(content_type).expect("static content type"),
    );
    resp.headers_mut()
        .insert("Cache-Control", HeaderValue::from_static("no-store"));
    if let Some(csp) = csp {
        if let Ok(v) = HeaderValue::from_str(csp) {
            resp.headers_mut().insert("Content-Security-Policy", v);
        }
    }
    resp
}

// ------------------------------------------------------------- requests

#[derive(Deserialize)]
struct PairRequest {
    code: String,
}

#[derive(Deserialize)]
struct CheckRequest {
    paragraph_id: String,
    text: String,
    #[serde(default)]
    #[allow(dead_code)]
    // cursor priority lands with the Phase 4 UX pass; accepted and ignored for now
    has_focus: bool,
}

#[derive(Deserialize)]
struct AcceptedRequest {
    paragraph_id: String,
    issue_id: String,
    #[serde(default)]
    rule_id: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    applied_start: usize,
    #[serde(default)]
    applied_end: usize,
    #[serde(default)]
    replacement_len: usize,
    /// Hash of the pre-edit paragraph text (never the text itself).
    #[serde(default)]
    paragraph_text_hash: String,
}

// --------------------------------------------------------------- server

/// Handle over the running listener. Dropping it does NOT stop the task;
/// call [`stop`].
pub struct ServerHandle {
    pub port: u16,
    abort: tokio::task::AbortHandle,
}

impl ServerHandle {
    pub fn stop(&self) {
        self.abort.abort();
    }
}

/// Generate (or load) the per-install self-signed leaf, then serve.
/// `data_dir` is where the public certificate lives; the private key goes
/// to the OS keychain when available, else a 0600 file next to the cert
/// (both disclosed in the UI and SECURITY.md).
pub async fn start(
    settings: AssistSettings,
    state: Arc<AssistState>,
    data_dir: std::path::PathBuf,
) -> Result<ServerHandle, String> {
    let port = settings.port;
    let (cert_der, key_der) = ensure_certificate(&data_dir)?;
    // The ring provider is installed once process-wide; a second install
    // fails harmlessly (another assist start beat us to it).
    let _ = rustls::crypto::ring::default_provider().install_default();
    let server_config = Arc::new(
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der)
            .map_err(|e| format!("assist TLS setup: {e}"))?,
    );
    let acceptor = TlsAcceptor::from(server_config);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                format!(
                    "Port {port} is already in use. Another Word Assist listener (or a foreign process) is bound to it. Change the port in the Word Assist settings."
                )
            } else {
                format!("cannot bind 127.0.0.1:{port}: {e}")
            }
        })?;

    log::info!(
        "Word Assist listening on https://127.0.0.1:{port} (origin https://localhost:{port})"
    );
    *state.bound_port.lock().expect("bound port lock") = Some(port);
    state
        .running
        .store(true, std::sync::atomic::Ordering::SeqCst);
    state.audit.record(
        "assist_listener",
        "127.0.0.1",
        &format!("word assist listener started on port {port}"),
        0,
        0,
    );

    let task = tokio::spawn(async move {
        loop {
            let (stream, _peer) = match listener.accept().await {
                Ok(x) => x,
                Err(e) => {
                    log::warn!("assist accept error: {e}");
                    continue;
                }
            };
            let acceptor = acceptor.clone();
            let state = Arc::clone(&state);
            tokio::spawn(async move {
                let tls = match acceptor.accept(stream).await {
                    Ok(t) => t,
                    Err(_) => return, // handshake failures are routine noise
                };
                let svc = service_fn(move |req| handle(req, Arc::clone(&state)));
                let _ = hyper::server::conn::http1::Builder::new()
                    .keep_alive(true)
                    .serve_connection(TokioIo::new(tls), svc)
                    .await;
            });
        }
    });
    Ok(ServerHandle {
        port,
        abort: task.abort_handle(),
    })
}

// ------------------------------------------------------------- dispatch

async fn handle(
    req: Request<Incoming>,
    state: Arc<AssistState>,
) -> Result<Response<RespBody>, std::convert::Infallible> {
    // Extract a snapshot of the settings and drop the lock BEFORE any
    // await (MutexGuard is not Send; holding it across the dispatch would
    // make the handler future non-Send).
    let settings = {
        let guard = state.settings.lock().expect("settings lock");
        match guard.clone() {
            Some(s) => s,
            None => {
                return Ok(error_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Word Assist is not enabled.",
                ))
            }
        }
    };

    let method = req.method().clone();
    let path = req.uri().path().to_string();

    // Rate limit every request (health included; it is the cheap one but
    // still an oracle).
    if !state.rate.allow("loopback") {
        return Ok(error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "rate limited",
        ));
    }

    // Host pinning: exactly one accepted value.
    let expected_host = format!("localhost:{}", settings.port);
    match req.headers().get(HOST).and_then(|v| v.to_str().ok()) {
        Some(h) if h == expected_host => {}
        _ => return Ok(error_response(StatusCode::BAD_REQUEST, "unexpected Host")),
    }

    // /v1/* auth model (health is the only open route and leaks nothing
    // beyond the engine version).
    if path.starts_with("/v1/") && path != "/v1/health" {
        if let Err(msg) = check_origin(&req, &settings) {
            return Ok(error_response(StatusCode::FORBIDDEN, msg));
        }
        if let Err(msg) = check_bearer(&req, &state) {
            return Ok(error_response(StatusCode::UNAUTHORIZED, msg));
        }
    }

    let resp = match (&method, path.as_str()) {
        (&hyper::Method::GET, "/v1/health") => health(&state, &settings),
        (&hyper::Method::POST, "/v1/pair") => pair(req, state).await,
        (&hyper::Method::POST, "/v1/check") => check(req, state, settings).await,
        (&hyper::Method::POST, "/v1/accepted") => accepted(req, state).await,
        (&hyper::Method::GET, "/addin/manifest.xml") => {
            let xml = include_str!("../../../word-addin/dist/manifest.xml")
                .replace("{PORT}", &settings.port.to_string());
            static_response("application/xml; charset=utf-8", xml.into_bytes(), None)
        }
        (&hyper::Method::GET, "/addin/taskpane.html") => static_response(
            "text/html; charset=utf-8",
            include_str!("../../../word-addin/dist/taskpane.html")
                .as_bytes()
                .to_vec(),
            Some(ADDIN_CSP),
        ),
        (&hyper::Method::GET, "/addin/taskpane.js") => static_response(
            "text/javascript; charset=utf-8",
            include_str!("../../../word-addin/dist/taskpane.js")
                .as_bytes()
                .to_vec(),
            None,
        ),
        (&hyper::Method::GET, "/addin/icon-32.png") => static_response(
            "image/png",
            include_bytes!("../../../word-addin/dist/icon-32.png").to_vec(),
            None,
        ),
        (_, "/v1/health") | (_, "/v1/pair") | (_, "/v1/check") | (_, "/v1/accepted") => {
            error_response(StatusCode::METHOD_NOT_ALLOWED, "method not allowed")
        }
        _ => error_response(StatusCode::NOT_FOUND, "not found"),
    };
    Ok(resp)
}

fn check_origin(req: &Request<Incoming>, settings: &AssistSettings) -> Result<(), &'static str> {
    let expected = format!("https://localhost:{}", settings.port);
    match req.headers().get(ORIGIN).and_then(|v| v.to_str().ok()) {
        Some(o) if o == expected => Ok(()),
        Some(_) => Err("unexpected Origin"),
        None => Err("missing Origin header"),
    }
}

fn check_bearer(req: &Request<Incoming>, state: &AssistState) -> Result<(), &'static str> {
    match req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        Some(v) if v.starts_with("Bearer ") => {
            if state.token_matches(&v["Bearer ".len()..]) {
                Ok(())
            } else {
                Err("invalid bearer token; pair again from the Word Assist tab")
            }
        }
        _ => Err("missing bearer token"),
    }
}

// --------------------------------------------------------------- routes

fn health(state: &AssistState, settings: &AssistSettings) -> Response<RespBody> {
    json_response(
        StatusCode::OK,
        &serde_json::json!({
            "ok": true,
            "engine": super::ENGINE_VERSION,
            "paired": token_present(state),
            "strictness": settings.strictness.name(),
            "tier1_enabled": settings.tier1_enabled,
        }),
    )
}

fn token_present(state: &AssistState) -> bool {
    state.token.lock().map(|g| g.is_some()).unwrap_or(false)
}

async fn pair(req: Request<Incoming>, state: Arc<AssistState>) -> Response<RespBody> {
    let Some(body) = read_body(req).await else {
        return error_response(StatusCode::PAYLOAD_TOO_LARGE, "body too large");
    };
    let Ok(parsed) = serde_json::from_slice::<PairRequest>(&body) else {
        return error_response(StatusCode::BAD_REQUEST, "expected {\"code\": \"...\"}");
    };
    match state.take_pairing_code(&parsed.code) {
        Ok(true) => {
            let token = random_token().unwrap_or_default();
            if token.is_empty() {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "token generation failed",
                );
            }
            state
                .set_token(token.clone())
                .map_err(|e| log::warn!("token store: {e}"))
                .ok();
            state.ledger_record("paired", "", "-", "-", "-", 0, 0, 0, "");
            state.audit.record(
                "assist_pair",
                "localhost",
                "add-in paired; bearer token issued",
                body.len() as u64,
                0,
            );
            state.ledger_record("paired", "", "-", "-", "-", 0, 0, 0, "");
            json_response(StatusCode::OK, &serde_json::json!({ "token": token }))
        }
        Ok(false) => error_response(
            StatusCode::UNAUTHORIZED,
            "wrong or expired pairing code; get a fresh code from the Word Assist tab",
        ),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

async fn check(
    req: Request<Incoming>,
    state: Arc<AssistState>,
    settings: AssistSettings,
) -> Response<RespBody> {
    let Some(body) = read_body(req).await else {
        return error_response(StatusCode::PAYLOAD_TOO_LARGE, "body too large");
    };
    let Ok(parsed) = serde_json::from_slice::<CheckRequest>(&body) else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "expected {\"paragraph_id\", \"text\"}",
        );
    };
    if parsed.text.chars().count() > 20_000 {
        return error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "paragraph exceeds 20k characters",
        );
    }

    let experimental_settings = settings.clone();
    let experimental = move |script: &str| experimental_settings.experimental(script);
    let strictness = settings.strictness;

    // Tier 0 is CPU work: off the async runtime.
    let result = {
        let text = parsed.text.clone();
        let dialect = settings.dialect.clone();
        let rules = settings.rules.clone();
        let mode = settings.mask_mode;
        tokio::task::spawn_blocking(move || {
            check_paragraph(&text, &dialect, &rules, &strictness, mode, &experimental)
        })
        .await
        .map_err(|e| format!("tier0 task: {e}"))
    };
    let Ok(result) = result else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "tier 0 failed");
    };

    // Per-sentence assembly with cache + Tier 1 submission.
    let mut sentences_out = Vec::new();
    for sentence in crate::assist::sentences::split_sentences(&parsed.text) {
        let cached = state.cache.get(&sentence.text);
        let (issues, mut tier1) = match cached {
            Some(v) => (v.issues.clone(), v.tier1.clone()),
            None => {
                // Not cached: find this sentence's issues from the fresh
                // tier0 pass and store the verdict.
                let issues = result
                    .sentences
                    .iter()
                    .find(|s| s.start == sentence.start)
                    .map(|s| s.issues.clone())
                    .unwrap_or_default();
                let tier1 = if settings.tier1_enabled && !issues.is_empty() {
                    Tier1Status::Pending
                } else {
                    Tier1Status::Off
                };
                state.cache.put(
                    &sentence.text,
                    SentenceVerdict {
                        issues: issues.clone(),
                        tier1: tier1.clone(),
                    },
                );
                (issues, tier1)
            }
        };

        // Tier 1 submission: newest wins, single flight, budgeted.
        if settings.tier1_enabled
            && !issues.is_empty()
            && matches!(tier1, Tier1Status::Pending | Tier1Status::Failed { .. })
        {
            let model = if settings.tier1_model.is_empty() {
                "no model configured; pick one in the Word Assist settings".to_string()
            } else {
                settings.tier1_model.clone()
            };
            if model.contains(' ') {
                tier1 = Tier1Status::Failed { reason: model };
            } else {
                let sentence_text = sentence.text.clone();
                let state_for_result = Arc::clone(&state);
                let submitted = state.scheduler.submit(
                    super::cache::text_hash(&sentence_text),
                    sentence_text.clone(),
                    parsed.paragraph_id.clone(),
                    &parsed.paragraph_id,
                    Arc::new(OllamaCaller {
                        state: Arc::clone(&state),
                    }),
                    model,
                    move |raw| on_tier1_result(state_for_result, sentence_text, raw),
                );
                if !submitted {
                    tier1 = Tier1Status::Failed {
                        reason: "tier 1 call budget exhausted for this paragraph".into(),
                    };
                }
            }
        }

        sentences_out.push(serde_json::json!({
            "start": sentence.start,
            "end": sentence.end,
            "text": sentence.text,
            "issues": issues,
            "tier1": tier1,
        }));
    }

    state.audit.record(
        "assist_check",
        "localhost",
        &format!(
            "paragraph {} checked ({} chars, {} sentences with issues)",
            parsed.paragraph_id,
            parsed.text.chars().count(),
            result.sentences.len()
        ),
        body.len() as u64,
        0,
    );

    json_response(
        StatusCode::OK,
        &serde_json::json!({
            "paragraph_id": parsed.paragraph_id,
            "language_status": result.language_status,
            "sentences": sentences_out,
        }),
    )
}

/// Cache update after a Tier 1 call: parse (rule 1), validate (rules 2-6),
/// diff, store. Runs on the blocking pool thread; never panics into Word.
fn on_tier1_result(state: Arc<AssistState>, sentence_text: String, raw: Result<String, String>) {
    let tier1 = match raw {
        Err(reason) => {
            state.ledger_record("failed", "", "tier1", "tier1", "-", 0, 0, 0, "");
            Tier1Status::Failed { reason }
        }
        Ok(reply) => match parse_tier1_response(&reply) {
            Err(rejection) => {
                state.ledger_record("rejected", "", "tier1", "tier1", "-", 0, 0, 0, "");
                Tier1Status::Rejected {
                    rule: rejection.rule,
                }
            }
            Ok(resp) => match validate_revision(&sentence_text, &resp.revised) {
                Err(rejection) => {
                    state.ledger_record("rejected", "", "tier1", "tier1", "-", 0, 0, 0, "");
                    Tier1Status::Rejected {
                        rule: rejection.rule,
                    }
                }
                Ok(()) => {
                    // The sentence offset inside its paragraph is unknown
                    // here (the cache is per sentence); the add-in knows
                    // the sentence bounds it sent. Edits stay sentence-
                    // relative; start/end are filled by the checker below
                    // when assembling the response for the CURRENT bounds.
                    let edits = word_diff(&sentence_text, &resp.revised);
                    Tier1Status::Ready(Box::new(super::cache::Tier1Suggestion {
                        start: 0,
                        end: sentence_text.chars().count(),
                        original: sentence_text.clone(),
                        revised: resp.revised,
                        explanation: resp.explanation,
                        edits,
                    }))
                }
            },
        },
    };
    // Merge into the cached verdict (tier0 issues are untouched).
    let issues = state
        .cache
        .get(&sentence_text)
        .map(|v| v.issues.clone())
        .unwrap_or_default();
    state
        .cache
        .put(&sentence_text, SentenceVerdict { issues, tier1 });
}

async fn accepted(req: Request<Incoming>, state: Arc<AssistState>) -> Response<RespBody> {
    let Some(body) = read_body(req).await else {
        return error_response(StatusCode::PAYLOAD_TOO_LARGE, "body too large");
    };
    let Ok(parsed) = serde_json::from_slice::<AcceptedRequest>(&body) else {
        return error_response(StatusCode::BAD_REQUEST, "malformed accepted payload");
    };
    // Hashes and counts, never text (PROVENANCE_SPEC §0.3). The audit
    // trail feeds the Privacy Audit view; the HMAC-chained ledger (Phase
    // 3) is the user's own record of assistance for disclosures.
    state.ledger_record(
        "accepted",
        &parsed.paragraph_id,
        &parsed.issue_id,
        &parsed.rule_id,
        &parsed.category,
        parsed.applied_start,
        parsed.applied_end,
        parsed.replacement_len,
        &parsed.paragraph_text_hash,
    );
    state.audit.record(
        "assist_accepted",
        "localhost",
        &format!(
            "issue {} ({} / {}) applied in paragraph {} at {}..{} (+{} chars, pre-edit hash {})",
            parsed.issue_id,
            parsed.category,
            parsed.rule_id,
            parsed.paragraph_id,
            parsed.applied_start,
            parsed.applied_end,
            parsed.replacement_len,
            if parsed.paragraph_text_hash.is_empty() {
                "-"
            } else {
                &parsed.paragraph_text_hash[..parsed.paragraph_text_hash.len().min(12)]
            }
        ),
        body.len() as u64,
        0,
    );
    json_response(StatusCode::OK, &serde_json::json!({ "ok": true }))
}

/// Read the body with the hard cap. Returns None when over budget.
async fn read_body(req: Request<Incoming>) -> Option<hyper::body::Bytes> {
    let limited = Limited::new(req.into_body(), BODY_CAP);
    match limited.collect().await {
        Ok(collected) => Some(collected.to_bytes()),
        Err(_) => None,
    }
}

// ------------------------------------------------------------- tier 1 IO

/// The real Tier1Caller: one chat_at call to the local daemon with the
/// versioned prompt + structured output.
struct OllamaCaller {
    state: Arc<AssistState>,
}

impl Tier1Caller for OllamaCaller {
    fn call(&self, model: &str, sentence: &str) -> Result<String, String> {
        let req = build_tier1_request(model, sentence);
        let chat_req = crate::ollama::ChatRequest {
            model: req.model.clone(),
            messages: vec![
                crate::ollama::ChatMessage {
                    role: "system".into(),
                    content: req.system.clone(),
                },
                crate::ollama::ChatMessage {
                    role: "user".into(),
                    content: req.user.clone(),
                },
            ],
            stream: false,
            temperature: req.temperature,
            format: Some(req.format.clone()),
            think: Some(req.think),
        };
        let state = Arc::clone(&self.state);
        let handle = tokio::runtime::Handle::current();
        let outcome = std::thread::scope(|s| {
            s.spawn(move || {
                handle.block_on(async move {
                    tokio::time::timeout(
                        TIER1_TIMEOUT,
                        crate::ollama::chat_at(&state.ollama, crate::ollama::base_url(), chat_req),
                    )
                    .await
                })
            })
            .join()
        });
        match outcome {
            Ok(Ok(Ok(msg))) => Ok(msg.content),
            Ok(Ok(Err(e))) => Err(format!("Ollama: {e}")),
            Ok(Err(_elapsed)) => Err("Tier 1 timed out".into()),
            Err(_) => Err("Tier 1 task panicked".into()),
        }
    }
}

// ---------------------------------------------------------------- certs

/// Load the per-install certificate from `data_dir`, generating both the
/// leaf and the key on first run. Key storage: OS keychain first
/// (`scholarscribe` / `word-assist-tls-key-v1`), falling back to a 0600
/// file next to the certificate (disclosed in the UI; Q2 ruling).
fn ensure_certificate(
    data_dir: &std::path::Path,
) -> Result<(CertificateDer<'static>, PrivateKeyDer<'static>), String> {
    std::fs::create_dir_all(data_dir).map_err(|e| format!("create assist dir: {e}"))?;
    let cert_path = data_dir.join("word-assist-cert.der");
    let key_path = data_dir.join("word-assist-key.p8");

    if cert_path.exists() && key_path.exists() {
        let cert = std::fs::read(&cert_path).map_err(|e| format!("read assist cert: {e}"))?;
        let key = std::fs::read(&key_path).map_err(|e| format!("read assist key: {e}"))?;
        return Ok((
            CertificateDer::from(cert),
            PrivateKeyDer::try_from(key).map_err(|e| format!("assist key format: {e}"))?,
        ));
    }

    // First run (or lost files): generate a fresh leaf.
    let key_pair = rcgen::KeyPair::generate().map_err(|e| format!("key generation: {e}"))?;
    let mut params =
        rcgen::CertificateParams::new(vec!["localhost".to_string(), "127.0.0.1".to_string()])
            .map_err(|e| format!("cert params: {e}"))?;
    params.distinguished_name.push(
        rcgen::DnType::CommonName,
        "ScholarScribe Word Assist (localhost)",
    );
    params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| format!("self-sign: {e}"))?;

    let cert_der = cert.der().clone();
    let key_pkcs8 = key_pair.serialize_der();

    // Key: keychain preferred, 0600 file fallback (both disclosed).
    let stored_in_keychain = match keyring::Entry::new("scholarscribe", "word-assist-tls-key-v1") {
        Ok(entry) => entry.set_password(&hex_encode(&key_pkcs8)).is_ok(),
        Err(_) => false,
    };
    if !stored_in_keychain {
        write_private(&key_path, &key_pkcs8)?;
    }
    // The file copy is only written when the keychain is unavailable; the
    // certificate itself is public and always lands next to it.
    std::fs::write(&cert_path, cert_der.as_ref()).map_err(|e| format!("write assist cert: {e}"))?;

    log::info!(
        "Word Assist certificate generated (key in {}: {})",
        if stored_in_keychain {
            "OS keychain"
        } else {
            "app data (0600)"
        },
        cert_path.display()
    );
    Ok((cert_der, PrivateKeyDer::Pkcs8(key_pkcs8.into())))
}

fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut f = std::fs::File::create(path).map_err(|e| format!("create key file: {e}"))?;
        f.write_all(bytes)
            .map_err(|e| format!("write key file: {e}"))?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("restrict key file: {e}"))?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, bytes).map_err(|e| format!("write key file: {e}"))
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::{hex_encode, ADDIN_CSP, BODY_CAP};

    #[test]
    fn body_cap_is_64k() {
        assert_eq!(BODY_CAP, 64 * 1024);
    }

    #[test]
    fn csp_blocks_everything_but_self_and_cdn() {
        assert!(ADDIN_CSP.contains("default-src 'none'"));
        assert!(ADDIN_CSP.contains("appsforoffice.microsoft.com"));
        assert!(!ADDIN_CSP.contains("unsafe-eval"));
    }

    #[test]
    fn hex_round_trip() {
        assert_eq!(hex_encode(&[0x00, 0xff]), "00ff");
    }
}
