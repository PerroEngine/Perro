use super::*;
use std::{sync::mpsc, thread, time::Instant};

#[test]
fn world_remains_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<NetworkWorld>();
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn sparse_world_keeps_accept_and_packet_order_for_each_poll_mode() {
    for mode in 0..3 {
        let mut world = NetworkWorld::new();
        let hosts = (0..3)
            .map(|_| world.bind_tcp_host("127.0.0.1:0").expect("TCP host"))
            .collect::<Vec<_>>();
        let endpoints = (0..3)
            .map(|_| world.bind_udp("127.0.0.1:0").expect("UDP endpoint"))
            .collect::<Vec<_>>();
        assert!(world.remove_tcp_host(hosts[1]));
        assert!(world.remove_udp(endpoints[1]));

        // Queue reverse host/endpoint order. Poll still emits accepts first,
        // with ascending handle order inside each transport phase.
        let clients = [2, 0].map(|index| {
            TcpStream::connect(world.tcp_host_addr(hosts[index]).expect("host address"))
                .expect("TCP client")
        });
        let sender = UdpSocket::bind("127.0.0.1:0").expect("UDP sender");
        for index in [2, 0] {
            sender
                .send_to(
                    &[index as u8],
                    world.udp_addr(endpoints[index]).expect("UDP address"),
                )
                .expect("UDP send");
        }
        let events = match mode {
            0 => world.poll_events(1, 64),
            1 => world.poll_frame_events(1, 64),
            _ => world.poll_variant_events(1, 64),
        };
        assert_eq!(events.len(), 4, "poll mode {mode}: {events:?}");
        assert_eq!(
            events[0].source,
            NetSource::TcpConnection(TcpConnectionId(0))
        );
        assert_eq!(
            events[1].source,
            NetSource::TcpConnection(TcpConnectionId(1))
        );
        assert!(
            matches!(&events[0].event, NetEvent::TcpClientConnected { peer } if *peer == clients[1].local_addr().expect("client address").to_string())
        );
        assert!(
            matches!(&events[1].event, NetEvent::TcpClientConnected { peer } if *peer == clients[0].local_addr().expect("client address").to_string())
        );
        for (event, index) in events[2..].iter().zip([0, 2]) {
            assert_eq!(event.source, NetSource::UdpEndpoint(endpoints[index]));
            assert!(
                matches!(&event.event, NetEvent::UdpPacket { bytes, .. } if bytes == &[index as u8])
            );
        }
        assert_eq!(
            world.bind_udp("127.0.0.1:0").expect("reuse UDP"),
            endpoints[1]
        );
        assert_eq!(
            world.bind_tcp_host("127.0.0.1:0").expect("reuse host"),
            hosts[1]
        );
    }
}

#[test]
#[cfg_attr(
    not(feature = "network-tests"),
    ignore = "requires local socket access"
)]
fn websocket_reconnect_reactivates_removed_handle() {
    let host = WebSocketHost::bind("127.0.0.1:0").expect("WebSocket host");
    let url = format!("ws://{}", host.local_addr());
    let (stop, wait_for_stop) = mpsc::channel();
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut connections = Vec::new();
        while connections.len() < 3 {
            assert!(Instant::now() < deadline, "WebSocket accept timeout");
            if let Some(mut connection) = host.accept().expect("WebSocket accept") {
                // The client deliberately drops its first socket immediately.
                // Only greet sockets that remain alive for the assertion.
                if !connections.is_empty() {
                    connection
                        .send_text(format!("hello-{}", connections.len()))
                        .expect("WebSocket send");
                }
                connections.push(connection);
            } else {
                thread::yield_now();
            }
        }
        let _ = wait_for_stop.recv_timeout(Duration::from_secs(5));
    });
    let mut world = NetworkWorld::new();
    let id = world.connect_websocket(&url).expect("first connection");
    assert!(world.remove_websocket_connection(id));
    assert!(!world.remove_websocket_connection(id));
    world
        .reconnect_websocket(id, &url, WebSocketConnectOptions::new())
        .expect("reconnect removed handle");
    let next = world.connect_websocket(&url).expect("next connection");
    assert_eq!(
        next.0,
        id.0 + 1,
        "reconnect must reclaim its former free ID"
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if world.poll_events(8, 64).iter().any(|event| {
            event.source == NetSource::WebSocketConnection(id)
                && matches!(&event.event, NetEvent::WebSocketText { text, .. } if text == "hello-1")
        }) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "reconnected handle was not polled"
        );
        thread::yield_now();
    }
    stop.send(()).expect("stop server");
    server.join().expect("server thread");
}
