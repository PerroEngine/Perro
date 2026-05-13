# UDP

UDP lives in `perro_api::networking`.

Use for small unreliable packets.

## Endpoint

```rust
let a = UdpEndpoint::bind("127.0.0.1:0")?;
let b = UdpEndpoint::bind("127.0.0.1:0")?;

a.send_to(b"ping", b.local_addr())?;
```

Poll:

```rust
if let Some(packet) = b.recv_from(1200)? {
    println!("{} {:?}", packet.peer, packet.bytes);
}
```

## NetworkWorld

```rust
let mut world = NetworkWorld::new();
let endpoint = world.bind_udp("127.0.0.1:7777")?;

for event in world.poll_events(16, 1200) {
    if let NetEvent::UdpPacket { peer, bytes } = event.event {
        println!("{peer}: {bytes:?}");
    }
}
```
