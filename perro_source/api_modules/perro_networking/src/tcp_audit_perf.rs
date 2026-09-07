//! Baseline-compatible probes; no production implementation copied here.
use super::*;
use std::hint::black_box;
use std::time::Instant;

fn pair() -> (TcpConnection, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback bind");
    let client = TcpStream::connect(listener.local_addr().expect("address")).expect("connect");
    let (server, _) = listener.accept().expect("accept");
    (
        TcpConnection::from_stream(client).expect("connection"),
        server,
    )
}

#[test]
#[ignore = "manual release performance probe"]
fn audit_queue_append() {
    for count in [1_000, 10_000] {
        let mut samples = Vec::new();
        for _ in 0..15 {
            let (mut connection, _peer) = pair();
            let frames: Vec<_> = (0..count).map(|_| vec![7_u8; 32]).collect();
            let begin = Instant::now();
            for frame in frames {
                connection.enqueue_write(black_box(frame)).expect("enqueue");
            }
            samples.push(begin.elapsed().as_nanos());
            assert_eq!(connection.pending_write_bytes(), count * 32);
        }
        println!("AUDIT tcp_queue_append count={count} samples_ns={samples:?}");
    }
}

#[test]
#[ignore = "manual release performance probe"]
fn audit_idle_socket_polls() {
    let (mut connection, _peer) = pair();
    let udp = UdpEndpoint::bind("127.0.0.1:0").expect("UDP bind");
    for kind in ["tcp_raw", "tcp_frame", "udp"] {
        for max_bytes in [4_096, 65_536] {
            let mut samples = Vec::new();
            for _ in 0..15 {
                let begin = Instant::now();
                for _ in 0..10_000 {
                    match kind {
                        "tcp_raw" => assert!(
                            black_box(connection.poll_event(max_bytes))
                                .expect("poll")
                                .is_none()
                        ),
                        "tcp_frame" => assert!(
                            black_box(connection.poll_frame_event(max_bytes))
                                .expect("poll")
                                .is_none()
                        ),
                        _ => assert!(
                            black_box(udp.poll_event(max_bytes))
                                .expect("poll")
                                .is_none()
                        ),
                    }
                }
                samples.push(begin.elapsed().as_nanos());
            }
            println!(
                "AUDIT idle_socket kind={kind} max_bytes={max_bytes} operations=10000 samples_ns={samples:?}"
            );
        }
    }
}
