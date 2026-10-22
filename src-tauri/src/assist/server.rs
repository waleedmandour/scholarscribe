//! The Word Assist loopback listener (1b).
//!
//! Option A stack, exactly as proposed in recon §7 and approved by the
//! owner (ruling D-2's "reuse the in-tree hyper/rustls stack"): hyper 1.x
//! HTTP/1.1 per connection over tokio-rustls, explicit ring provider, an
//! EPHEMERAL CA + leaf chain from rcgen (H5; the owner's ruling replaces
//! the original self-signed leaf). The only new dependency is `rcgen`
//! (D-2 approved, MIT OR Apache-2.0); hyper/hyper-util/http-body-util/
//! tokio-rustls/rustls were already in Cargo.lock as reqwest transitive
//! deps at these exact versions — declaring them adds no crate.
//!
//! SECURITY MODEL (recon §6 Q1/Q2, rulings 4c + 5, hardening H1-H5):
//! - Binds `127.0.0.1:<port>` only. Never `0.0.0.0`.
//! - Host must be `localhost:<port>`; Origin must be
//!   `https://localhost:<port>` for every /v1 route except /v1/health.
//!   No CORS headers are ever emitted: the add-in page and the API share
//!   one origin, so any cross-origin browser request is rejected, not
//!   facilitated.
//! - Bearer token issued by /v1/pair against a 6-character one-time
//!   pairing code with a 5-minute TTL, a GLOBAL 5-failure budget that
//!   burns the code, a doubling cooldown before the next code, and a
//!   route-specific /v1/pair limiter; constant-time compares throughout
//!   (H1). The token persists as SHA-256 + metadata only (H2).
//! - 64 KiB body cap, sliding-window rate limits, TLS handshake/header/
//!   body timeouts, a 16-connection cap, max header count and size (H3).
//! - TLS 1.3 + 1.2 with the explicit ring provider, ALPN http/1.1 only
//!   (H4; 1.3-only comes after the owner's spike item (l)).
//! - Ephemeral CA with nameConstraints (DNS localhost, IP 127.0.0.1),
//!   CA key destroyed after issuing the leaf; the user trusts the CA,
//!   never the leaf (H5).
//! - No request logging of TEXT (the audit log records counts and hashes
//!   only — PROVENANCE_SPEC §0.3 pattern).
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
/// Tier 1 model-call timeout. The scheduler keeps its OWN deadline (H3:
/// handler-level budgets are independent of the socket limits).
const TIER1_TIMEOUT: Duration = Duration::from_secs(120);

/// H3: per-connection and per-request resource limits. Production uses
/// `NetLimits::production()`; the E2E tests drive tighter values through
/// `start_with_limits` so the timeout/cap paths are observable without
/// waiting on the production durations.
#[derive(Debug, Clone, Copy)]
pub struct NetLimits {
    /// Whole TLS handshake must complete inside this. Loopback handshakes
    /// take single-digit milliseconds; 5 s is 3 orders of magnitude of
    /// headroom and still caps slowloris-style TLS stalls.
    pub tls_handshake: Duration,
    /// Headers of the NEXT request must start arriving within this.
    /// hyper 1.x folds the keep-alive idle cap into the header read
    /// timeout (there is no separate keep_alive_timeout in hyper 1), so
    /// this one value bounds both slow headers and dormant keep-alive
    /// sockets. 30 s: Word pane requests are immediate bursts; a paused
    /// debugger is the only legitimate 30 s pause.
    pub header_read: Duration,
    /// Reading one request body. A 64 KiB body over loopback lands in
    /// milliseconds; 30 s only fires on a stalled or hostile client.
    pub body_read: Duration,
    /// Max simultaneous connections. Word's pane uses at most a browser
    /// per-host handful (≤6); 16 leaves headroom for Edge previews and
    /// health checks. Excess connections are closed immediately.
    pub max_concurrent: usize,
    /// Max header COUNT per request (legit requests carry ≤8).
    pub max_headers: usize,
    /// Max header BYTES per request (hyper's limit is bytes, ~10× the
    /// largest legitimate header set, blocks oversized-header floods at
    /// parse time).
    pub max_header_bytes: usize,
}

impl NetLimits {
    pub const fn production() -> Self {
        Self {
            tls_handshake: Duration::from_secs(5),
            header_read: Duration::from_secs(30),
            body_read: Duration::from_secs(30),
            max_concurrent: 16,
            max_headers: 32,
            max_header_bytes: 16 * 1024,
        }
    }
}

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

/// Generate (or load) the per-install certificate chain, then serve with
/// the production resource limits. `data_dir` is where the public
/// certificates live; the leaf private key goes to the OS keychain when
/// available, else a 0600 file next to the certs on unix (both disclosed
/// in the UI and SECURITY.md); on Windows the keychain is required.
pub async fn start(
    settings: AssistSettings,
    state: Arc<AssistState>,
    data_dir: std::path::PathBuf,
) -> Result<ServerHandle, String> {
    start_with_limits(settings, state, data_dir, NetLimits::production()).await
}

