use std::{
    collections::VecDeque,
    fmt,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use perro_ids::SignalID;
use perro_variant::Variant;
use serde_json::Value as JsonValue;

pub type HttpResult<T> = Result<T, HttpError>;
pub type HttpHeaders = Vec<(String, String)>;

const DEFAULT_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_MAX_RESPONSE_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HttpID(pub u32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HttpTLSMode {
    DefaultRustls,
    PlatformVerifier,
    NativeTls,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpProxy {
    pub url: String,
}

impl HttpProxy {
    pub fn http(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }

    pub fn socks(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpConfig {
    pub timeout_ms: u64,
    pub max_response_bytes: usize,
    pub cookies_enabled: bool,
    pub proxy: Option<HttpProxy>,
    pub tls_mode: HttpTLSMode,
}

impl HttpConfig {
    pub fn timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    pub fn max_response_bytes(mut self, max_response_bytes: usize) -> Self {
        self.max_response_bytes = max_response_bytes;
        self
    }

    pub fn cookies_enabled(mut self, cookies_enabled: bool) -> Self {
        self.cookies_enabled = cookies_enabled;
        self
    }

    pub fn proxy(mut self, proxy: HttpProxy) -> Self {
        self.proxy = Some(proxy);
        self
    }

    pub fn tls_mode(mut self, tls_mode: HttpTLSMode) -> Self {
        self.tls_mode = tls_mode;
        self
    }
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            timeout_ms: DEFAULT_TIMEOUT_MS,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            cookies_enabled: false,
            proxy: None,
            tls_mode: HttpTLSMode::DefaultRustls,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HttpBody {
    Empty,
    Bytes(Vec<u8>),
    Text(String),
    Variant(Variant),
}

#[derive(Clone, Debug, PartialEq)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: HttpHeaders,
    pub body: HttpBody,
    pub timeout_ms: Option<u64>,
    pub max_response_bytes: Option<usize>,
}

impl HttpRequest {
    pub fn get(url: impl Into<String>) -> Self {
        Self::new(HttpMethod::Get, url, HttpBody::Empty)
    }

    pub fn post_variant(url: impl Into<String>, body: Variant) -> Self {
        Self::new(HttpMethod::Post, url, HttpBody::Variant(body))
    }

    pub fn post_bytes(url: impl Into<String>, body: Vec<u8>) -> Self {
        Self::new(HttpMethod::Post, url, HttpBody::Bytes(body))
    }

    pub fn new(method: HttpMethod, url: impl Into<String>, body: HttpBody) -> Self {
        Self {
            method,
            url: url.into(),
            headers: Vec::new(),
            body,
            timeout_ms: None,
            max_response_bytes: None,
        }
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    pub fn max_response_bytes(mut self, max_response_bytes: usize) -> Self {
        self.max_response_bytes = Some(max_response_bytes);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpResponse {
    pub id: HttpID,
    pub url: String,
    pub status: u16,
    pub headers: HttpHeaders,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn bytes(&self) -> &[u8] {
        &self.body
    }

    pub fn text(&self) -> HttpResult<String> {
        String::from_utf8(self.body.clone()).map_err(|err| {
            HttpError::new(
                self.id,
                self.url.clone(),
                HttpErrorKind::Text,
                err.to_string(),
            )
        })
    }

    pub fn variant(&self) -> HttpResult<Variant> {
        let value: JsonValue = serde_json::from_slice(&self.body).map_err(|err| {
            HttpError::new(
                self.id,
                self.url.clone(),
                HttpErrorKind::Json,
                err.to_string(),
            )
        })?;
        Ok(Variant::from_json_value(value))
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn ok(&self) -> bool {
        (200..=299).contains(&self.status)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpErrorKind {
    QueueClosed,
    Send,
    Status,
    Read,
    TooLarge,
    Json,
    Text,
    InvalidHeader,
    Config,
}

impl HttpErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            HttpErrorKind::QueueClosed => "QueueClosed",
            HttpErrorKind::Send => "Send",
            HttpErrorKind::Status => "Status",
            HttpErrorKind::Read => "Read",
            HttpErrorKind::TooLarge => "TooLarge",
            HttpErrorKind::Json => "Json",
            HttpErrorKind::Text => "Text",
            HttpErrorKind::InvalidHeader => "InvalidHeader",
            HttpErrorKind::Config => "Config",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpError {
    pub id: HttpID,
    pub url: String,
    pub kind: HttpErrorKind,
    pub message: String,
}

impl HttpError {
    pub fn new(
        id: HttpID,
        url: impl Into<String>,
        kind: HttpErrorKind,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id,
            url: url.into(),
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} {}: {}", self.kind.as_str(), self.url, self.message)
    }
}

impl std::error::Error for HttpError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HttpEvent {
    Completed(HttpResponse),
    Failed(HttpError),
}

impl HttpEvent {
    pub fn signal_name(&self) -> &'static str {
        match self {
            HttpEvent::Completed(_) => "HTTP_Completed",
            HttpEvent::Failed(_) => "HTTP_Failed",
        }
    }

    pub fn signal_id(&self) -> SignalID {
        SignalID::from_string(self.signal_name())
    }

    pub fn signal_params(&self) -> Vec<Variant> {
        match self {
            HttpEvent::Completed(response) => vec![
                Variant::from(response.id.0),
                Variant::from(response.status),
                Variant::from(response.url.clone()),
                Variant::from(response.body.clone()),
            ],
            HttpEvent::Failed(error) => vec![
                Variant::from(error.id.0),
                Variant::from(error.url.clone()),
                Variant::from(error.kind.as_str()),
                Variant::from(error.message.clone()),
            ],
        }
    }
}

struct HttpWork {
    id: HttpID,
    request: HttpRequest,
}

pub struct HttpClient {
    config: HttpConfig,
    next_id: u32,
    tx: Sender<HttpWork>,
    rx: Receiver<HttpEvent>,
    local_events: VecDeque<HttpEvent>,
}

impl HttpClient {
    pub fn new() -> Self {
        Self::with_config(HttpConfig::default())
    }

    pub fn with_config(config: HttpConfig) -> Self {
        let (work_tx, work_rx) = mpsc::channel::<HttpWork>();
        let (event_tx, event_rx) = mpsc::channel::<HttpEvent>();
        let worker_config = config.clone();
        thread::spawn(move || http_worker(worker_config, work_rx, event_tx));

        Self {
            config,
            next_id: 0,
            tx: work_tx,
            rx: event_rx,
            local_events: VecDeque::new(),
        }
    }

    pub fn request(&mut self, request: HttpRequest) -> HttpID {
        let id = self.next_http_id();
        let url = request.url.clone();
        if let Err(err) = self.tx.send(HttpWork { id, request }) {
            self.local_events
                .push_back(HttpEvent::Failed(HttpError::new(
                    id,
                    err.0.request.url,
                    HttpErrorKind::QueueClosed,
                    "http worker queue closed",
                )));
        } else if url.is_empty() {
            self.local_events
                .push_back(HttpEvent::Failed(HttpError::new(
                    id,
                    url,
                    HttpErrorKind::Send,
                    "empty url",
                )));
        }
        id
    }

    pub fn get(&mut self, url: impl Into<String>) -> HttpID {
        self.request(HttpRequest::get(url))
    }

    pub fn post_variant(&mut self, url: impl Into<String>, body: Variant) -> HttpID {
        self.request(HttpRequest::post_variant(url, body))
    }

    pub fn put_variant(&mut self, url: impl Into<String>, body: Variant) -> HttpID {
        self.request(HttpRequest::new(
            HttpMethod::Put,
            url,
            HttpBody::Variant(body),
        ))
    }

    pub fn patch_variant(&mut self, url: impl Into<String>, body: Variant) -> HttpID {
        self.request(HttpRequest::new(
            HttpMethod::Patch,
            url,
            HttpBody::Variant(body),
        ))
    }

    pub fn post_bytes(&mut self, url: impl Into<String>, body: Vec<u8>) -> HttpID {
        self.request(HttpRequest::post_bytes(url, body))
    }

    pub fn poll(&mut self) -> Option<HttpEvent> {
        if let Some(event) = self.local_events.pop_front() {
            return Some(event);
        }
        self.rx.try_recv().ok()
    }

    pub fn poll_all(&mut self, max_events: usize) -> Vec<HttpEvent> {
        let mut out = Vec::new();
        for _ in 0..max_events {
            let Some(event) = self.poll() else {
                break;
            };
            out.push(event);
        }
        out
    }

    pub fn config(&self) -> &HttpConfig {
        &self.config
    }

    fn next_http_id(&mut self) -> HttpID {
        let id = HttpID(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        id
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

fn http_worker(config: HttpConfig, work_rx: Receiver<HttpWork>, event_tx: Sender<HttpEvent>) {
    let shared_agent = build_agent(&config).ok();
    for work in work_rx {
        let event = run_http_work(&config, shared_agent.as_ref(), work);
        if event_tx.send(event).is_err() {
            break;
        }
    }
}

fn run_http_work(
    config: &HttpConfig,
    shared_agent: Option<&ureq::Agent>,
    work: HttpWork,
) -> HttpEvent {
    let id = work.id;
    let request = work.request;
    let url = request.url.clone();

    let agent = if config.cookies_enabled {
        match shared_agent {
            Some(agent) => agent.clone(),
            None => {
                return HttpEvent::Failed(HttpError::new(
                    id,
                    url,
                    HttpErrorKind::Config,
                    "failed to create cookie-enabled http agent",
                ));
            }
        }
    } else {
        match build_agent(config) {
            Ok(agent) => agent,
            Err(err) => {
                return HttpEvent::Failed(HttpError::new(
                    id,
                    url,
                    HttpErrorKind::Config,
                    err.to_string(),
                ));
            }
        }
    };

    match send_request(&agent, config, id, request) {
        Ok(response) => {
            if (400..=599).contains(&response.status) {
                HttpEvent::Failed(HttpError::new(
                    id,
                    response.url,
                    HttpErrorKind::Status,
                    format!("http status {}", response.status),
                ))
            } else {
                HttpEvent::Completed(response)
            }
        }
        Err(err) => HttpEvent::Failed(err),
    }
}

fn build_agent(config: &HttpConfig) -> Result<ureq::Agent, ureq::Error> {
    let mut builder = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(config.timeout_ms)))
        .tls_config(tls_config(&config.tls_mode));

    if let Some(proxy) = &config.proxy {
        builder = builder.proxy(Some(ureq::Proxy::new(&proxy.url)?));
    }

    Ok(builder.build().new_agent())
}

fn tls_config(mode: &HttpTLSMode) -> ureq::tls::TlsConfig {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

    match mode {
        HttpTLSMode::DefaultRustls => TlsConfig::builder()
            .provider(TlsProvider::NativeTls)
            .root_certs(RootCerts::PlatformVerifier)
            .build(),
        HttpTLSMode::PlatformVerifier => TlsConfig::builder()
            .provider(TlsProvider::NativeTls)
            .root_certs(RootCerts::PlatformVerifier)
            .build(),
        HttpTLSMode::NativeTls => TlsConfig::builder()
            .provider(TlsProvider::NativeTls)
            .build(),
    }
}

fn send_request(
    agent: &ureq::Agent,
    config: &HttpConfig,
    id: HttpID,
    request: HttpRequest,
) -> HttpResult<HttpResponse> {
    let url = request.url.clone();
    let max_response_bytes = request
        .max_response_bytes
        .unwrap_or(config.max_response_bytes)
        .max(1);
    let timeout_ms = request.timeout_ms.unwrap_or(config.timeout_ms);

    let result = match request.method {
        HttpMethod::Get => {
            let builder = apply_headers(agent.get(&request.url), &request.headers);
            builder
                .config()
                .timeout_global(Some(Duration::from_millis(timeout_ms)))
                .build()
                .call()
        }
        HttpMethod::Delete => {
            let builder = apply_headers(agent.delete(&request.url), &request.headers);
            builder
                .config()
                .timeout_global(Some(Duration::from_millis(timeout_ms)))
                .build()
                .call()
        }
        HttpMethod::Head => {
            let builder = apply_headers(agent.head(&request.url), &request.headers);
            builder
                .config()
                .timeout_global(Some(Duration::from_millis(timeout_ms)))
                .build()
                .call()
        }
        HttpMethod::Post => {
            let (body, headers) = encode_body(request.body, request.headers, id, &url)?;
            let builder = apply_headers(agent.post(&url), &headers);
            builder
                .config()
                .timeout_global(Some(Duration::from_millis(timeout_ms)))
                .build()
                .send(body)
        }
        HttpMethod::Put => {
            let (body, headers) = encode_body(request.body, request.headers, id, &url)?;
            let builder = apply_headers(agent.put(&url), &headers);
            builder
                .config()
                .timeout_global(Some(Duration::from_millis(timeout_ms)))
                .build()
                .send(body)
        }
        HttpMethod::Patch => {
            let (body, headers) = encode_body(request.body, request.headers, id, &url)?;
            let builder = apply_headers(agent.patch(&url), &headers);
            builder
                .config()
                .timeout_global(Some(Duration::from_millis(timeout_ms)))
                .build()
                .send(body)
        }
    };

    let mut response = result.map_err(|err| map_ureq_error(id, &url, err))?;
    let status = response.status().as_u16();
    let headers = response_headers(&response);
    let body = response
        .body_mut()
        .with_config()
        .limit(max_response_bytes as u64)
        .read_to_vec()
        .map_err(|err| map_read_error(id, &url, err))?;

    Ok(HttpResponse {
        id,
        url,
        status,
        headers,
        body,
    })
}

fn apply_headers<B>(
    mut builder: ureq::RequestBuilder<B>,
    headers: &[(String, String)],
) -> ureq::RequestBuilder<B> {
    for (name, value) in headers {
        builder = builder.header(name.as_str(), value.as_str());
    }
    builder
}

fn encode_body(
    body: HttpBody,
    mut headers: HttpHeaders,
    id: HttpID,
    url: &str,
) -> HttpResult<(Vec<u8>, HttpHeaders)> {
    let bytes = match body {
        HttpBody::Empty => Vec::new(),
        HttpBody::Bytes(v) => v,
        HttpBody::Text(v) => v.into_bytes(),
        HttpBody::Variant(v) => {
            if !headers
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            {
                headers.push(("Content-Type".to_string(), "application/json".to_string()));
            }
            serde_json::to_vec(&v.to_json_value()).map_err(|err| {
                HttpError::new(id, url.to_string(), HttpErrorKind::Json, err.to_string())
            })?
        }
    };

    Ok((bytes, headers))
}

fn response_headers(response: &ureq::http::Response<ureq::Body>) -> HttpHeaders {
    response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|v| (name.as_str().to_string(), v.to_string()))
        })
        .collect()
}

fn map_ureq_error(id: HttpID, url: &str, err: ureq::Error) -> HttpError {
    match err {
        ureq::Error::StatusCode(status) => HttpError::new(
            id,
            url.to_string(),
            HttpErrorKind::Status,
            format!("http status {status}"),
        ),
        ureq::Error::Http(err) => HttpError::new(
            id,
            url.to_string(),
            HttpErrorKind::InvalidHeader,
            err.to_string(),
        ),
        other => HttpError::new(id, url.to_string(), HttpErrorKind::Send, other.to_string()),
    }
}

fn map_read_error(id: HttpID, url: &str, err: ureq::Error) -> HttpError {
    match err {
        ureq::Error::BodyExceedsLimit(_) => HttpError::new(
            id,
            url.to_string(),
            HttpErrorKind::TooLarge,
            err.to_string(),
        ),
        other => HttpError::new(id, url.to_string(), HttpErrorKind::Read, other.to_string()),
    }
}

#[macro_export]
macro_rules! emit_http_event {
    ($ctx:expr, $event:expr) => {{
        let event = $event;
        let params = event.signal_params();
        $ctx.Signals()
            .signal_emit(event.signal_id(), params.as_slice())
    }};
}

#[cfg(test)]
#[path = "../tests/unit/http_tests.rs"]
mod tests;
