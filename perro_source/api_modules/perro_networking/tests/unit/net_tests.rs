use std::{thread, time::Duration};

use perro_ids::SignalID;
use perro_variant::Variant;

use crate::{
    NetEvent, NetHandshake, NetSource, NetworkWorld, TcpConnection, TcpHost, UdpEndpoint,
    WEBSOCKET_HEARTBEAT_BYTES, WEBSOCKET_VARIANT_PROTOCOL, WebRtcIceCandidate, WebRtcPeer,
    WebRtcPeerConfig, WebRtcSignal, WebSocketAsyncConnection, WebSocketAsyncHost,
    WebSocketConnectOptions, WebSocketConnection, WebSocketHost, WebSocketHostOptions,
    decode_next_frame, encode_frame, heartbeat_ping,
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
    let a = UdpEndpoint::bind("127.0.0.1:0").unwrap();
    let b = UdpEndpoint::bind("127.0.0.1:0").unwrap();

    a.send_to(b"ping", b.local_addr()).unwrap();

    let packet = wait_for(|| b.recv_from(32).unwrap());
    assert_eq!(packet.bytes, b"ping");
    assert_eq!(packet.peer, a.local_addr().to_string());
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn tcp_host_accepts_and_reads_loopback_data() {
    let host = TcpHost::bind("127.0.0.1:0").unwrap();
    let addr = host.local_addr();

    let client = thread::spawn(move || {
        let mut client = TcpConnection::connect(addr).unwrap();
        client.write(b"hello").unwrap();
    });

    let mut server = wait_for(|| host.accept().unwrap());
    let event = wait_for(|| server.poll_event(32).unwrap());

    client.join().unwrap();
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
    let mut bytes = encode_frame(b"one").unwrap();
    bytes.extend_from_slice(&encode_frame(b"two").unwrap());
    bytes.extend_from_slice(&[0, 0]);

    assert_eq!(
        decode_next_frame(&mut bytes, 32).unwrap(),
        Some(b"one".to_vec())
    );
    assert_eq!(
        decode_next_frame(&mut bytes, 32).unwrap(),
        Some(b"two".to_vec())
    );
    assert_eq!(decode_next_frame(&mut bytes, 32).unwrap(), None);
    assert_eq!(bytes, vec![0, 0]);
}

#[test]
fn handshake_roundtrips_and_validates() {
    let handshake = NetHandshake::new("perro_game", "match", 7);
    let decoded = NetHandshake::decode(&handshake.encode().unwrap()).unwrap();

    assert_eq!(decoded, handshake);
    decoded.validate(&handshake).unwrap();
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
    let host = TcpHost::bind("127.0.0.1:0").unwrap();
    let addr = host.local_addr();

    let client = thread::spawn(move || {
        let mut client = TcpConnection::connect(addr).unwrap();
        client.write_frame(b"hello").unwrap();
        client.write_frame(heartbeat_ping()).unwrap();
    });

    let mut server = wait_for(|| host.accept().unwrap());
    let frame = wait_for(|| server.poll_frame_event(32).unwrap());
    let heartbeat = wait_for(|| server.poll_frame_event(32).unwrap());

    client.join().unwrap();
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
    let host = world.bind_tcp_host("127.0.0.1:0").unwrap();
    let addr = world.tcp_host_addr(host).unwrap();
    let udp_a = world.bind_udp("127.0.0.1:0").unwrap();
    let udp_b = world.bind_udp("127.0.0.1:0").unwrap();
    let udp_b_addr = world.udp_addr(udp_b).unwrap();

    let client = thread::spawn(move || {
        let mut client = TcpConnection::connect(addr).unwrap();
        client.write(b"world").unwrap();
    });
    world.udp_send_to(udp_a, b"ping", udp_b_addr).unwrap();

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

    client.join().unwrap();
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
    let host = WebSocketHost::bind("127.0.0.1:0").unwrap();
    let url = format!("ws://{}", host.local_addr());

    let client = thread::spawn(move || {
        let mut client = WebSocketConnection::connect(url).unwrap();
        client.send_text("hello").unwrap();
        client.send_binary(b"bytes".to_vec()).unwrap();
    });

    let mut server = wait_for(|| host.accept().unwrap());
    let text = wait_for(|| server.poll_event(64).unwrap());
    let binary = wait_for(|| server.poll_event(64).unwrap());

    client.join().unwrap();
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
    let host = WebSocketHost::bind("127.0.0.1:0").unwrap();
    let url = format!("ws://{}", host.local_addr());

    let client = thread::spawn(move || {
        let mut client = WebSocketConnection::connect(url).unwrap();
        let mut body = std::collections::BTreeMap::new();
        body.insert("name".into(), Variant::from("perro"));
        body.insert("ok".into(), Variant::from(true));
        client.send_variant(&Variant::from(body)).unwrap();
    });

    let mut server = wait_for(|| host.accept().unwrap());
    let event = wait_for(|| server.poll_variant_event(128).unwrap());

    client.join().unwrap();
    let NetEvent::WebSocketVariant { peer, value } = event else {
        panic!("expected websocket variant event");
    };
    assert_eq!(peer, server.peer_string());
    let object = value.as_object().unwrap();
    assert_eq!(object.get("name").unwrap(), &Variant::from("perro"));
    assert_eq!(object.get("ok").unwrap(), &Variant::from(true));
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn websocket_compressed_text_and_binary_roundtrip() {
    let host = WebSocketHost::bind("127.0.0.1:0").unwrap();
    let url = format!("ws://{}", host.local_addr());

    let client = thread::spawn(move || {
        let mut client = WebSocketConnection::connect(url).unwrap();
        client
            .send_compressed_text("hello hello hello hello")
            .unwrap();
        client
            .send_compressed_binary(b"bytes bytes bytes bytes".to_vec())
            .unwrap();
    });

    let mut server = wait_for(|| host.accept().unwrap());
    let text = wait_for(|| server.poll_event(128).unwrap());
    let binary = wait_for(|| server.poll_event(128).unwrap());

    client.join().unwrap();
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
    let runtime = tokio::runtime::Runtime::new().unwrap();

    runtime.block_on(async {
        let host = WebSocketAsyncHost::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", host.local_addr());

        let host_for_accept = host.clone();
        let (client, server) = tokio::join!(
            WebSocketAsyncConnection::connect(url),
            wait_for_async(|| {
                let host = host_for_accept.clone();
                async move { host.accept().await.unwrap() }
            })
        );
        let client = client.unwrap();
        let server = server;
        client.send_compressed_text("async hello").await.unwrap();

        let event = wait_for_async(|| {
            let server = server.clone();
            async move { server.poll_event(128).await.unwrap() }
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
    .unwrap();
    let url = format!("ws://{}", host.local_addr());

    let client = thread::spawn(move || {
        let mut client = WebSocketConnection::connect_with_options(
            url,
            WebSocketConnectOptions::new()
                .header("X-Perro-Test", "ok")
                .variant_protocol()
                .max_message_bytes(256),
        )
        .unwrap();
        assert_eq!(
            client.selected_subprotocol(),
            Some(WEBSOCKET_VARIANT_PROTOCOL)
        );
        client.send_text("{bad json").unwrap();
        client.send_heartbeat_ping().unwrap();
    });

    let mut server = wait_for(|| host.accept().unwrap());
    assert_eq!(
        server.selected_subprotocol(),
        Some(WEBSOCKET_VARIANT_PROTOCOL)
    );
    let invalid = wait_for(|| server.poll_variant_event_default().unwrap());
    let ping = wait_for(|| server.poll_event_default().unwrap());

    client.join().unwrap();
    assert!(matches!(invalid, NetEvent::WebSocketInvalidJson { .. }));
    assert_eq!(
        ping,
        NetEvent::WebSocketPing {
            peer: server.peer_string(),
            bytes: WEBSOCKET_HEARTBEAT_BYTES.to_vec()
        }
    );
}

#[test]
fn webrtc_signal_roundtrips_offer_answer_and_ice() {
    let offer = WebRtcSignal::offer("v=0\r\ns=perro\r\n");
    let answer = WebRtcSignal::answer("v=0\r\ns=answer\r\n");
    let ice = WebRtcSignal::ice_candidate(
        WebRtcIceCandidate::new("candidate:1 1 udp 1 127.0.0.1 7777 typ host")
            .with_sdp_mid("0")
            .with_sdp_mline_index(0)
            .with_username_fragment("ufrag"),
    );

    assert_eq!(
        WebRtcSignal::from_json_str(&offer.to_json_string().unwrap()).unwrap(),
        offer
    );
    assert_eq!(
        WebRtcSignal::from_json_str(&answer.to_json_string().unwrap()).unwrap(),
        answer
    );
    assert_eq!(
        WebRtcSignal::from_json_str(&ice.to_json_string().unwrap()).unwrap(),
        ice
    );
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn websocket_sends_webrtc_signal_and_polls_event() {
    let host = WebSocketHost::bind("127.0.0.1:0").unwrap();
    let url = format!("ws://{}", host.local_addr());

    let client = thread::spawn(move || {
        let mut client = WebSocketConnection::connect(url).unwrap();
        client
            .send_webrtc_signal(&WebRtcSignal::offer("v=0\r\ns=perro\r\n"))
            .unwrap();
    });

    let mut server = wait_for(|| host.accept().unwrap());
    let event = wait_for(|| server.poll_webrtc_signal_event(256).unwrap());

    client.join().unwrap();
    assert_eq!(
        event,
        NetEvent::WebRtcOffer {
            peer: server.peer_string(),
            sdp: "v=0\r\ns=perro\r\n".to_string()
        }
    );
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn webrtc_peer_wraps_crate_peer_connection() {
    let mut peer = WebRtcPeer::new(WebRtcPeerConfig::new()).unwrap();
    let channel = peer.create_data_channel("game").unwrap();
    let offer = peer.create_offer().unwrap();

    assert_eq!(channel.0, 0);
    assert!(matches!(offer, WebRtcSignal::Offer { .. }));
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