/// The full start path with injectable resource limits (H3/H6). Port 0 is
/// allowed here for tests: the handle reports the actually-bound port and
/// the Host/Origin pins use that port.
pub async fn start_with_limits(
    mut settings: AssistSettings,
    state: Arc<AssistState>,
    data_dir: std::path::PathBuf,
    limits: NetLimits,
) -> Result<ServerHandle, String> {
    // H5: ephemeral CA + leaf (owner's D-5 model replaces the old
    // self-signed leaf).
    let material = ensure_certificate_chain(&data_dir, &state.audit)?;
    // H4: build the config with the EXPLICIT ring provider and EXPLICIT
    // protocol versions [TLS1.3, TLS1.2] — not the defaults — and pin
    // ALPN to http/1.1 only. (The owner tightens to 1.3-only after spike
    // item (l) shows which webview runtime Word uses.)
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut server_config = rustls::ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
        .map_err(|e| format!("assist TLS version floor: {e}"))?
        .with_no_client_auth()
        .with_single_cert(vec![material.leaf], material.key)
        .map_err(|e| format!("assist TLS setup: {e}"))?;
    server_config.alpn_protocols = vec![b"http/1.1".to_vec()];
    let acceptor = TlsAcceptor::from(Arc::new(server_config));

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], settings.port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                format!(
                    "Port {0} is already in use. Another Word Assist listener (or a foreign process) is bound to it. Change the port in the Word Assist settings.",
                    settings.port
                )
            } else {
                format!("cannot bind 127.0.0.1:{}: {e}", settings.port)
            }
        })?;

    // Port 0 (tests): resolve the actual port and pin Host/Origin checks
    // to it. With an explicit port this is a no-op.
    settings.port = listener.local_addr().map_err(|e| e.to_string())?.port();

    // The handler snapshots the session settings from the shared state
    // (enable() sets them too; this keeps the resolved port coherent and
    // lets the E2E harness drive start_with_limits directly).
    *state.settings.lock().expect("settings lock") = Some(settings.clone());

    log::info!(
        "Word Assist listening on https://127.0.0.1:{0} (origin https://localhost:{0})",
        settings.port
    );
    *state.bound_port.lock().expect("bound port lock") = Some(settings.port);
    state
        .running
        .store(true, std::sync::atomic::Ordering::SeqCst);
    state.audit.record(
        "assist_listener",
        "127.0.0.1",
        &format!("word assist listener started on port {0}", settings.port),
        0,
        0,
    );

    // H3: connection accounting. Over the cap, the TCP stream is dropped
    // immediately (closed, not queued).
    let live: Arc<std::sync::atomic::AtomicUsize> =
        Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let task = tokio::spawn(async move {
        loop {
            let (stream, _peer) = match listener.accept().await {
                Ok(x) => x,
                Err(e) => {
                    log::warn!("assist accept error: {e}");
                    continue;
                }
            };
            if live.load(std::sync::atomic::Ordering::SeqCst) >= limits.max_concurrent {
                log::warn!(
                    "assist: connection cap ({0}) reached; excess connection closed",
                    limits.max_concurrent
                );
                drop(stream);
                continue;
            }
            live.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let guard = ConnGuard(Arc::clone(&live));
            let acceptor = acceptor.clone();
            let state = Arc::clone(&state);
            tokio::spawn(async move {
                // H3: the handshake itself is bounded; a stalled client
                // cannot hold a slot forever.
                let tls = match tokio::time::timeout(limits.tls_handshake, acceptor.accept(stream))
                    .await
                {
                    Ok(Ok(t)) => t,
                    _ => return, // handshake timeout/failures are routine noise
                };
                let svc = service_fn(move |req| handle(req, Arc::clone(&state), limits));
                let _ = hyper::server::conn::http1::Builder::new()
                    .keep_alive(true)
                    // hyper 1 needs a timer for header_read_timeout to fire.
                    .timer(hyper_util::rt::TokioTimer::new())
                    // Bounds slow headers AND idle keep-alive (see NetLimits).
                    .header_read_timeout(limits.header_read)
                    .max_headers(limits.max_headers)
                    .max_header_size(limits.max_header_bytes)
                    .serve_connection(TokioIo::new(tls), svc)
                    .await;
                drop(guard);
            });
        }
    });
    Ok(ServerHandle {
        port: settings.port,
        abort: task.abort_handle(),
    })
}

/// Decrements the live-connection counter when its future completes
/// (handshake failure included).
struct ConnGuard(Arc<std::sync::atomic::AtomicUsize>);

impl Drop for ConnGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

// ------------------------------------------------------------- dispatch

