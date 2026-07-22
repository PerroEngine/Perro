use std::{
    io::{Read, Write},
    net::TcpStream,
    thread,
    time::Duration,
};

use perro_ids::SignalID;
use perro_variant::Variant;

use crate::{
    MAX_TCP_PENDING_WRITE_BYTES, NetEvent, NetHandshake, NetSource, NetworkWorld, TcpConnection,
    TcpHost, UdpEndpoint, WEBSOCKET_HEARTBEAT_BYTES, WEBSOCKET_VARIANT_PROTOCOL,
    WebSocketAsyncConnection, WebSocketAsyncHost, WebSocketConnectOptions, WebSocketConnection,
    WebSocketHost, WebSocketHostOptions, decode_next_frame, encode_frame, heartbeat_ping,
};

#[test]
fn net_event_maps_to_signal_name_id_and_params() {
    let event = NetEvent::TcpData {
        peer: "127.0.0.1:10".to_string(),
        bytes: b"hello".to_vec(),
    };

    assert_eq!(event.signal_name(), "TCP_Data");
    assert_eq!(event.signal_id(), SignalID::from_string("TCP_Data"));
    assert_eq!(
        event.signal_params(),
        vec![
            Variant::from("127.0.0.1:10".to_string()),
            Variant::from(b"hello".to_vec())
        ]
    );
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn udp_endpoint_sends_loopback_packet() {
    let a = UdpEndpoint::bind("127.0.0.1:0").expect("test setup must succeed");
    let b = UdpEndpoint::bind("127.0.0.1:0").expect("test setup must succeed");

    a.send_to(b"ping", b.local_addr())
        .expect("test setup must succeed");

    let packet = wait_for(|| b.recv_from(32).expect("test setup must succeed"));
    assert_eq!(packet.bytes, b"ping");
    assert_eq!(packet.peer, a.local_addr().to_string());
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn tcp_host_accepts_and_reads_loopback_data() {
    let host = TcpHost::bind("127.0.0.1:0").expect("test setup must succeed");
    let addr = host.local_addr();

    let client = thread::spawn(move || {
        let mut client = TcpConnection::connect(addr).expect("test setup must succeed");
        client.write(b"hello").expect("test setup must succeed");
    });

    let mut server = wait_for(|| host.accept().expect("test setup must succeed"));
    let event = wait_for(|| server.poll_event(32).expect("test setup must succeed"));

    client.join().expect("test setup must succeed");
    assert_eq!(
        event,
        NetEvent::TcpData {
            peer: server.peer_string(),
            bytes: b"hello".to_vec()
        }
    );
}

#[test]
fn frame_codec_roundtrips_and_leaves_partial_data() {
    let mut bytes = encode_frame(b"one").expect("test setup must succeed");
    bytes.extend_from_slice(&encode_frame(b"two").expect("test setup must succeed"));
    bytes.extend_from_slice(&[0, 0]);

    assert_eq!(
        decode_next_frame(&mut bytes, 32).expect("test setup must succeed"),
        Some(b"one".to_vec())
    );
    assert_eq!(
        decode_next_frame(&mut bytes, 32).expect("test setup must succeed"),
        Some(b"two".to_vec())
    );
    assert_eq!(
        decode_next_frame(&mut bytes, 32).expect("test setup must succeed"),
        None
    );
    assert_eq!(bytes, vec![0, 0]);
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn tcp_frame_queue_accepts_multiple_valid_frames_and_reports_eof() {
    let host = TcpHost::bind("127.0.0.1:0").expect("test setup must succeed");
    let addr = host.local_addr();
    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(addr).expect("test setup must succeed");
        let mut bytes = encode_frame(b"12345678").expect("test setup must succeed");
        bytes.extend_from_slice(&encode_frame(b"abcdefgh").expect("test setup must succeed"));
        stream.write_all(&bytes).expect("test setup must succeed");
    });
    let mut server = wait_for(|| host.accept().expect("test setup must succeed"));

    let first = wait_for(|| server.poll_frame_event(8).expect("test setup must succeed"));
    let second = wait_for(|| server.poll_frame_event(8).expect("test setup must succeed"));
    let disconnected = wait_for(|| server.poll_frame_event(8).expect("test setup must succeed"));

    client.join().expect("test setup must succeed");
    assert!(matches!(
        first,
        NetEvent::TcpFrame { ref bytes, .. } if bytes == b"12345678"
    ));
    assert!(matches!(
        second,
        NetEvent::TcpFrame { ref bytes, .. } if bytes == b"abcdefgh"
    ));
    assert!(matches!(disconnected, NetEvent::TcpDisconnected { .. }));
    assert_eq!(
        server.poll_frame_event(8).expect("test setup must succeed"),
        None
    );
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn tcp_frame_queue_decodes_many_tiny_frames() {
    const FRAME_COUNT: usize = 10_000;

    let host = TcpHost::bind("127.0.0.1:0").expect("test setup must succeed");
    let addr = host.local_addr();
    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(addr).expect("test setup must succeed");
        stream
            .write_all(&vec![0_u8; FRAME_COUNT * 4])
            .expect("test setup must succeed");
    });
    let mut server = wait_for(|| host.accept().expect("test setup must succeed"));

    for _ in 0..FRAME_COUNT {
        assert_eq!(
            wait_for(|| server.poll_frame(0).expect("test setup must succeed")),
            Vec::<u8>::new()
        );
    }
    assert!(matches!(
        wait_for(|| server.poll_frame_event(0).expect("test setup must succeed")),
        NetEvent::TcpDisconnected { .. }
    ));
    client.join().expect("test setup must succeed");
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn tcp_frame_write_keeps_partial_nonblocking_send() {
    let host = TcpHost::bind("127.0.0.1:0").expect("test setup must succeed");
    let addr = host.local_addr();
    let payload = vec![0x5a; 8 * 1024 * 1024];
    let expected = payload.clone();
    let reader = thread::spawn(move || {
        let mut stream = TcpStream::connect(addr).expect("test setup must succeed");
        thread::sleep(Duration::from_millis(20));
        let mut header = [0_u8; 4];
        stream
            .read_exact(&mut header)
            .expect("test setup must succeed");
        let len = u32::from_be_bytes(header) as usize;
        let mut bytes = vec![0_u8; len];
        stream
            .read_exact(&mut bytes)
            .expect("test setup must succeed");
        bytes
    });
    let mut writer = wait_for(|| host.accept().expect("test setup must succeed"));

    writer
        .write_frame(&payload)
        .expect("test setup must succeed");
    for _ in 0..2_000 {
        writer.flush_pending().expect("test setup must succeed");
        if writer.pending_write_bytes() == 0 {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }

    assert_eq!(writer.pending_write_bytes(), 0);
    assert_eq!(reader.join().expect("test setup must succeed"), expected);
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn tcp_frame_write_applies_queue_backpressure() {
    let host = TcpHost::bind("127.0.0.1:0").expect("test setup must succeed");
    let addr = host.local_addr();
    let _client = TcpStream::connect(addr).expect("test setup must succeed");
    let mut writer = wait_for(|| host.accept().expect("test setup must succeed"));
    let payload = vec![0x5a; 8 * 1024 * 1024];

    let err = loop {
        if let Err(err) = writer.write_frame(&payload) {
            break err;
        }
    };

    assert!(err.to_string().contains("pending write queue exceeds max"));
    assert!(writer.pending_write_bytes() <= MAX_TCP_PENDING_WRITE_BYTES);
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn network_world_removes_framed_tcp_connection_after_eof() {
    let mut world = NetworkWorld::new();
    let host = world
        .bind_tcp_host("127.0.0.1:0")
        .expect("test setup must succeed");
    let addr = world.tcp_host_addr(host).expect("test setup must succeed");
    let client = TcpStream::connect(addr).expect("test setup must succeed");

    let id = wait_for(|| {
        world
            .poll_frame_events(8, 32)
            .into_iter()
            .find_map(|event| match (event.source, event.event) {
                (NetSource::TcpConnection(id), NetEvent::TcpClientConnected { .. }) => Some(id),
                _ => None,
            })
    });
    drop(client);
    wait_for(|| {
        world
            .poll_frame_events(8, 32)
            .into_iter()
            .any(|event| {
                event.source == NetSource::TcpConnection(id)
                    && matches!(event.event, NetEvent::TcpDisconnected { .. })
            })
            .then_some(())
    });

    assert!(world.tcp_send_frame(id, b"gone").is_err());
}

#[test]
fn handshake_roundtrips_and_validates() {
    let handshake = NetHandshake::new("perro_game", "match", 7);
    let decoded = NetHandshake::decode(&handshake.encode().expect("test setup must succeed"))
        .expect("test setup must succeed");

    assert_eq!(decoded, handshake);
    decoded
        .validate(&handshake)
        .expect("test setup must succeed");
    assert!(
        decoded
            .validate(&NetHandshake::new("other", "match", 7))
            .is_err()
    );
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn tcp_frame_event_reads_loopback_frame_and_heartbeat() {
    let host = TcpHost::bind("127.0.0.1:0").expect("test setup must succeed");
    let addr = host.local_addr();

    let client = thread::spawn(move || {
        let mut client = TcpConnection::connect(addr).expect("test setup must succeed");
        client
            .write_frame(b"hello")
            .expect("test setup must succeed");
        client
            .write_frame(heartbeat_ping())
            .expect("test setup must succeed");
    });

    let mut server = wait_for(|| host.accept().expect("test setup must succeed"));
    let frame = wait_for(|| {
        server
            .poll_frame_event(32)
            .expect("test setup must succeed")
    });
    let heartbeat = wait_for(|| {
        server
            .poll_frame_event(32)
            .expect("test setup must succeed")
    });

    client.join().expect("test setup must succeed");
    assert_eq!(
        frame,
        NetEvent::TcpFrame {
            peer: server.peer_string(),
            bytes: b"hello".to_vec()
        }
    );
    assert_eq!(
        heartbeat,
        NetEvent::HeartbeatPing {
            peer: server.peer_string()
        }
    );
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn network_world_accepts_tcp_and_polls_udp() {
    let mut world = NetworkWorld::new();
    let host = world
        .bind_tcp_host("127.0.0.1:0")
        .expect("test setup must succeed");
    let addr = world.tcp_host_addr(host).expect("test setup must succeed");
    let udp_a = world
        .bind_udp("127.0.0.1:0")
        .expect("test setup must succeed");
    let udp_b = world
        .bind_udp("127.0.0.1:0")
        .expect("test setup must succeed");
    let udp_b_addr = world.udp_addr(udp_b).expect("test setup must succeed");

    let client = thread::spawn(move || {
        let mut client = TcpConnection::connect(addr).expect("test setup must succeed");
        client.write(b"world").expect("test setup must succeed");
    });
    world
        .udp_send_to(udp_a, b"ping", udp_b_addr)
        .expect("test setup must succeed");

    let mut seen_tcp = false;
    let mut seen_udp = false;
    let mut seen_tcp_source = false;
    wait_for(|| {
        let events = world.poll_events(8, 32);
        seen_tcp |= events
            .iter()
            .any(|event| matches!(event.event, NetEvent::TcpData { .. }));
        seen_udp |= events.iter().any(|event| {
            matches!(
                event.event,
                NetEvent::UdpPacket { ref bytes, .. } if bytes == b"ping"
            )
        });
        seen_tcp_source |= events
            .iter()
            .any(|event| matches!(event.source, NetSource::TcpConnection(_)));
        (seen_tcp && seen_udp).then_some(())
    });

    client.join().expect("test setup must succeed");
    assert!(seen_tcp_source);
    assert!(seen_tcp);
    assert!(seen_udp);
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn websocket_host_accepts_loopback_text_and_binary() {
    let host = WebSocketHost::bind("127.0.0.1:0").expect("test setup must succeed");
    let url = format!("ws://{}", host.local_addr());

    let client = thread::spawn(move || {
        let mut client = WebSocketConnection::connect(url).expect("test setup must succeed");
        client.send_text("hello").expect("test setup must succeed");
        client
            .send_binary(b"bytes".to_vec())
            .expect("test setup must succeed");
    });

    let mut server = wait_for(|| host.accept().expect("test setup must succeed"));
    let text = wait_for(|| server.poll_event(64).expect("test setup must succeed"));
    let binary = wait_for(|| server.poll_event(64).expect("test setup must succeed"));

    client.join().expect("test setup must succeed");
    assert_eq!(
        text,
        NetEvent::WebSocketText {
            peer: server.peer_string(),
            text: "hello".to_string()
        }
    );
    assert_eq!(
        binary,
        NetEvent::WebSocketBinary {
            peer: server.peer_string(),
            bytes: b"bytes".to_vec()
        }
    );
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn websocket_variant_sends_json_text_and_polls_variant_event() {
    let host = WebSocketHost::bind("127.0.0.1:0").expect("test setup must succeed");
    let url = format!("ws://{}", host.local_addr());

    let client = thread::spawn(move || {
        let mut client = WebSocketConnection::connect(url).expect("test setup must succeed");
        let mut body = std::collections::BTreeMap::new();
        body.insert("name".into(), Variant::from("perro"));
        body.insert("ok".into(), Variant::from(true));
        client
            .send_variant(&Variant::from(body))
            .expect("test setup must succeed");
    });

    let mut server = wait_for(|| host.accept().expect("test setup must succeed"));
    let event = wait_for(|| {
        server
            .poll_variant_event(128)
            .expect("test setup must succeed")
    });

    client.join().expect("test setup must succeed");
    let NetEvent::WebSocketVariant { peer, value } = event else {
        panic!("expected websocket variant event");
    };
    assert_eq!(peer, server.peer_string());
    let object = value.as_object().expect("test setup must succeed");
    assert_eq!(
        object.get("name").expect("test setup must succeed"),
        &Variant::from("perro")
    );
    assert_eq!(
        object.get("ok").expect("test setup must succeed"),
        &Variant::from(true)
    );
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn websocket_compressed_text_and_binary_roundtrip() {
    let host = WebSocketHost::bind("127.0.0.1:0").expect("test setup must succeed");
    let url = format!("ws://{}", host.local_addr());

    let client = thread::spawn(move || {
        let mut client = WebSocketConnection::connect(url).expect("test setup must succeed");
        client
            .send_compressed_text("hello hello hello hello")
            .expect("test setup must succeed");
        client
            .send_compressed_binary(b"bytes bytes bytes bytes".to_vec())
            .expect("test setup must succeed");
    });

    let mut server = wait_for(|| host.accept().expect("test setup must succeed"));
    let text = wait_for(|| server.poll_event(128).expect("test setup must succeed"));
    let binary = wait_for(|| server.poll_event(128).expect("test setup must succeed"));

    client.join().expect("test setup must succeed");
    assert_eq!(
        text,
        NetEvent::WebSocketText {
            peer: server.peer_string(),
            text: "hello hello hello hello".to_string()
        }
    );
    assert_eq!(
        binary,
        NetEvent::WebSocketBinary {
            peer: server.peer_string(),
            bytes: b"bytes bytes bytes bytes".to_vec()
        }
    );
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn websocket_async_facade_connects_and_sends_compressed_text() {
    let runtime = tokio::runtime::Runtime::new().expect("test setup must succeed");

    runtime.block_on(async {
        let host = WebSocketAsyncHost::bind("127.0.0.1:0")
            .await
            .expect("test setup must succeed");
        let url = format!("ws://{}", host.local_addr());

        let host_for_accept = host.clone();
        let client_task = tokio::spawn(WebSocketAsyncConnection::connect(url));
        let server_task = tokio::spawn(wait_for_async(move || {
            let host = host_for_accept.clone();
            async move { host.accept().await.expect("test setup must succeed") }
        }));
        let client = client_task
            .await
            .expect("test setup must succeed")
            .expect("test setup must succeed");
        let server = server_task.await.expect("test setup must succeed");
        client
            .send_compressed_text("async hello")
            .await
            .expect("test setup must succeed");

        let event = wait_for_async(|| {
            let server = server.clone();
            async move {
                server
                    .poll_event(128)
                    .await
                    .expect("test setup must succeed")
            }
        })
        .await;

        let NetEvent::WebSocketText { text, .. } = event else {
            panic!("expected websocket text event");
        };
        assert_eq!(text, "async hello");
    });
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn websocket_options_select_subprotocol_and_poll_invalid_json() {
    let host = WebSocketHost::bind_with_options(
        "127.0.0.1:0",
        WebSocketHostOptions::new()
            .variant_protocol()
            .require_subprotocol(true)
            .max_message_bytes(256),
    )
    .expect("test setup must succeed");
    let url = format!("ws://{}", host.local_addr());

    let client = thread::spawn(move || {
        let mut client = WebSocketConnection::connect_with_options(
            url,
            WebSocketConnectOptions::new()
                .header("X-Perro-Test", "ok")
                .variant_protocol()
                .max_message_bytes(256),
        )
        .expect("test setup must succeed");
        assert_eq!(
            client.selected_subprotocol(),
            Some(WEBSOCKET_VARIANT_PROTOCOL)
        );
        client
            .send_text("{bad json")
            .expect("test setup must succeed");
        client
            .send_heartbeat_ping()
            .expect("test setup must succeed");
    });

    let mut server = wait_for(|| host.accept().expect("test setup must succeed"));
    assert_eq!(
        server.selected_subprotocol(),
        Some(WEBSOCKET_VARIANT_PROTOCOL)
    );
    let invalid = wait_for(|| {
        server
            .poll_variant_event_default()
            .expect("test setup must succeed")
    });
    let ping = wait_for(|| {
        server
            .poll_event_default()
            .expect("test setup must succeed")
    });

    client.join().expect("test setup must succeed");
    assert!(matches!(invalid, NetEvent::WebSocketInvalidJson { .. }));
    assert_eq!(
        ping,
        NetEvent::WebSocketPing {
            peer: server.peer_string(),
            bytes: WEBSOCKET_HEARTBEAT_BYTES.to_vec()
        }
    );
}

fn wait_for<T>(mut f: impl FnMut() -> Option<T>) -> T {
    for _ in 0..200 {
        if let Some(v) = f() {
            return v;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("timeout");
}

async fn wait_for_async<T, F, Fut>(mut f: F) -> T
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    for _ in 0..200 {
        if let Some(v) = f().await {
            return v;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("timeout");
}
