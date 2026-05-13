use std::{thread, time::Duration};

use perro_ids::SignalID;
use perro_variant::Variant;

use crate::net::{
    NetEvent, NetHandshake, NetSource, NetworkWorld, TcpConnection, TcpHost, UdpEndpoint,
    WebSocketConnection, WebSocketHost, decode_next_frame, encode_frame, heartbeat_ping,
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
fn udp_endpoint_sends_loopback_packet() {
    let a = UdpEndpoint::bind("127.0.0.1:0").unwrap();
    let b = UdpEndpoint::bind("127.0.0.1:0").unwrap();

    a.send_to(b"ping", b.local_addr()).unwrap();

    let packet = wait_for(|| b.recv_from(32).unwrap());
    assert_eq!(packet.bytes, b"ping");
    assert_eq!(packet.peer, a.local_addr().to_string());
}

#[test]
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

fn wait_for<T>(mut f: impl FnMut() -> Option<T>) -> T {
    for _ in 0..200 {
        if let Some(v) = f() {
            return v;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("timeout");
}
