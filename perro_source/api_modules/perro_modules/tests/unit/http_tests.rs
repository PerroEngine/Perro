use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use perro_ids::SignalID;
use perro_variant::Variant;

use crate::http::{
    HttpClient, HttpConfig, HttpErrorKind, HttpEvent, HttpID, HttpProxy, HttpResponse, HttpTLSMode,
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
            assert_eq!(resp.text().unwrap(), "hello");
            assert!(resp.ok());
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn http_post_variant_sends_json_and_response_variant_reads_object() {
    let seen_body = Arc::new(Mutex::new(Vec::new()));
    let seen_body_for_server = Arc::clone(&seen_body);
    let server = TestServer::start(move |request| {
        *seen_body_for_server.lock().unwrap() = request.body;
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
        String::from_utf8(seen_body.lock().unwrap().clone()).unwrap(),
        r#"{"name":"perro"}"#
    );

    let HttpEvent::Completed(resp) = event else {
        panic!("expected completed event");
    };
    let value = resp.variant().unwrap();
    assert_eq!(
        value.as_object().unwrap().get("ok").unwrap(),
        &Variant::from(true)
    );
}

#[test]
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
fn cookie_enabled_client_keeps_cookie_between_requests() {
    let count = Arc::new(Mutex::new(0usize));
    let count_for_server = Arc::clone(&count);
    let server = TestServer::start_multi(2, move |request| {
        let mut count = count_for_server.lock().unwrap();
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
    assert_eq!(resp.text().unwrap(), "cookie");
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
        Self::start_multi(1, move |request| handler.take().unwrap()(request))
    }

    fn start_multi(
        requests: usize,
        mut handler: impl FnMut(TestRequest) -> Vec<u8> + Send + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let handle = thread::spawn(move || {
            for _ in 0..requests {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_request(&mut stream);
                let response = handler(request);
                stream.write_all(&response).unwrap();
            }
        });
        Self { addr, handle }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    fn join(self) {
        self.handle.join().unwrap();
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
        .unwrap();
    let mut data = Vec::new();
    let mut buf = [0_u8; 512];
    let header_end;
    loop {
        let n = stream.read(&mut buf).unwrap();
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
        let n = stream.read(&mut buf).unwrap();
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