async fn handle(
    req: Request<Incoming>,
    state: Arc<AssistState>,
    limits: NetLimits,
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

    // H1: route-specific limiter on /v1/pair, ON TOP of the global one.
    // The pairing endpoint accepts one 5-attempt code; 10 req/60 s is
    // already generous for a human. Flood hits are NOT audited (auditing
    // every flood line would let an attacker drown the audit log); they
    // go to the application log instead.
    if path == "/v1/pair" && !state.pair_rate.allow("pair") {
        log::warn!("assist: /v1/pair rate limit hit (route limiter)");
        return Ok(error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "too many pairing attempts; wait a minute",
        ));
    }

    // Host pinning: exactly one accepted value.
    let expected_host = format!("localhost:{}", settings.port);
    match req.headers().get(HOST).and_then(|v| v.to_str().ok()) {
        Some(h) if h == expected_host => {}
        _ => return Ok(error_response(StatusCode::BAD_REQUEST, "unexpected Host")),
    }

    // /v1/* auth model: health is open (it leaks the engine version and
    // paired state only); pair is authenticated by the one-time pairing
    // code; everything else needs Origin pin + bearer.
    if route_requires_bearer(&path) {
        if let Err(msg) = check_origin(&req, &settings) {
            return Ok(error_response(StatusCode::FORBIDDEN, msg));
        }
        if let Err(msg) = check_bearer(&req, &state) {
            return Ok(error_response(StatusCode::UNAUTHORIZED, msg));
        }
    } else if path == "/v1/pair" {
        // Pair skips the bearer but not the Origin pin: an unpaired client
        // must be able to reach /v1/pair (that is the whole point of
        // pairing), and the pairing code is the secret that authenticates
        // it. Before this explicit branch the pair route demanded a bearer
        // token no unpaired client can have, which made first-time pairing
        // impossible. Same-origin GETs may omit Origin, which is why health
        // (GET) stays outside both checks.
        if let Err(msg) = check_origin(&req, &settings) {
            return Ok(error_response(StatusCode::FORBIDDEN, msg));
        }
    }

    let resp = match (&method, path.as_str()) {
        (&hyper::Method::GET, "/v1/health") => health(&state, &settings),
        (&hyper::Method::POST, "/v1/pair") => pair(req, state, limits).await,
        (&hyper::Method::POST, "/v1/check") => check(req, state, settings, limits).await,
        (&hyper::Method::POST, "/v1/accepted") => accepted(req, state, limits).await,
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

/// Route classification for the /v1 auth model: health (version only)
/// and pair (authenticated by the one-time pairing code) skip the bearer;
/// everything else under /v1 requires it. Pair still goes through the
/// Origin pin in `handle` — this predicate only decides the bearer step.
fn route_requires_bearer(path: &str) -> bool {
    path.starts_with("/v1/") && path != "/v1/health" && path != "/v1/pair"
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

async fn pair(
    req: Request<Incoming>,
    state: Arc<AssistState>,
    limits: NetLimits,
) -> Response<RespBody> {
    let Some(body) = read_body(req, limits.body_read).await else {
        return error_response(StatusCode::PAYLOAD_TOO_LARGE, "body too large");
    };
    let Ok(parsed) = serde_json::from_slice::<PairRequest>(&body) else {
        return error_response(StatusCode::BAD_REQUEST, "expected {\"code\": \"...\"}");
    };
    // H1: the outcome decides the audit wording. The presented code is
    // NEVER logged or audited — the audit events say what happened, not
    // what was typed.
    match state.pairing.take(&parsed.code) {
        Ok(super::settings::PairOutcome::Accepted) => {
            // H2: the token is issued here and persists as a hash record.
            let token = match state.issue_token() {
                Ok(t) => t,
                Err(e) => {
                    log::warn!("token issue failed: {e}");
                    state.audit.record(
                        "assist_pair",
                        "localhost",
                        "pairing code accepted but token issue FAILED (see log)",
                        body.len() as u64,
                        0,
                    );
                    return error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "token generation failed; the code is burned — request a fresh one",
                    );
                }
            };
            state.ledger_record("paired", "", "-", "-", "-", 0, 0, 0, "");
            state.audit.record(
                "assist_pair",
                "localhost",
                "add-in paired; bearer token issued (stored as hash only)",
                body.len() as u64,
                0,
            );
            json_response(StatusCode::OK, &serde_json::json!({ "token": token }))
        }
        Ok(super::settings::PairOutcome::Failure { attempts_used }) => {
            state.audit.record(
                "assist_pair_failed",
                "localhost",
                &format!("wrong pairing code (attempt {attempts_used} of 5)"),
                body.len() as u64,
                0,
            );
            error_response(
                StatusCode::UNAUTHORIZED,
                "wrong or expired pairing code; get a fresh code from the Word Assist tab",
            )
        }
        Ok(super::settings::PairOutcome::LockedOut) => {
            state.audit.record(
                "assist_pair_lockout",
                "localhost",
                "pairing burned (5 failed attempts or replay after burn); a NEW code requires an explicit user action after a cooldown",
                body.len() as u64,
                0,
            );
            error_response(
                StatusCode::UNAUTHORIZED,
                "pairing is disabled for now; open the Word Assist tab and request a new code",
            )
        }
        Ok(super::settings::PairOutcome::Expired) => {
            state.audit.record(
                "assist_pair_failed",
                "localhost",
                "pairing code expired (TTL elapsed); code cleared",
                body.len() as u64,
                0,
            );
            error_response(
                StatusCode::UNAUTHORIZED,
                "pairing code expired; get a fresh code from the Word Assist tab",
            )
        }
        Ok(super::settings::PairOutcome::NoCode) => {
            state.audit.record(
                "assist_pair_failed",
                "localhost",
                "no pairing code is active",
                body.len() as u64,
                0,
            );
            error_response(
                StatusCode::UNAUTHORIZED,
                "no pairing code is active; get a code from the Word Assist tab",
            )
        }
        Err(e) => {
            log::warn!("pairing state error: {e}");
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "pairing state error")
        }
    }
}

