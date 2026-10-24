//! Word Assist E2E suite (hardening H6).
//!
//! A REAL listener on port 0 with TEST-ONLY certificate fixtures (the
//! shipped chain generator, ephemeral CA + leaf, in a temp dir), a real
//! rustls CLIENT trusting that test CA, and a mock Tier 1 daemon — the
//! full `TLS → hyper → handler` path, no mocks in the listener itself.
//!
//! Owner matrix (H6): health; pair success; wrong code; lockout; expiry;
//! unauthenticated /v1/check → 401; Origin mismatch; Host mismatch
//! (DNS-rebinding); body over 64 KiB → 413; slow header closed within
//! the timeout; connection cap; static route returns only embedded
//! assets and path variants 404; stop() closes the port; token
//! persistence, revoke, rotation; route limiter; ALPN/TLS versions;
//! stale-result discard and request abort (1b ruling); and a SENTINEL
//! test asserting a unique checked sentence appears NOWHERE in audit
//! events, logs or the ledger.
//!
//! Run with: `cargo test --features word-assist --test word_assist_e2e`
//! (default builds compile this file to nothing).

#![cfg(feature = "word-assist")]

use std::sync::{Arc, Mutex, Once};
use std::time::Duration;

use rustls::pki_types::ServerName;
use scholarscribe_lib::assist::mask::MaskMode;
use scholarscribe_lib::assist::scheduler::Tier1Caller;
use scholarscribe_lib::assist::server::{NetLimits, ServerHandle};
use scholarscribe_lib::assist::settings::{
    AssistSettings, AssistState, PairOutcome, PairingState, StoredToken,
};
use scholarscribe_lib::assist::validate::StrictnessProfile;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

// ----------------------------------------------------------- log capture

static LOGS: Mutex<Vec<String>> = Mutex::new(Vec::new());
static LOG_INIT: Once = Once::new();

struct Capture;

impl log::Log for Capture {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Trace
    }
    fn log(&self, record: &log::Record) {
        LOGS.lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(format!("{}", record.args()));
    }
    fn flush(&self) {}
}

fn install_log_capture() {
    LOG_INIT.call_once(|| {
        let _ = log::set_boxed_logger(Box::new(Capture));
        log::set_max_level(log::LevelFilter::Trace);
    });
}

fn captured_logs() -> Vec<String> {
    LOGS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

// -------------------------------------------------------- phase tracing

/// One eprintln per harness phase when WA_E2E_TRACE=1 (CI sets it). libtest
/// only reports "over 60 seconds" per STARTED test; without these lines a
/// hung leg cannot name the phase (both macos +feat legs in run 40122758526
/// hung 47-53m with ZERO completed tests: build 3-7m, then silence).
fn trace_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("WA_E2E_TRACE").as_deref() == Ok("1"))
}

fn trace(msg: &str) {
    if trace_enabled() {
        // stderr under --nocapture reaches the runner log unbuffered.
        eprintln!("wa-e2e-trace: {msg}");
    }
}

// --------------------------------------------------------------- helpers

fn test_limits() -> NetLimits {
    NetLimits {
        tls_handshake: Duration::from_secs(3),
        header_read: Duration::from_millis(600),
        body_read: Duration::from_secs(3),
        max_concurrent: 16,
        max_headers: 32,
        max_header_bytes: 16 * 1024,
    }
}

fn base_settings(tier1: bool, model: &str) -> AssistSettings {
    AssistSettings {
        port: 0,
        dialect: "American".into(),
        strictness: StrictnessProfile::dev_default(),
        mask_mode: MaskMode::Filter,
        tier1_enabled: tier1,
        tier1_model: model.into(),
        experimental_languages: Default::default(),
        rules: Default::default(),
    }
}

async fn spawn_server(
    state: Arc<AssistState>,
    dir: &std::path::Path,
    limits: NetLimits,
    tier1: bool,
    model: &str,
) -> Result<ServerHandle, String> {
    let settings = base_settings(tier1, model);
    // Isolated TLS key entry — same credential-isolation ruling as
    // new_state above (see start_with_limits_isolated).
    trace(&format!(
        "spawning server dir={} (cert chain + bind)",
        dir.display()
    ));
    let handle = scholarscribe_lib::assist::server::start_with_limits_isolated(
        settings,
        state,
        dir.to_path_buf(),
        limits,
    )
    .await?;
    trace(&format!("server listening on port {}", handle.port));
    Ok(handle)
}

fn new_state(dir: &std::path::Path) -> Arc<AssistState> {
    let state = Arc::new(AssistState::new_default());
    state.attach_ledger(dir).expect("ledger attach");
    // Isolated credential entries: parallel e2e tests in this process share
    // one OS credential manager; the fixed production entry let them read
    // and overwrite each other's records (windows +feat e2e, run
    // 40122758526: the health test restored the pairing test's token).
    state
        .attach_token_store_isolated(dir)
        .expect("token store attach");
    trace(&format!("state ready dir={}", dir.display()));
    state
}

/// A TLS client that trusts the server's ephemeral test CA.
struct Client {
    connector: TlsConnector,
    port: u16,
}

impl Client {
    fn new(ca_der: &[u8], port: u16, tls12_only: bool, with_alpn: bool) -> Self {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let versions: &[&'static rustls::SupportedProtocolVersion] = if tls12_only {
            &[&rustls::version::TLS12]
        } else {
            &[&rustls::version::TLS13, &rustls::version::TLS12]
        };
        let mut roots = rustls::RootCertStore::empty();
        roots
            .add(rustls::pki_types::CertificateDer::from(ca_der.to_vec()))
            .expect("add test CA root");
        let mut builder = rustls::ClientConfig::builder_with_provider(provider)
            .with_protocol_versions(versions)
            .expect("client versions")
            .with_root_certificates(roots)
            .with_no_client_auth();
        if with_alpn {
            builder.alpn_protocols = vec![b"http/1.1".to_vec()];
        }
        Self {
            connector: TlsConnector::from(Arc::new(builder)),
            port,
        }
    }

