use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use perro_ids::SignalID;
use perro_variant::Variant;

use crate::http::{
    HttpClient, HttpConfig, HttpErrorKind, HttpEvent, HttpID, HttpProxy, HttpQueueConfig,
    HttpResponse, HttpSubmitErrorKind, HttpTLSMode,
};

#[test]
fn http_event_maps_to_signal_name_id_and_params() {
    let event = HttpEvent::Completed(HttpResponse {
        id: HttpID(7),
        url: "http://localhost".to_string(),
        status: 200,
        headers: Vec::new(),
        body: b"ok".to_vec(),
    });

    assert_eq!(event.signal_name(), "HTTP_Completed");
    assert_eq!(event.signal_id(), SignalID::from_string("HTTP_Completed"));
    assert_eq!(
        event.signal_params(),
        vec![
            Variant::from(7_u32),
            Variant::from(200_u16),
            Variant::from("http://localhost".to_string()),
            Variant::from(b"ok".to_vec())
        ]
    );
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn http_get_loopback_completes() {
    let server = TestServer::start(|_| response(200, &[], b"hello"));
    let mut client = HttpClient::new();
    let id = client.get(server.url("/hello"));

    let event = wait_event(&mut client);
    server.join();

    match event {
        HttpEvent::Completed(resp) => {
            assert_eq!(resp.id, id);
            assert_eq!(resp.status, 200);
            assert_eq!(resp.text().expect("test setup must succeed"), "hello");
            assert!(resp.ok());
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn http_post_variant_sends_json_and_response_variant_reads_object() {
    let seen_body = Arc::new(Mutex::new(Vec::new()));
    let seen_body_for_server = Arc::clone(&seen_body);
    let server = TestServer::start(move |request| {
        *seen_body_for_server
            .lock()
            .expect("test setup must succeed") = request.body;
        response(
            200,
            &[("Content-Type", "application/json")],
            br#"{"ok":true}"#,
        )
    });

    let mut body = BTreeMap::new();
    body.insert("name".into(), Variant::from("perro"));
    let mut client = HttpClient::new();
    client.post_variant(server.url("/json"), Variant::from(body));

    let event = wait_event(&mut client);
    server.join();

    assert_eq!(
        String::from_utf8(seen_body.lock().expect("test setup must succeed").clone())
            .expect("test setup must succeed"),
        r#"{"name":"perro"}"#
    );

    let HttpEvent::Completed(resp) = event else {
        panic!("expected completed event");
    };
    let value = resp.variant().expect("test setup must succeed");
    assert_eq!(
        value
            .as_object()
            .expect("test setup must succeed")
            .get("ok")
            .expect("test setup must succeed"),
        &Variant::from(true)
    );
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn http_too_large_response_fails() {
    let server = TestServer::start(|_| response(200, &[], b"too big"));
    let mut client = HttpClient::with_config(HttpConfig::default().max_response_bytes(3));
    client.get(server.url("/large"));

    let event = wait_event(&mut client);
    server.join();

    let HttpEvent::Failed(err) = event else {
        panic!("expected failed event");
    };
    assert_eq!(err.kind, HttpErrorKind::TooLarge);
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn http_bad_endpoint_fails() {
    let mut client = HttpClient::with_config(HttpConfig::default().timeout_ms(100));
    client.get("http://127.0.0.1:9/nope");

    let event = wait_event(&mut client);

    let HttpEvent::Failed(err) = event else {
        panic!("expected failed event");
    };
    assert_eq!(err.kind, HttpErrorKind::Send);
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn cookie_enabled_client_keeps_cookie_between_requests() {
    let count = Arc::new(Mutex::new(0usize));
    let count_for_server = Arc::clone(&count);
    let server = TestServer::start_multi(2, move |request| {
        let mut count = count_for_server.lock().expect("test setup must succeed");
        *count += 1;
        if *count == 1 {
            response(200, &[("Set-Cookie", "sid=abc; Path=/")], b"set")
        } else {
            let has_cookie = request.headers.iter().any(|(name, value)| {
                name.eq_ignore_ascii_case("cookie") && value.contains("sid=abc")
            });
            response(200, &[], if has_cookie { b"cookie" } else { b"missing" })
        }
    });

    let mut client = HttpClient::with_config(HttpConfig::default().cookies_enabled(true));
    client.get(server.url("/set"));
    assert!(matches!(wait_event(&mut client), HttpEvent::Completed(_)));
    client.get(server.url("/check"));

    let event = wait_event(&mut client);
    server.join();

    let HttpEvent::Completed(resp) = event else {
        panic!("expected completed event");
    };
    assert_eq!(resp.text().expect("test setup must succeed"), "cookie");
}

#[test]
fn proxy_and_tls_config_builds_or_fails_cleanly() {
    let _ = HttpConfig::default()
        .proxy(HttpProxy::http("http://127.0.0.1:8080"))
        .tls_mode(HttpTLSMode::PlatformVerifier);
    let _ = HttpConfig::default()
        .proxy(HttpProxy::socks("socks5://127.0.0.1:1080"))
        .tls_mode(HttpTLSMode::NativeTls);
}

#[test]
fn tls_modes_select_distinct_provider_and_roots() {
    use ureq::tls::{RootCerts, TlsProvider};

    let rustls = crate::http::tls_config(&HttpTLSMode::DefaultRustls);
    assert_eq!(rustls.provider(), TlsProvider::Rustls);
    assert!(matches!(rustls.root_certs(), RootCerts::WebPki));

    let platform = crate::http::tls_config(&HttpTLSMode::PlatformVerifier);
    assert_eq!(platform.provider(), TlsProvider::Rustls);
    assert!(matches!(platform.root_certs(), RootCerts::PlatformVerifier));

    let native = crate::http::tls_config(&HttpTLSMode::NativeTls);
    assert_eq!(native.provider(), TlsProvider::NativeTls);
    assert!(matches!(native.root_certs(), RootCerts::PlatformVerifier));
}

#[test]
fn empty_url_emits_one_terminal_event() {
    let mut client = HttpClient::new();
    let id = client.get("");

    let event = wait_event(&mut client);
    let HttpEvent::Failed(err) = event else {
        panic!("expected failed event");
    };
    assert_eq!(err.id, id);
    assert_eq!(err.kind, HttpErrorKind::Send);

    thread::sleep(Duration::from_millis(20));
    assert!(client.poll_all(8).is_empty());
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn saturated_request_queue_rejects_without_block_and_keeps_one_terminal_per_id() {
    let (started_tx, started_rx) = mpsc::sync_channel(0);
    let (release_tx, release_rx) = mpsc::sync_channel(0);
    let mut request_index = 0usize;
    let server = TestServer::start_multi(2, move |_| {
        if request_index == 0 {
            started_tx.send(()).expect("signal first request");
            release_rx.recv().expect("release first request");
        }
        request_index += 1;
        response(200, &[], b"ok")
    });
    let queue = HttpQueueConfig::default()
        .worker_count(1)
        .request_capacity(1)
        .event_capacity(1);
    let mut client = HttpClient::with_config_and_queue(HttpConfig::default(), queue);

    let first = client.get(server.url("/first"));
    started_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("first request start");
    let second = client.get(server.url("/second"));
    let rejected = client.get(server.url("/rejected"));
    let direct_error = client
        .try_request(crate::http::HttpRequest::get(server.url("/direct")))
        .expect_err("direct backpressure");
    assert_eq!(direct_error.kind, HttpSubmitErrorKind::QueueFull);

    let HttpEvent::Failed(error) = wait_event(&mut client) else {
        panic!("expected queue-full event");
    };
    assert_eq!(error.id, rejected);
    assert_eq!(error.kind, HttpErrorKind::Send);
    assert_eq!(client.rejected_requests(), 2);
    release_tx.send(()).expect("release server");

    let mut terminal_counts = BTreeMap::from([(rejected.0, 1usize)]);
    for _ in 0..2 {
        let id = http_event_id(&wait_event(&mut client));
        *terminal_counts.entry(id.0).or_default() += 1;
    }
    server.join();

    assert_eq!(terminal_counts.get(&first.0), Some(&1));
    assert_eq!(terminal_counts.get(&second.0), Some(&1));
    assert_eq!(terminal_counts.get(&rejected.0), Some(&1));
    thread::sleep(Duration::from_millis(20));
    assert!(client.poll_all(8).is_empty());
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn http_worker_pool_runs_requests_concurrently_with_bounded_events() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind server");
    let addr = listener.local_addr().expect("server addr");
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let server_active = Arc::clone(&active);
    let server_peak = Arc::clone(&peak);
    let server = thread::spawn(move || {
        let mut handlers = Vec::new();
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept request");
            let active = Arc::clone(&server_active);
            let peak = Arc::clone(&server_peak);
            handlers.push(thread::spawn(move || {
                let _ = read_request(&mut stream);
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(current, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(75));
                stream
                    .write_all(&response(200, &[], b"ok"))
                    .expect("write response");
                active.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for handler in handlers {
            handler.join().expect("join handler");
        }
    });

    let queue = HttpQueueConfig::default()
        .worker_count(2)
        .request_capacity(2)
        .event_capacity(1);
    let mut client = HttpClient::with_config_and_queue(HttpConfig::default(), queue);
    let first = client.get(format!("http://{addr}/first"));
    let second = client.get(format!("http://{addr}/second"));
    let mut ids = [
        http_event_id(&wait_event(&mut client)),
        http_event_id(&wait_event(&mut client)),
    ];
    ids.sort_by_key(|id| id.0);
    server.join().expect("join server");

    assert_eq!(ids, [first, second]);
    assert!(peak.load(Ordering::SeqCst) >= 2);
    assert_eq!(client.queue_config(), queue);
}

fn http_event_id(event: &HttpEvent) -> HttpID {
    match event {
        HttpEvent::Completed(response) => response.id,
        HttpEvent::Failed(error) => error.id,
    }
}

fn wait_event(client: &mut HttpClient) -> HttpEvent {
    for _ in 0..200 {
        if let Some(event) = client.poll() {
            return event;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("timeout");
}

struct TestServer {
    addr: String,
    handle: thread::JoinHandle<()>,
}

impl TestServer {
    fn start(handler: impl FnOnce(TestRequest) -> Vec<u8> + Send + 'static) -> Self {
        let mut handler = Some(handler);
        Self::start_multi(1, move |request| {
            handler.take().expect("test setup must succeed")(request)
        })
    }

    fn start_multi(
        requests: usize,
        mut handler: impl FnMut(TestRequest) -> Vec<u8> + Send + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test setup must succeed");
        let addr = listener
            .local_addr()
            .expect("test setup must succeed")
            .to_string();
        let handle = thread::spawn(move || {
            for _ in 0..requests {
                let (mut stream, _) = listener.accept().expect("test setup must succeed");
                let request = read_request(&mut stream);
                let response = handler(request);
                stream
                    .write_all(&response)
                    .expect("test setup must succeed");
            }
        });
        Self { addr, handle }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    fn join(self) {
        self.handle.join().expect("test setup must succeed");
    }
}

#[derive(Debug)]
struct TestRequest {
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> TestRequest {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("test setup must succeed");
    let mut data = Vec::new();
    let mut buf = [0_u8; 512];
    let header_end;
    loop {
        let n = stream.read(&mut buf).expect("test setup must succeed");
        data.extend_from_slice(&buf[..n]);
        if let Some(i) = find_header_end(&data) {
            header_end = i;
            break;
        }
    }

    let header_text = String::from_utf8_lossy(&data[..header_end]);
    let headers = parse_headers(&header_text);
    let content_len = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    let body_start = header_end + 4;
    while data.len() < body_start + content_len {
        let n = stream.read(&mut buf).expect("test setup must succeed");
        data.extend_from_slice(&buf[..n]);
    }

    TestRequest {
        headers,
        body: data[body_start..body_start + content_len].to_vec(),
    }
}

fn parse_headers(text: &str) -> Vec<(String, String)> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

fn find_header_end(data: &[u8]) -> Option<usize> {
    data.windows(4).position(|w| w == b"\r\n\r\n")
}

fn response(status: u16, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let status_text = match status {
        200 => "OK",
        404 => "Not Found",
        _ => "OK",
    };
    let mut out = format!(
        "HTTP/1.1 {status} {status_text}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        out.push_str(name);
        out.push_str(": ");
        out.push_str(value);
        out.push_str("\r\n");
    }
    out.push_str("\r\n");
    let mut bytes = out.into_bytes();
    bytes.extend_from_slice(body);
    bytes
}