async fn check(
    req: Request<Incoming>,
    state: Arc<AssistState>,
    settings: AssistSettings,
    limits: NetLimits,
) -> Response<RespBody> {
    let Some(body) = read_body(req, limits.body_read).await else {
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
                    // The scheduler owns the stale-hash discard (1b
                    // ruling): a superseded result never reaches this
                    // callback, so the cache can only ever see the
                    // newest verdict.
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
    // The scheduler guarantees freshness: a superseded result is dropped
    // there (Scheduler::stale_drops) and never reaches this function.
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

async fn accepted(
    req: Request<Incoming>,
    state: Arc<AssistState>,
    limits: NetLimits,
) -> Response<RespBody> {
    let Some(body) = read_body(req, limits.body_read).await else {
        return error_response(StatusCode::PAYLOAD_TOO_LARGE, "body too large");
    };
    let Ok(parsed) = serde_json::from_slice::<AcceptedRequest>(&body) else {
        return error_response(StatusCode::BAD_REQUEST, "malformed accepted payload");
    };
    // Hashes and counts, never text (PROVENANCE_SPEC §0.3). The audit
    // trail feeds the Privacy Audit view; the hash-chained ledger (Phase
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

/// Read the body with the hard cap and a wall-clock timeout (H3).
/// Returns None when over budget (413) or when the body stalls past
/// `timeout` (the caller answers 413 for both — the observable difference
/// matters only to an attacker probing the cap).
async fn read_body(req: Request<Incoming>, timeout: Duration) -> Option<hyper::body::Bytes> {
    let limited = Limited::new(req.into_body(), BODY_CAP);
    match tokio::time::timeout(timeout, limited.collect()).await {
        Ok(Ok(collected)) => Some(collected.to_bytes()),
        Ok(Err(_)) => None,    // over cap
        Err(_elapsed) => None, // body read timeout
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
        // Test seam: E2E tests point this at a mock Ollama daemon so the
        // full check→call→cache loop is exercisable without a real model.
        let base = state
            .tier1_base_url_override
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .unwrap_or_else(|| crate::ollama::base_url().to_string());
        let outcome = std::thread::scope(|s| {
            s.spawn(move || {
                handle.block_on(async move {
                    tokio::time::timeout(
                        TIER1_TIMEOUT,
                        crate::ollama::chat_at(&state.ollama, &base, chat_req),
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
//
// H5 (owner ruling replacing the D-5 self-signed leaf): an EPHEMERAL CA
// signs the leaf, then is destroyed. The user trusts the CA cert, never
// the leaf.
// - CA: CA:TRUE, pathlen 0, KeyUsage = keyCertSign ONLY, nameConstraints
//   permitting DNS `localhost` and IP 127.0.0.1/32, validity 3 years.
//   The CA key is used exactly once (to issue the leaf), is never
//   serialized to this code, never written to disk or keychain, and is
//   dropped immediately after issuance.
// - Leaf: serverAuth EKU only, SAN localhost + 127.0.0.1, validity
//   strictly inside the CA's (365 days).
// - Leaf key: OS keychain (`scholarscribe` / `word-assist-tls-key-v1`).
//   Fallback: a 0600 file next to the certs, ONLY if the keychain fails,
//   ONLY on unix, WITH an audit event. On Windows the fallback is
//   REFUSED (no ACL dependency is added; Windows Credential Manager is
//   always present, so the code refuses rather than writes an
//   unhardened file) — stated for the owner as the chosen option.
// - Renewal: expired/missing material regenerates CA+leaf; the user
//   re-trusts the new CA (consent is the manual trust step).

/// CA validity: 3 years (owner proposal, accepted verbatim).
pub const CA_VALIDITY_DAYS: u64 = 3 * 365 + 1;
/// Leaf validity: 365 days — comfortably inside the CA's window; a
/// renewal regenerates CA + leaf together and re-asks consent.
pub const LEAF_VALIDITY_DAYS: u64 = 365;
/// Clock-skew allowance on the notBefore side.
const BACKDATE_SECS: u64 = 24 * 3600;

pub const CA_CERT_FILE: &str = "word-assist-ca.der";
pub const LEAF_CERT_FILE: &str = "word-assist-cert.der";
pub const KEY_FILE: &str = "word-assist-key.p8";
pub const KEYCHAIN_TLS_ENTRY: &str = "word-assist-tls-key-v1";

/// The loaded certificate chain plus where the leaf key came from.
pub struct TlsMaterial {
    pub ca: CertificateDer<'static>,
    pub leaf: CertificateDer<'static>,
    pub key: PrivateKeyDer<'static>,
    pub key_location: KeyLocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyLocation {
    Keychain,
    File,
}

/// Load or generate the CA+leaf chain for `data_dir`. `audit` receives a
/// `assist_tls_key_fallback` event when the 0600 file fallback is used.
pub fn ensure_certificate_chain(
    data_dir: &std::path::Path,
    audit: &crate::audit::AuditLog,
) -> Result<TlsMaterial, String> {
    std::fs::create_dir_all(data_dir).map_err(|e| format!("create assist dir: {e}"))?;
    let ca_path = data_dir.join(CA_CERT_FILE);
    let leaf_path = data_dir.join(LEAF_CERT_FILE);

    if ca_path.exists() && leaf_path.exists() {
        match load_existing_chain(&ca_path, &leaf_path, data_dir, audit) {
            Ok(material) => return Ok(material),
            Err(e) => {
                // Corrupt, expired, or key missing: regenerate below. The
                // old files are overwritten; trust must be re-granted for
                // the new CA (the registration flow re-asks consent).
                log::warn!("assist TLS material unusable ({e}); regenerating CA + leaf");
            }
        }
    }
    generate_chain(data_dir, audit)
}

/// Load CA + leaf from disk and the key from the keychain (or file
/// fallback). Fails when anything is expired or unreadable so the caller
/// can regenerate.
fn load_existing_chain(
    ca_path: &std::path::Path,
    leaf_path: &std::path::Path,
    data_dir: &std::path::Path,
    audit: &crate::audit::AuditLog,
) -> Result<TlsMaterial, String> {
    let ca_der = std::fs::read(ca_path).map_err(|e| format!("read assist CA: {e}"))?;
    let leaf_der = std::fs::read(leaf_path).map_err(|e| format!("read assist leaf: {e}"))?;
    let key =
        load_leaf_key(data_dir, audit)?.ok_or("assist TLS key not found in keychain or file")?;

    // Validity + sanity via x509-parser (H5 test parity): an expired or
    // unparseable chain is regenerated, never served.
    let now = time::OffsetDateTime::now_utc();
    for (label, der) in [("CA", &ca_der), ("leaf", &leaf_der)] {
        let (_, cert) = x509_parser::parse_x509_certificate(der.as_slice())
            .map_err(|e| format!("parse {label} cert: {e}"))?;
        let validity = cert.validity();
        if now < validity.not_before.to_datetime() {
            return Err(format!("{label} cert not yet valid"));
        }
        if now > validity.not_after.to_datetime() {
            return Err(format!("{label} cert expired"));
        }
    }
    Ok(TlsMaterial {
        ca: CertificateDer::from(ca_der),
        leaf: CertificateDer::from(leaf_der),
        key,
        key_location: leaf_key_location(),
    })
}

/// Where the leaf key would come from (mirrors load_leaf_key's decision).
fn leaf_key_location() -> KeyLocation {
    if let Ok(entry) = keyring::Entry::new("scholarscribe", KEYCHAIN_TLS_ENTRY) {
        if entry.get_password().is_ok() {
            return KeyLocation::Keychain;
        }
    }
    KeyLocation::File
}

/// Read the leaf key: keychain first (hex PKCS#8), then the 0600 file
/// next to the certs — unix only, with an audit event. Windows: keychain
/// only (fallback refused by ruling).
fn load_leaf_key(
    data_dir: &std::path::Path,
    audit: &crate::audit::AuditLog,
) -> Result<Option<PrivateKeyDer<'static>>, String> {
    if let Ok(entry) = keyring::Entry::new("scholarscribe", KEYCHAIN_TLS_ENTRY) {
        match entry.get_password() {
            Ok(hex) => {
                let bytes =
                    hex_decode(&hex).map_err(|e| format!("assist key format (keychain): {e}"))?;
                return PrivateKeyDer::try_from(bytes)
                    .map(Some)
                    .map_err(|e| format!("assist key format: {e}"));
            }
            Err(keyring::Error::NoEntry) => {}
            Err(e) => log::warn!("TLS key keychain read failed ({e}); trying file fallback"),
        }
    }
    #[cfg(unix)]
    {
        let path = data_dir.join(KEY_FILE);
        if path.exists() {
            let bytes = std::fs::read(&path).map_err(|e| format!("read assist key: {e}"))?;
            audit.record(
                "assist_tls_key_fallback",
                "localhost",
                &format!(
                    "TLS leaf key loaded from the 0600 file fallback (keychain unavailable): {}",
                    path.display()
                ),
                0,
                0,
            );
            return PrivateKeyDer::try_from(bytes)
                .map(Some)
                .map_err(|e| format!("assist key format: {e}"));
        }
        Ok(None)
    }
    #[cfg(not(unix))]
    {
        // Windows: no file fallback (refused by ruling, see module docs).
        let _ = data_dir;
        Ok(None)
    }
}

/// Generate a fresh ephemeral CA + leaf, store CA and leaf (public DER)
/// next to `data_dir` and the leaf key in the keychain (or 0600 file on
/// unix, with an audit event; refused on Windows).
fn generate_chain(
    data_dir: &std::path::Path,
    audit: &crate::audit::AuditLog,
) -> Result<TlsMaterial, String> {
    use rcgen::{
        BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, GeneralSubtree, IsCa,
        KeyUsagePurpose, NameConstraints,
    };
    let now = time::OffsetDateTime::now_utc();
    let backdated = now - time::Duration::seconds(BACKDATE_SECS as i64);

    // ---- the ephemeral CA -------------------------------------------
    let ca_key = rcgen::KeyPair::generate().map_err(|e| format!("CA key generation: {e}"))?;
    let mut ca_params = CertificateParams::default();
    ca_params.distinguished_name.push(
        DnType::CommonName,
        "ScholarScribe Word Assist Root CA (ephemeral)",
    );
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0)); // CA:TRUE, pathlen 0
    ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign]; // keyCertSign ONLY
    ca_params.name_constraints = Some(NameConstraints {
        permitted_subtrees: vec![
            GeneralSubtree::DnsName("localhost".to_string()),
            GeneralSubtree::IpAddress(rcgen::CidrSubnet::V4([127, 0, 0, 1], [255, 0, 0, 0])),
        ],
        excluded_subtrees: vec![],
    });
    ca_params.not_before = backdated;
    ca_params.not_after = backdated + time::Duration::days(CA_VALIDITY_DAYS as i64);
    let ca_cert = ca_params
        .self_signed(&ca_key)
        .map_err(|e| format!("CA self-sign: {e}"))?;
    let ca_der = ca_cert.der().clone();

    // ---- the leaf, signed ONCE by the CA ----------------------------
    let leaf_key = rcgen::KeyPair::generate().map_err(|e| format!("leaf key generation: {e}"))?;
    let mut leaf_params =
        CertificateParams::new(vec!["localhost".to_string(), "127.0.0.1".to_string()])
            .map_err(|e| format!("cert params: {e}"))?;
    leaf_params
        .distinguished_name
        .push(DnType::CommonName, "ScholarScribe Word Assist (localhost)");
    leaf_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth]; // serverAuth only
    leaf_params.not_before = backdated;
    leaf_params.not_after = backdated + time::Duration::days(LEAF_VALIDITY_DAYS as i64);
    let leaf_cert = leaf_params
        .signed_by(&leaf_key, &rcgen::Issuer::from_params(&ca_params, &ca_key))
        .map_err(|e| format!("leaf sign: {e}"))?;
    let leaf_der = leaf_cert.der().clone();
    let leaf_pkcs8 = leaf_key.serialize_der();

    // ---- the ephemeral CA key dies here ------------------------------
    // It was used exactly once (above). We never serialize it, never
    // write it anywhere; the rcgen/ring object is dropped immediately.
    // (ring zeroizes its own secret allocations on drop; we hold no copy
    // of the CA key bytes ourselves.)
    drop(ca_key);

    // ---- persist: certs are public; the leaf key is the secret -------
    let ca_path = data_dir.join(CA_CERT_FILE);
    let leaf_path = data_dir.join(LEAF_CERT_FILE);
    std::fs::write(&ca_path, ca_der.as_ref()).map_err(|e| format!("write assist CA: {e}"))?;
    std::fs::write(&leaf_path, leaf_der.as_ref()).map_err(|e| format!("write assist cert: {e}"))?;

    let stored_in_keychain = match keyring::Entry::new("scholarscribe", KEYCHAIN_TLS_ENTRY) {
        Ok(entry) => entry.set_password(&hex_encode(&leaf_pkcs8)).is_ok(),
        Err(_) => false,
    };
    let key_location = if stored_in_keychain {
        KeyLocation::Keychain
    } else {
        #[cfg(unix)]
        {
            let key_path = data_dir.join(KEY_FILE);
            super::settings::write_private(&key_path, &leaf_pkcs8)?;
            audit.record(
                "assist_tls_key_fallback",
                "localhost",
                &format!(
                    "OS keychain unavailable; TLS leaf key written to the 0600 file fallback at {} (owner's data dir)",
                    data_dir.join(KEY_FILE).display()
                ),
                0,
                0,
            );
            KeyLocation::File
        }
        #[cfg(not(unix))]
        {
            // Windows: the fallback is REFUSED (owner decision point:
            // "apply an owner-only ACL or refuse the fallback; state
            // which" — this build refuses; the keychain is required).
            return Err(
                "the OS keychain (Windows Credential Manager) is required to store the Word Assist TLS key; the unencrypted file fallback is refused on Windows"
                    .into(),
            );
        }
    };

    log::info!(
        "Word Assist certificate chain generated (leaf key in {loc}; CA at {ca})",
        loc = if key_location == KeyLocation::Keychain {
            "OS keychain"
        } else {
            "app data (0600)"
        },
        ca = ca_path.display()
    );
    Ok(TlsMaterial {
        ca: ca_der,
        leaf: leaf_der,
        key: PrivateKeyDer::Pkcs8(leaf_pkcs8.into()),
        key_location,
    })
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err("odd hex length".into());
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{hex_decode, hex_encode, NetLimits, ADDIN_CSP, BODY_CAP};

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
        assert_eq!(hex_decode("00ff").expect("decode"), vec![0x00, 0xff]);
        assert!(hex_decode("0").is_err());
    }

    #[test]
    fn pair_route_is_bearer_exempt_but_v1_routes_are_not() {
        assert!(!super::route_requires_bearer("/v1/health"));
        assert!(!super::route_requires_bearer("/v1/pair"));
        assert!(super::route_requires_bearer("/v1/check"));
        assert!(super::route_requires_bearer("/v1/accepted"));
        assert!(!super::route_requires_bearer("/addin/taskpane.html"));
        assert!(!super::route_requires_bearer("/v2/whatever"));
    }

    #[test]
    fn production_net_limits_match_the_stated_values() {
        let l = NetLimits::production();
        assert_eq!(l.tls_handshake.as_secs(), 5);
        assert_eq!(l.header_read.as_secs(), 30);
        assert_eq!(l.body_read.as_secs(), 30);
        assert_eq!(l.max_concurrent, 16);
        assert_eq!(l.max_headers, 32);
        assert_eq!(l.max_header_bytes, 16 * 1024);
    }

    /// H5: parse the generated chain with x509-parser and assert CA flag,
    /// pathlen, keyUsage, nameConstraints, EKU, SANs and validity bounds.
    #[test]
    fn generated_chain_matches_the_d5_model() {
        let dir = std::env::temp_dir().join(format!("wa-cert-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let audit = crate::audit::AuditLog::default();
        let material = super::ensure_certificate_chain(&dir, &audit).expect("chain");

        // ---- CA assertions ----
        let (rem, ca) =
            x509_parser::parse_x509_certificate(material.ca.as_ref()).expect("parse ca");
        assert!(rem.is_empty());
        assert!(ca.is_ca(), "CA basic constraint must be CA:TRUE");
        let bc = ca
            .basic_constraints()
            .expect("bc parses")
            .expect("bc present")
            .value;
        assert_eq!(bc.path_len_constraint, Some(0), "pathlen must be 0");
        let ku = ca
            .key_usage()
            .expect("keyUsage parses")
            .expect("keyUsage present")
            .value;
        assert!(ku.key_cert_sign(), "CA must have keyCertSign");
        assert!(!ku.digital_signature(), "CA keyUsage is keyCertSign ONLY");
        assert!(!ku.key_encipherment() && !ku.data_encipherment() && !ku.key_agreement());
        let nc = ca
            .name_constraints()
            .expect("nameConstraints parses")
            .expect("nameConstraints present")
            .value;
        let permitted_dns: Vec<&str> = nc
            .permitted_subtrees
            .as_ref()
            .map(|v| v.iter())
            .unwrap_or_default()
            .filter_map(|s| match &s.base {
                x509_parser::extensions::GeneralName::DNSName(d) => Some(*d),
                _ => None,
            })
            .collect();
        assert_eq!(permitted_dns, vec!["localhost"], "DNS constraint");
        let permitted_ips: Vec<Vec<u8>> = nc
            .permitted_subtrees
            .as_ref()
            .map(|v| v.iter())
            .unwrap_or_default()
            .filter_map(|s| match &s.base {
                x509_parser::extensions::GeneralName::IPAddress(b) => Some(b.to_vec()),
                _ => None,
            })
            .collect();
        assert_eq!(
            permitted_ips,
            vec![vec![127, 0, 0, 1, 255, 0, 0, 0]],
            "IP constraint = 127.0.0.1/32 (addr+mask)"
        );
        assert!(
            nc.excluded_subtrees
                .as_ref()
                .map(|v| v.is_empty())
                .unwrap_or(true),
            "no excluded subtrees"
        );
        // CA must NOT have a serverAuth EKU (it only signs).
        let ca_eku = ca
            .extended_key_usage()
            .ok()
            .and_then(|e| e.map(|x| x.value.server_auth));
        assert!(
            ca_eku.map(|v| !v).unwrap_or(true),
            "CA has no serverAuth EKU"
        );

        // ---- leaf assertions ----
        let (_, leaf) =
            x509_parser::parse_x509_certificate(material.leaf.as_ref()).expect("parse leaf");
        assert!(!leaf.is_ca(), "leaf must be CA:FALSE");
        let eku = leaf
            .extended_key_usage()
            .expect("EKU parses")
            .expect("EKU present")
            .value;
        assert!(eku.server_auth, "leaf EKU serverAuth");
        assert!(!eku.client_auth, "leaf EKU is serverAuth ONLY");
        let sans = leaf
            .subject_alternative_name()
            .expect("SANs parse")
            .expect("SANs present")
            .value
            .general_names
            .clone();
        let has_dns = sans.iter().any(|n| {
            matches!(
                n,
                x509_parser::extensions::GeneralName::DNSName("localhost")
            )
        });
        let has_ip = sans.iter().any(|n| {
            matches!(n,
            x509_parser::extensions::GeneralName::IPAddress(b) if *b == [127u8, 0, 0, 1])
        });
        assert!(has_dns && has_ip, "SANs must be localhost + 127.0.0.1");
        assert!(
            !sans.iter().any(|n| matches!(n,
                x509_parser::extensions::GeneralName::DNSName(d) if *d != "localhost")),
            "no other DNS SANs"
        );

        // ---- validity bounds ----
        let ca_validity = ca.validity();
        let leaf_validity = leaf.validity();
        let ca_days = (ca_validity.not_after - ca_validity.not_before)
            .expect("positive CA validity")
            .whole_days();
        let leaf_days = (leaf_validity.not_after - leaf_validity.not_before)
            .expect("positive leaf validity")
            .whole_days();
        assert_eq!(ca_days, super::CA_VALIDITY_DAYS as i64, "CA = 3 years");
        assert_eq!(
            leaf_days,
            super::LEAF_VALIDITY_DAYS as i64,
            "leaf = 365 days"
        );
        assert!(
            leaf_validity.not_after.timestamp() <= ca_validity.not_after.timestamp(),
            "leaf validity must not outlive the CA"
        );
        // Leaf is genuinely issued by this CA.
        assert_eq!(leaf.issuer(), ca.subject(), "leaf issuer = CA subject");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// H5: a SECOND ensure_certificate_chain call must LOAD the existing
    /// chain, not regenerate it (files byte-identical).
    #[test]
    fn chain_load_is_stable_across_calls() {
        let dir = std::env::temp_dir().join(format!("wa-cert-stable-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let audit = crate::audit::AuditLog::default();
        let m1 = super::ensure_certificate_chain(&dir, &audit).expect("first");
        let m2 = super::ensure_certificate_chain(&dir, &audit).expect("second");
        assert_eq!(m1.ca.as_ref(), m2.ca.as_ref(), "CA must be reloaded");
        assert_eq!(m1.leaf.as_ref(), m2.leaf.as_ref(), "leaf must be reloaded");
        std::fs::remove_dir_all(&dir).ok();
    }

    // ---------------------------------------------------------------
    // TEST-ONLY GENERATOR (H5, spike item (q)). Run manually:
    //   cargo test --features word-assist generate_owner_test_ca -- --ignored --nocapture
    // It writes a test CA (same constraints as the shipped one) plus two
    // leaves into target/word-assist-test-ca/. The owner installs the
    // TEST CA MANUALLY (anything that writes a trust store stays manual
    // and owner-run). leaf-localhost should validate in Word's pane and
    // Edge; leaf-other-name must FAIL (name constraint enforcement).
    // Nothing here is committed; the directory lives under target/.
    #[test]
    #[ignore = "TEST-ONLY owner-run generator; writes to target/"]
    fn generate_owner_test_ca() {
        use rcgen::{
            BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, GeneralSubtree,
            IsCa, KeyUsagePurpose, NameConstraints,
        };
        let out = std::path::Path::new("target/word-assist-test-ca");
        std::fs::create_dir_all(out).expect("mkdir");

        let ca_key = rcgen::KeyPair::generate().expect("ca key");
        let mut ca_params = CertificateParams::default();
        ca_params.distinguished_name.push(
            DnType::CommonName,
            "ScholarScribe TEST Word Assist Root CA (owner spike q)",
        );
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
        ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign];
        ca_params.name_constraints = Some(NameConstraints {
            permitted_subtrees: vec![
                GeneralSubtree::DnsName("localhost".to_string()),
                GeneralSubtree::IpAddress(rcgen::CidrSubnet::V4([127, 0, 0, 1], [255, 0, 0, 0])),
            ],
            excluded_subtrees: vec![],
        });
        let now = time::OffsetDateTime::now_utc();
        ca_params.not_before = now - time::Duration::seconds(3600);
        ca_params.not_after = now + time::Duration::days(365);
        let ca_cert = ca_params.self_signed(&ca_key).expect("ca cert");

        let leaf_key = rcgen::KeyPair::generate().expect("leaf key");
        let mut leaf_params =
            CertificateParams::new(vec!["localhost".to_string(), "127.0.0.1".to_string()])
                .expect("params");
        leaf_params.distinguished_name.push(
            DnType::CommonName,
            "ScholarScribe TEST Word Assist (localhost)",
        );
        leaf_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        leaf_params.not_before = now - time::Duration::seconds(3600);
        leaf_params.not_after = now + time::Duration::days(365);
        let leaf = leaf_params
            .signed_by(&leaf_key, &rcgen::Issuer::from_params(&ca_params, &ca_key))
            .expect("leaf");

        let other_key = rcgen::KeyPair::generate().expect("other key");
        let mut other_params =
            CertificateParams::new(vec!["other-name.test".to_string()]).expect("params");
        other_params
            .distinguished_name
            .push(DnType::CommonName, "WRONG NAME (must fail)");
        other_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        other_params.not_before = now - time::Duration::seconds(3600);
        other_params.not_after = now + time::Duration::days(365);
        let other = other_params
            .signed_by(&other_key, &rcgen::Issuer::from_params(&ca_params, &ca_key))
            .expect("other leaf");

        std::fs::write(out.join("TEST-ca.pem"), ca_cert.pem()).expect("ca pem");
        std::fs::write(out.join("leaf-localhost.pem"), leaf.pem()).expect("leaf pem");
        std::fs::write(out.join("leaf-localhost.key.pem"), leaf_key.serialize_pem())
            .expect("leaf key");
        std::fs::write(out.join("leaf-other-name.pem"), other.pem()).expect("other pem");
        std::fs::write(out.join("README.txt"),
            "TEST-ONLY artifacts (H5 spike item q).\n\
             1. Trust TEST-ca.pem MANUALLY (owner-run; nothing here writes a trust store).\n\
             2. Serve leaf-localhost on https://localhost:38080; it MUST load in Word's pane and Edge.\n\
             3. Serve leaf-other-name; it MUST FAIL (name constraints enforced by the webview runtime).\n\
             4. Delete the CA from the trust store afterwards.\n\
             This directory is under target/ and is never committed.\n").expect("readme");
        println!("TEST CA written to {}/", out.display());
    }
}