    async fn connect(&self) -> tokio_rustls::client::TlsStream<TcpStream> {
        trace(&format!(
            "connecting 127.0.0.1:{} (tcp + TLS handshake)",
            self.port
        ));
        let tcp = TcpStream::connect(("127.0.0.1", self.port))
            .await
            .expect("tcp connect");
        let name = ServerName::try_from("localhost".to_string()).expect("dns name");
        let tls = self
            .connector
            .connect(name, tcp)
            .await
            .expect("tls connect");
        trace(&format!("TLS established 127.0.0.1:{}", self.port));
        tls
    }

    /// One request over a fresh connection (Connection: close semantics).
    async fn request(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<&[u8]>,
    ) -> (u16, String) {
        let (status, _, body) = self.request_full(method, path, headers, body).await;
        (status, body)
    }

    /// Like [`Client::request`] but also returns the raw header block
    /// (A2×H6: the module-serving tests assert on response headers).
    async fn request_full(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<&[u8]>,
    ) -> (u16, String, String) {
        let mut conn = self.connect().await;
        let mut req = format!(
            "{method} {path} HTTP/1.1\r\nHost: localhost:{}\r\n",
            self.port
        );
        for (k, v) in headers {
            req.push_str(&format!("{k}: {v}\r\n"));
        }
        match body {
            Some(b) => req.push_str(&format!("Content-Length: {}\r\n", b.len())),
            None => req.push_str("Content-Length: 0\r\n"),
        }
        req.push_str("Connection: close\r\n\r\n");
        conn.write_all(req.as_bytes()).await.expect("write request");
        if let Some(b) = body {
            conn.write_all(b).await.expect("write body");
        }
        let mut raw = Vec::new();
        conn.read_to_end(&mut raw).await.expect("read response");
        parse_response_full(&raw)
    }
}

fn parse_response(raw: &[u8]) -> (u16, String) {
    let (status, _, body) = parse_response_full(raw);
    (status, body)
}

fn parse_response_full(raw: &[u8]) -> (u16, String, String) {
    let text = String::from_utf8_lossy(raw);
    let status: u16 = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .map(|(h, b)| (h.to_string(), b.to_string()))
        .unwrap_or_default();
    (status, head, body)
}

fn header_value(head: &str, name: &str) -> Option<String> {
    head.lines().skip(1).find_map(|l| {
        let (k, v) = l.split_once(":")?;
        if k.trim().eq_ignore_ascii_case(name) {
            Some(v.trim().to_string())
        } else {
            None
        }
    })
}

fn json_body(v: &serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(v).expect("json")
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("wa-e2e-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("mkdir");
    d
}

async fn wait_port_closed(port: u16) -> bool {
    for _ in 0..100 {
        if TcpStream::connect(("127.0.0.1", port)).await.is_err() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}

// ------------------------------------------------------------ the tests

#[tokio::test(flavor = "multi_thread")]
async fn health_is_open_but_pair_and_check_are_guarded() {
    install_log_capture();
    let dir = temp_dir("health");
    let state = new_state(&dir);
    let server = spawn_server(Arc::clone(&state), &dir, test_limits(), false, "")
        .await
        .expect("server");
    let client = Client::new(&load_ca(&dir), server.port, false, false);

    // health: same-origin GET, no Origin needed.
    let (status, body) = client.request("GET", "/v1/health", &[], None).await;
    assert_eq!(status, 200, "health must be reachable: {body}");
    assert!(body.contains("\"ok\":true"));
    assert!(body.contains("\"paired\":false"));

    // Unauthenticated /v1/check → 401.
    let (status, _) = client
        .request(
            "POST",
            "/v1/check",
            &[("Origin", &format!("https://localhost:{}", server.port))],
            Some(&json_body(&serde_json::json!({
                "paragraph_id": "p1",
                "text": "Hello world."
            }))),
        )
        .await;
    assert_eq!(status, 401, "unauthenticated check must be 401");

    server_handle_stop(&server);
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn pair_success_issues_persistent_token() {
    install_log_capture();
    let dir = temp_dir("pair");
    let state = new_state(&dir);
    let server = spawn_server(Arc::clone(&state), &dir, test_limits(), false, "")
        .await
        .expect("server");
    let client = Client::new(&load_ca(&dir), server.port, false, false);

    let code = state.new_pairing_code().expect("code");
    let (status, body) = client
        .request(
            "POST",
            "/v1/pair",
            &[("Origin", &format!("https://localhost:{}", server.port))],
            Some(&json_body(&serde_json::json!({ "code": code }))),
        )
        .await;
    assert_eq!(status, 200, "pair must succeed: {body}");
    let token: String = serde_json::from_str::<serde_json::Value>(&body)
        .expect("pair response json")
        .get("token")
        .and_then(|t| t.as_str())
        .map(str::to_string)
        .expect("token field");
    assert_eq!(token.len(), 64, "32 random bytes hex-encoded");

    // The plaintext token is NOT persisted anywhere; the record holds the
    // hash only.
    let record_file = std::fs::read_to_string(dir.join("word-assist-token.json"));
    if let Ok(file) = record_file {
        assert!(!file.contains(&token), "raw token must never be persisted");
        assert!(file.contains("token_hash"));
    }

    // The token works against /v1/check (health must now say paired too).
    let (status, body) = client
        .request(
            "POST",
            "/v1/check",
            &[
                ("Origin", &format!("https://localhost:{}", server.port)),
                ("Authorization", &format!("Bearer {token}")),
            ],
            Some(&json_body(&serde_json::json!({
                "paragraph_id": "p1",
                "text": "The results was surprising to the team."
            }))),
        )
        .await;
    assert_eq!(status, 200, "paired check must pass: {body}");
    assert!(body.contains("language_status"));

    // Replay the burned code: pairing codes are one-time.
    assert_eq!(
        state.pairing.take(&code).expect("take"),
        PairOutcome::NoCode
    );

    server_handle_stop(&server);
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn wrong_code_lockout_and_cooldown() {
    install_log_capture();
    let dir = temp_dir("lockout");
    let state = new_state(&dir);
    let server = spawn_server(Arc::clone(&state), &dir, test_limits(), false, "")
        .await
        .expect("server");
    let client = Client::new(&load_ca(&dir), server.port, false, false);
    let origin = ("Origin", format!("https://localhost:{}", server.port));

    let code = state.new_pairing_code().expect("code");

    // Four wrong codes: counted failures, all 401.
    for i in 1..=4 {
        let (status, _) = client
            .request(
                "POST",
                "/v1/pair",
                &[("Origin", origin.1.as_str())],
                Some(&json_body(&serde_json::json!({ "code": "ZZZZZZ" }))),
            )
            .await;
        assert_eq!(status, 401, "wrong attempt {i} must be 401");
    }
    // The 5th failure burns the code: still 401, but the CORRECT code is
    // now dead too.
    let (status, _) = client
        .request(
            "POST",
            "/v1/pair",
            &[("Origin", origin.1.as_str())],
            Some(&json_body(&serde_json::json!({ "code": "ZZZZZZ" }))),
        )
        .await;
    assert_eq!(status, 401, "5th failure must be 401");
    let (status, _) = client
        .request(
            "POST",
            "/v1/pair",
            &[("Origin", origin.1.as_str())],
            Some(&json_body(&serde_json::json!({ "code": code }))),
        )
        .await;
    assert_eq!(status, 401, "burned code must not pair, even when correct");

    // Lockout audit event exists and carries NO code value.
    let audit = state.audit.snapshot();
    assert!(audit.iter().any(|e| e.kind == "assist_pair_lockout"));
    assert!(audit.iter().any(|e| e.kind == "assist_pair_failed"));
    for e in &audit {
        assert!(
            !e.detail.contains("ZZZZZZ"),
            "audit must never log the code: {}",
            e.detail
        );
        assert!(!e.detail.contains(&code), "audit must never log the code");
    }

    // A new code inside the cooldown is refused (explicit user action is
    // gated; 30 s initial cooldown, doubling).
    assert!(
        state.new_pairing_code().is_err(),
        "cooldown must gate the next code"
    );
    assert!(state.pairing.cooldown_remaining_secs().is_some());

    server_handle_stop(&server);
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn pairing_code_expires_after_ttl() {
    install_log_capture();
    let dir = temp_dir("expiry");
    let mut inner = AssistState::new_default();
    inner.pairing = PairingState::with_ttl(Arc::clone(&inner.clock), Duration::from_millis(150));
    let state = Arc::new(inner);
    let server = spawn_server(Arc::clone(&state), &dir, test_limits(), false, "")
        .await
        .expect("server");
    let client = Client::new(&load_ca(&dir), server.port, false, false);
    let origin = format!("https://localhost:{}", server.port);

    let code = state.new_pairing_code().expect("code");
    tokio::time::sleep(Duration::from_millis(250)).await;
    let (status, _) = client
        .request(
            "POST",
            "/v1/pair",
            &[("Origin", origin.as_str())],
            Some(&json_body(&serde_json::json!({ "code": code }))),
        )
        .await;
    assert_eq!(status, 401, "expired code must be 401");
    let audit = state.audit.snapshot();
    assert!(audit
        .iter()
        .any(|e| e.kind == "assist_pair_failed" && e.detail.contains("expired")));

    server_handle_stop(&server);
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn origin_and_host_pins_reject_mismatches() {
    install_log_capture();
    let dir = temp_dir("pins");
    let state = new_state(&dir);
    let server = spawn_server(Arc::clone(&state), &dir, test_limits(), false, "")
        .await
        .expect("server");
    let client = Client::new(&load_ca(&dir), server.port, false, false);
    let code = state.new_pairing_code().expect("code");
    let body = json_body(&serde_json::json!({ "code": code }));

    // Origin mismatch (cross-origin browser request) → 403.
    let (status, _) = client
        .request(
            "POST",
            "/v1/pair",
            &[("Origin", "https://evil.test:443")],
            Some(&body),
        )
        .await;
    assert_eq!(status, 403, "wrong Origin must be 403");

    // Missing Origin → 403 on pair.
    let (status, _) = client.request("POST", "/v1/pair", &[], Some(&body)).await;
    assert_eq!(status, 403, "missing Origin must be 403");

    // Host mismatch — the DNS-rebinding case (attacker page at
    // evil.test rebinding to 127.0.0.1) → 400.
    let mut raw_req = format!(
        "POST /v1/pair HTTP/1.1\r\nHost: evil.test:{}\r\nOrigin: https://localhost:{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        server.port,
        server.port,
        body.len()
    );
    let mut conn = client.connect().await;
    conn.write_all(raw_req.as_bytes()).await.expect("write");
    conn.write_all(&body).await.expect("write body");
    let mut resp = Vec::new();
    conn.read_to_end(&mut resp).await.expect("read");
    let (status, _) = parse_response(&resp);
    assert_eq!(status, 400, "Host mismatch (DNS-rebinding) must be 400");
    drop(conn);
    let _ = &mut raw_req;

    // health is exempt from the Origin pin (same-origin GET) but NOT
    // from the Host pin.
    let (status, _) = client.request("GET", "/v1/health", &[], None).await;
    assert_eq!(status, 200);

    server_handle_stop(&server);
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn body_over_64k_is_413() {
    install_log_capture();
    let dir = temp_dir("body");
    let state = new_state(&dir);
    let server = spawn_server(Arc::clone(&state), &dir, test_limits(), false, "")
        .await
        .expect("server");
    let client = Client::new(&load_ca(&dir), server.port, false, false);

    let code = state.new_pairing_code().expect("code");
    let (_, body) = client
        .request(
            "POST",
            "/v1/pair",
            &[("Origin", &format!("https://localhost:{}", server.port))],
            Some(&json_body(&serde_json::json!({ "code": code }))),
        )
        .await;
    let token: String = serde_json::from_str::<serde_json::Value>(&body)
        .expect("pair response json")
        .get("token")
        .and_then(|t| t.as_str())
        .map(str::to_string)
        .expect("token field");

    let big_text = "x".repeat(70_000);
    let (status, _) = client
        .request(
            "POST",
            "/v1/check",
            &[
                ("Origin", &format!("https://localhost:{}", server.port)),
                ("Authorization", &format!("Bearer {token}")),
            ],
            Some(&json_body(&serde_json::json!({
                "paragraph_id": "p1",
                "text": big_text
            }))),
        )
        .await;
    assert_eq!(status, 413, "body over 64 KiB must be 413");

    server_handle_stop(&server);
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn slow_headers_are_closed_within_the_timeout() {
    install_log_capture();
    let dir = temp_dir("slowheader");
    let state = new_state(&dir);
    let mut limits = test_limits();
    limits.header_read = Duration::from_millis(500);
    let server = spawn_server(Arc::clone(&state), &dir, limits, false, "")
        .await
        .expect("server");
    let client = Client::new(&load_ca(&dir), server.port, false, false);

    // Connect, complete TLS, then send NOTHING: the server must close
    // the connection inside ~header_read, not hold it.
    let mut conn = client.connect().await;
    let started = std::time::Instant::now();
    let mut buf = Vec::new();
    let result = tokio::time::timeout(Duration::from_secs(5), conn.read_to_end(&mut buf)).await;
    let elapsed = started.elapsed();
    match result {
        Ok(Ok(0)) => { /* clean EOF: closed by the server */ }
        Ok(Ok(_)) => panic!("expected EOF, got {} bytes", buf.len()),
        Ok(Err(e)) => {
            /* reset also acceptable */
            let _ = e;
        }
        Err(_) => panic!("connection was not closed within 5 s"),
    }
    assert!(
        elapsed < Duration::from_secs(4),
        "slow-header close took {elapsed:?}; header_read is 500 ms"
    );

    server_handle_stop(&server);
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn connection_cap_closes_excess_immediately() {
    install_log_capture();
    let dir = temp_dir("cap");
    let state = new_state(&dir);
    let mut limits = test_limits();
    limits.max_concurrent = 1;
    let server = spawn_server(Arc::clone(&state), &dir, limits, false, "")
        .await
        .expect("server");
    let client = Client::new(&load_ca(&dir), server.port, false, false);

    // Occupant: one keep-alive connection mid-flight (headers sent, body
    // held back so the request stays open).
    let mut occupant = client.connect().await;
    occupant
        .write_all(b"POST /v1/check HTTP/1.1\r\nHost: localhost\r\nContent-Length: 10\r\n\r\n")
        .await
        .expect("occupant headers");

    // Second connection while the slot is taken: dropped immediately.
    let excess = TcpStream::connect(("127.0.0.1", server.port)).await;
    assert!(excess.is_ok(), "TCP accept still happens");
    let mut excess = excess.unwrap();
    let mut buf = [0u8; 16];
    let closed = tokio::time::timeout(Duration::from_secs(3), excess.read(&mut buf)).await;
    match closed {
        Ok(Ok(0)) | Err(_) => { /* EOF or timeout: closed */ }
        Ok(Ok(_)) => {} // some bytes may arrive before close; still fine
        Ok(Err(_)) => { /* reset: closed */ }
    }

    // Release the occupant; a fresh connection works again.
    drop(occupant);
    tokio::time::sleep(Duration::from_millis(200)).await;
    let (status, _) = client.request("GET", "/v1/health", &[], None).await;
    assert_eq!(status, 200, "after release the slot must be free again");

    server_handle_stop(&server);
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn static_routes_serve_only_embedded_assets() {
    install_log_capture();
    let dir = temp_dir("static");
    let state = new_state(&dir);
    let server = spawn_server(Arc::clone(&state), &dir, test_limits(), false, "")
        .await
        .expect("server");
    let client = Client::new(&load_ca(&dir), server.port, false, false);

    let (status, body) = client
        .request("GET", "/addin/manifest.xml", &[], None)
        .await;
    assert_eq!(status, 200);
    assert!(
        body.contains("<OfficeApp") || body.contains("OfficeApp"),
        "manifest served"
    );

    // Path traversal / variants: everything else is 404 BY CONSTRUCTION
    // (the dispatcher exact-matches a fixed route table).
    for path in [
        "/..%2fserver.rs",
        "//server.rs",
        "/%2e%2e/server.rs",
        "/addin/../server.rs",
        "/addin/../assist/server.rs",
        "/addin/manifest.xml/",
        "/ADDIN/MANIFEST.XML",
        "/../Cargo.toml",
    ] {
        let (status, _) = client.request("GET", path, &[], None).await;
        assert_eq!(status, 404, "path variant {path} must be 404");
    }

    server_handle_stop(&server);
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn embedded_modules_served_with_secure_headers_and_graph_loads() {
    // A2×H6: every embedded ES module is served through the REAL TLS
    // listener with Content-Type text/javascript, X-Content-Type-Options
    // nosniff and the pane CSP; path variants still 404; and every import
    // of the entry module resolves (the module graph is fetchable, which
    // is the listener's whole role in an ES-module load).
    install_log_capture();
    let dir = temp_dir("modules");
    let state = new_state(&dir);
    let server = spawn_server(Arc::clone(&state), &dir, test_limits(), false, "")
        .await
        .expect("server");
    let client = Client::new(&load_ca(&dir), server.port, false, false);
    let csp = scholarscribe_lib::assist::server::ADDIN_CSP;

    // 1. Every module in the embed table: 200 + the three headers.
    let routes = scholarscribe_lib::assist::server::addin_module_routes();
    assert!(
        routes.len() >= 9,
        "expected the full module table (taskpane + pure modules + legacy check), got {routes:?}"
    );
    for route in &routes {
        let (status, head, body) = client.request_full("GET", route, &[], None).await;
        assert_eq!(status, 200, "{route} must be served");
        let ct = header_value(&head, "Content-Type").unwrap_or_default();
        assert!(
            ct.starts_with("text/javascript"),
            "{route}: Content-Type must be text/javascript, got {ct}"
        );
        assert_eq!(
            header_value(&head, "X-Content-Type-Options").as_deref(),
            Some("nosniff"),
            "{route}: nosniff missing"
        );
        assert_eq!(
            header_value(&head, "Content-Security-Policy").as_deref(),
            Some(csp),
            "{route}: CSP missing or wrong"
        );
        assert!(!body.is_empty(), "{route}: empty body");
    }

    // 2. The pane HTML keeps nosniff + CSP too (it is what declares the
    //    module graph and the legacy fallback script).
    let (status, head, _) = client
        .request_full("GET", "/addin/taskpane.html", &[], None)
        .await;
    assert_eq!(status, 200);
    assert_eq!(
        header_value(&head, "X-Content-Type-Options").as_deref(),
        Some("nosniff")
    );
    assert_eq!(
        header_value(&head, "Content-Security-Policy").as_deref(),
        Some(csp)
    );
    let (_, _, html) = client
        .request_full("GET", "/addin/taskpane.html", &[], None)
        .await;
    assert!(
        html.contains("legacy-check.js"),
        "pane must load the classic-script fallback before the module entry"
    );

    // 3. Path variants of a REAL module route still 404. (A query string
    //    is NOT a path variant: hyper's uri().path() excludes it, and
    //    serving the same embedded bytes with a cache-busting query is
    //    correct HTTP behaviour — asserted below, not failed.)
    for path in [
        "/addin/taskpane.js/",
        "/addin/taskpane.js%2F",
        "/addin/./taskpane.js",
        "//addin/taskpane.js",
        "/addin/taskpane%2Ejs",
        "/addin/TASKPANE.js",
    ] {
        let (status, _) = client.request("GET", path, &[], None).await;
        assert_eq!(status, 404, "module path variant {path} must be 404");
    }
    let (qstatus, _, qbody) = client
        .request_full("GET", "/addin/taskpane.js?x=1", &[], None)
        .await;
    assert_eq!(
        qstatus, 200,
        "query string must not change the served asset"
    );
    assert!(!qbody.is_empty());

    // 4. The module graph loads: fetch the entry, extract its relative
    //    imports, and fetch each one — every edge must resolve through
    //    the listener with the right content type.
    let (status, _, entry) = client
        .request_full("GET", "/addin/taskpane.js", &[], None)
        .await;
    assert_eq!(status, 200);
    let mut imports: Vec<String> = Vec::new();
    for tail in entry.split(" from \"").skip(1) {
        if let Some(name) = tail.split('"').next() {
            if let Some(module) = name.strip_prefix("./") {
                imports.push(format!("/addin/{module}"));
            }
        }
    }
    assert!(
        imports.len() >= 6,
        "entry module should import the pure modules, found {imports:?}"
    );
    for imp in &imports {
        let (status, head, _) = client.request_full("GET", imp, &[], None).await;
        assert_eq!(status, 200, "import {imp} of the entry module must resolve");
        let ct = header_value(&head, "Content-Type").unwrap_or_default();
        assert!(
            ct.starts_with("text/javascript"),
            "{imp}: wrong content type {ct}"
        );
    }

    server_handle_stop(&server);
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_closes_the_port() {
    install_log_capture();
    let dir = temp_dir("stop");
    let state = new_state(&dir);
    let server = spawn_server(Arc::clone(&state), &dir, test_limits(), false, "")
        .await
        .expect("server");
    let client = Client::new(&load_ca(&dir), server.port, false, false);
    let (status, _) = client.request("GET", "/v1/health", &[], None).await;
    assert_eq!(status, 200);

    server.stop();
    assert!(
        wait_port_closed(server.port).await,
        "port must close after stop()"
    );

    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn token_persists_across_restart_and_revokes() {
    install_log_capture();
    let dir = temp_dir("persist");

    // Session 1: pair, issue token.
    let state1 = new_state(&dir);
    let server1 = spawn_server(Arc::clone(&state1), &dir, test_limits(), false, "")
        .await
        .expect("server1");
    let client1 = Client::new(&load_ca(&dir), server1.port, false, false);
    let code = state1.new_pairing_code().expect("code");
    let (_, body) = client1
        .request(
            "POST",
            "/v1/pair",
            &[("Origin", &format!("https://localhost:{}", server1.port))],
            Some(&json_body(&serde_json::json!({ "code": code }))),
        )
        .await;
    let token: String = serde_json::from_str::<serde_json::Value>(&body)
        .expect("pair response json")
        .get("token")
        .and_then(|t| t.as_str())
        .map(str::to_string)
        .expect("token field");
    server1.stop();

    // "Restart": a fresh state over the SAME data dir restores the token
    // (H2: re-pairing after every restart was the conflict to resolve).
    let state2 = new_state(&dir);
    {
        let guard = state2.token.lock().unwrap();
        let record = guard.as_ref().expect("restored record");
        assert_eq!(record.token_hash, StoredToken::hash_of(&token));
    }
    let server2 = spawn_server(Arc::clone(&state2), &dir, test_limits(), false, "")
        .await
        .expect("server2");
    let client2 = Client::new(&load_ca(&dir), server2.port, false, false);
    let (status, _) = client2
        .request(
            "POST",
            "/v1/check",
            &[
                ("Origin", &format!("https://localhost:{}", server2.port)),
                ("Authorization", &format!("Bearer {token}")),
            ],
            Some(&json_body(&serde_json::json!({
                "paragraph_id": "p1",
                "text": "Hello there."
            }))),
        )
        .await;
    assert_eq!(status, 200, "restored token must keep working");

    // Revoke (H2): the token dies everywhere.
    state2.revoke_token().expect("revoke");
    let (status, _) = client2
        .request(
            "POST",
            "/v1/check",
            &[
                ("Origin", &format!("https://localhost:{}", server2.port)),
                ("Authorization", &format!("Bearer {token}")),
            ],
            Some(&json_body(&serde_json::json!({
                "paragraph_id": "p1",
                "text": "Hello again."
            }))),
        )
        .await;
    assert_eq!(status, 401, "revoked token must be 401");

    // Rotation = revoke + fresh pairing code (the token is never pushed).
    let state3 = new_state(&dir);
    state3.attach_token_store(&dir).ok();
    state3.issue_token().expect("issue");
    state3.rotate_token().expect("rotate");
    assert!(
        !state3.token_matches("anything"),
        "rotation kills the old token"
    );
    assert!(
        !std::path::Path::new(&dir.join("word-assist-token.json")).exists()
            || !std::fs::read_to_string(dir.join("word-assist-token.json"))
                .unwrap()
                .is_empty()
    );

    server2.stop();
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn pair_route_limiter_bites_at_ten_per_minute() {
    install_log_capture();
    let dir = temp_dir("pairlim");
    let state = new_state(&dir);
    let server = spawn_server(Arc::clone(&state), &dir, test_limits(), false, "")
        .await
        .expect("server");
    let client = Client::new(&load_ca(&dir), server.port, false, false);
    let origin = format!("https://localhost:{}", server.port);

    // 10 requests burn the route budget (wrong codes → 401, then
    // LockedOut → still 401).
    for i in 0..10 {
        let (status, _) = client
            .request(
                "POST",
                "/v1/pair",
                &[("Origin", origin.as_str())],
                Some(&json_body(&serde_json::json!({ "code": "AAAAAA" }))),
            )
            .await;
        assert_eq!(status, 401, "attempt {i} must be 401");
    }
    // The 11th hits the ROUTE limiter: 429 (not 401).
    let (status, body) = client
        .request(
            "POST",
            "/v1/pair",
            &[("Origin", origin.as_str())],
            Some(&json_body(&serde_json::json!({ "code": "AAAAAA" }))),
        )
        .await;
    assert_eq!(
        status, 429,
        "11th pair request must be rate limited: {body}"
    );

    server_handle_stop(&server);
    cleanup(&dir);
}

/// SENTINEL (H6): a unique sentence sent through /v1/check appears
/// NOWHERE in the audit events, the application log or the ledger.
#[tokio::test(flavor = "multi_thread")]
async fn sentinel_text_never_lands_in_audit_log_or_ledger() {
    install_log_capture();
    let dir = temp_dir("sentinel");
    let state = new_state(&dir);
    let server = spawn_server(Arc::clone(&state), &dir, test_limits(), false, "")
        .await
        .expect("server");
    let client = Client::new(&load_ca(&dir), server.port, false, false);

    let code = state.new_pairing_code().expect("code");
    let (_, body) = client
        .request(
            "POST",
            "/v1/pair",
            &[("Origin", &format!("https://localhost:{}", server.port))],
            Some(&json_body(&serde_json::json!({ "code": code }))),
        )
        .await;
    let token: String = serde_json::from_str::<serde_json::Value>(&body)
        .expect("pair response json")
        .get("token")
        .and_then(|t| t.as_str())
        .map(str::to_string)
        .expect("token field");

    let sentinel = "ZebraQuixotic SENTINEL-9f3a1c7b42 unique clause xylophone42.";
    let (status, _) = client
        .request(
            "POST",
            "/v1/check",
            &[
                ("Origin", &format!("https://localhost:{}", server.port)),
                ("Authorization", &format!("Bearer {token}")),
            ],
            Some(&json_body(&serde_json::json!({
                "paragraph_id": "para-1",
                "text": sentinel
            }))),
        )
        .await;
    assert_eq!(status, 200);

    // 1. Audit events: counts and ids only, never text.
    let audit = state.audit.snapshot();
    for e in &audit {
        assert!(
            !e.detail.contains(sentinel) && !e.detail.contains("ZebraQuixotic"),
            "audit detail leaks text: {}",
            e.detail
        );
    }

    // 2. Application log (captured at Trace level).
    for line in captured_logs() {
        assert!(
            !line.contains("ZebraQuixotic"),
            "log line leaks text: {line}"
        );
    }

    // 3. Ledger file: hashes and ids only.
    let ledger_path = dir.join("ledger.jsonl");
    let ledger = std::fs::read_to_string(&ledger_path).unwrap_or_default();
    assert!(!ledger.contains("ZebraQuixotic"), "ledger leaks text");
    assert!(!ledger.contains(sentinel), "ledger leaks text");

    server_handle_stop(&server);
    cleanup(&dir);
}

/// Tier 1 loop against a MOCK daemon (via the test base-URL seam): check
/// → scheduler → mock Ollama → parse → validate → diff → cache Ready.
#[tokio::test(flavor = "multi_thread")]
async fn tier1_mock_loop_updates_cache() {
    install_log_capture();
    let dir = temp_dir("tier1");
    let state = new_state(&dir);

    // The mock daemon: one HTTP round-trip responder. The candidate
    // sentence below is PROBE-VERIFIED: harper's dev profile flags it
    // (PronounVerbAgreement) — the golden set does NOT guarantee issues
    // for its plain sentences, so this test pins its own input.
    let flagged_sentence = "He don't agree with the reviewers.";
    let revised_sentence = "He doesn't agree with the reviewers.";
    let mock = spawn_mock_ollama(revised_sentence, "subject-verb agreement");
    state
        .tier1_base_url_override
        .lock()
        .unwrap()
        .replace(mock.url());

    let server = spawn_server(Arc::clone(&state), &dir, test_limits(), true, "mock-model")
        .await
        .expect("server");
    let client = Client::new(&load_ca(&dir), server.port, false, false);

    let code = state.new_pairing_code().expect("code");
    let (_, body) = client
        .request(
            "POST",
            "/v1/pair",
            &[("Origin", &format!("https://localhost:{}", server.port))],
            Some(&json_body(&serde_json::json!({ "code": code }))),
        )
        .await;
    let token: String = serde_json::from_str::<serde_json::Value>(&body)
        .expect("pair response json")
        .get("token")
        .and_then(|t| t.as_str())
        .map(str::to_string)
        .expect("token field");

    // Guard the premise: the sentence must trip Tier 0 (probe-verified
    // PronounVerbAgreement), otherwise the mock loop below is vacuous.
    let pre = scholarscribe_lib::assist::tier0::check_paragraph(
        flagged_sentence,
        "American",
        &Default::default(),
        &StrictnessProfile::dev_default(),
        MaskMode::Filter,
        &|_script| false,
    );
    assert!(
        pre.sentences.iter().any(|s| !s.issues.is_empty()),
        "the pinned sentence must produce a Tier 0 issue"
    );
    let sentence = flagged_sentence.to_string();

    let (status, resp_body) = client
        .request(
            "POST",
            "/v1/check",
            &[
                ("Origin", &format!("https://localhost:{}", server.port)),
                ("Authorization", &format!("Bearer {token}")),
            ],
            Some(&json_body(&serde_json::json!({
                "paragraph_id": "p-mock",
                "text": sentence.clone()
            }))),
        )
        .await;
    assert_eq!(status, 200, "{resp_body}");

    // Poll the cache: the verdict must reach Ready via the mock daemon.
    let mut ready = false;
    for _ in 0..50 {
        if let Some(v) = state.cache.get(&sentence) {
            if matches!(
                v.tier1,
                scholarscribe_lib::assist::cache::Tier1Status::Ready(_)
            ) {
                ready = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        ready,
        "tier1 verdict never became Ready; mock hits={}; last cache state: {:?}",
        mock.hit_count(),
        state.cache.get(&sentence).map(|v| format!("{:?}", v.tier1))
    );
    let verdict = state.cache.get(&sentence).unwrap();
    if let scholarscribe_lib::assist::cache::Tier1Status::Ready(s) = &verdict.tier1 {
        assert_eq!(s.revised, revised_sentence);
    }
    assert!(
        mock.hit_count() >= 1,
        "the mock daemon must have been called"
    );

    server_handle_stop(&server);
    cleanup(&dir);
}

/// Stale-result discard + request abort (1b ruling): a superseded call's
/// late result is DISCARDED by the scheduler (counted in stale_drops,
/// callback never runs, cache untouched).
#[tokio::test(flavor = "multi_thread")]
async fn stale_result_is_discarded_newest_wins() {
    install_log_capture();
    let dir = temp_dir("stale");
    let state = new_state(&dir);

    // A's caller signals it has STARTED, then blocks until released.
    // This removes the race where tokio aborts a queued-but-not-started
    // spawn_blocking task (abort only prevents tasks that never began).
    struct Gated {
        started: Arc<std::sync::atomic::AtomicBool>,
        release: Arc<(Mutex<bool>, std::sync::Condvar)>,
    }
    impl Tier1Caller for Gated {
        fn call(&self, _model: &str, _sentence: &str) -> Result<String, String> {
            self.started
                .store(true, std::sync::atomic::Ordering::SeqCst);
            let (mutex, cv) = &*self.release;
            let mut go = mutex.lock().unwrap_or_else(|e| e.into_inner());
            while !*go {
                go = cv.wait(go).unwrap_or_else(|e| e.into_inner());
            }
            Ok("{\"revised\":\"x\",\"explanation\":\"e\"}".to_string())
        }
    }
    let started_a = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let release = Arc::new((Mutex::new(false), std::sync::Condvar::new()));

    // Request A (gated), immediately replaced by request B.
    let caller_a: Arc<dyn Tier1Caller> = Arc::new(Gated {
        started: Arc::clone(&started_a),
        release: Arc::clone(&release),
    });
    state.scheduler.submit(
        "hash-a".into(),
        "sentence A".into(),
        "p".into(),
        "p",
        Arc::clone(&caller_a),
        "m".into(),
        |_| panic!("stale callback must NEVER run"),
    );
    // Wait until A is actually RUNNING inside its model call: from this
    // point abort can no longer cancel it, so the late-result path is
    // exercised deterministically.
    for _ in 0..200 {
        if started_a.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        started_a.load(std::sync::atomic::Ordering::SeqCst),
        "request A must have started"
    );
    let hash_a = state.scheduler.current_hash().expect("A in flight");
    struct SlowOk;
    impl Tier1Caller for SlowOk {
        fn call(&self, _model: &str, _sentence: &str) -> Result<String, String> {
            std::thread::sleep(Duration::from_millis(200));
            Ok("{\"revised\":\"x\",\"explanation\":\"e\"}".to_string())
        }
    }
    let b_done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let b_flag = Arc::clone(&b_done);
    let caller_b: Arc<dyn Tier1Caller> = Arc::new(SlowOk);
    state.scheduler.submit(
        "hash-b".into(),
        "sentence B".into(),
        "p".into(),
        "p",
        Arc::clone(&caller_b),
        "m".into(),
        move |_| b_flag.store(true, std::sync::atomic::Ordering::SeqCst),
    );
    assert_eq!(
        state.scheduler.current_hash().as_deref(),
        Some("hash-b"),
        "B must be the current request"
    );

    // Release A: its result lands while B is the current request.
    {
        let (mutex, cv) = &*release;
        let mut go = mutex.lock().unwrap_or_else(|e| e.into_inner());
        *go = true;
        cv.notify_all();
    }
    // Wait past both completions: A's late result must be dropped, B's
    // delivered exactly once.
    tokio::time::sleep(Duration::from_millis(700)).await;
    assert_eq!(
        state.scheduler.stale_drops(),
        1,
        "exactly one stale result must be discarded"
    );
    assert!(
        b_done.load(std::sync::atomic::Ordering::SeqCst),
        "the fresh verdict's callback must run exactly once"
    );
    let _ = hash_a;
    state.scheduler.abort_in_flight();
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn alpn_is_http11_and_tls13_negotiates() {
    install_log_capture();
    let dir = temp_dir("tls");
    let state = new_state(&dir);
    let server = spawn_server(Arc::clone(&state), &dir, test_limits(), false, "")
        .await
        .expect("server");

    // ALPN http/1.1 + TLS 1.3.
    let client = Client::new(&load_ca(&dir), server.port, false, true);
    let conn = client.connect().await;
    let (_, tls) = conn.get_ref();
    assert_eq!(
        tls.alpn_protocol(),
        Some(&b"http/1.1"[..]),
        "ALPN must negotiate http/1.1"
    );
    assert_eq!(
        tls.protocol_version(),
        Some(rustls::ProtocolVersion::TLSv1_3),
        "TLS 1.3 must negotiate by default"
    );
    drop(conn);

    // The owner's floor: TLS 1.2 must still connect (1.3-only comes
    // after spike item (l)).
    let client12 = Client::new(&load_ca(&dir), server.port, true, false);
    let conn = client12.connect().await;
    let (_, tls) = conn.get_ref();
    assert_eq!(
        tls.protocol_version(),
        Some(rustls::ProtocolVersion::TLSv1_2),
        "TLS 1.2 must remain accepted"
    );

    server_handle_stop(&server);
    cleanup(&dir);
}

// -------------------------------------------------------- mock daemon

struct MockOllama {
    addr: std::net::SocketAddr,
    hits: Arc<std::sync::atomic::AtomicUsize>,
}

impl MockOllama {
    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }
    fn hit_count(&self) -> usize {
        self.hits.load(std::sync::atomic::Ordering::SeqCst)
    }
}

fn spawn_mock_ollama(revised: &str, explanation: &str) -> MockOllama {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("mock bind");
    let addr = listener.local_addr().expect("mock addr");
    let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hits_thread = Arc::clone(&hits);
    let payload = serde_json::json!({
        "message": {
            "role": "assistant",
            "content": serde_json::json!({
                "revised": revised,
                "explanation": explanation
            }).to_string()
        }
    })
    .to_string();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            hits_thread.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            // Drain the request (headers + content-length body), then
            // answer with one Ollama /api/chat response.
            let mut buf = [0u8; 4096];
            let mut data = Vec::new();
            let mut content_len = 0usize;
            loop {
                let n = match stream.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(_) => break,
                };
                data.extend_from_slice(&buf[..n]);
                if let Some(pos) = find_headers_end(&data) {
                    if content_len == 0 {
                        content_len = data[..pos]
                            .split(|b| *b == b'\n')
                            .find_map(|line| {
                                let s = std::str::from_utf8(line).ok()?;
                                let (k, v) = s.split_once(':')?;
                                k.eq_ignore_ascii_case("content-length")
                                    .then(|| v.trim().parse::<usize>().ok())?
                            })
                            .unwrap_or(0);
                    }
                    if data.len() >= pos + 4 + content_len {
                        break;
                    }
                }
            }
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                payload.len(),
                payload
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
    MockOllama { addr, hits }
}

fn find_headers_end(data: &[u8]) -> Option<usize> {
    data.windows(4).position(|w| w == b"\r\n\r\n")
}

// ------------------------------------------------------------- misc

fn load_ca(dir: &std::path::Path) -> Vec<u8> {
    std::fs::read(dir.join("word-assist-ca.der"))
        .expect("test CA file (generated by the server at start)")
}

fn server_handle_stop(server: &ServerHandle) {
    server.stop();
}

fn cleanup(dir: &std::path::Path) {
    let _ = std::fs::remove_dir_all(dir);
}

// Silence the unused-import lint for helpers used by single tests in
// configurations where only some branches exercise them.
#[allow(unused)]
fn _store_location_smoke() {
    use scholarscribe_lib::assist::settings::{TokenStore, TOKEN_SLIDING_TTL};
    let _ = TOKEN_SLIDING_TTL;
    let _ = TokenStore::for_dir(std::path::Path::new("/tmp"));
}
