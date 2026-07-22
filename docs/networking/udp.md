# UDP

> Native boundary: this Perro networking API runs on native builds. WASM builds
> do not expose raw UDP sockets.

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Practical Example | [Practical Example](#practical-example) |
| Reference | [Reference](#reference) |
| Endpoint | [Endpoint](#endpoint) |
| NetworkWorld | [NetworkWorld](#networkworld) |

## Purpose

UDP sends small, connectionless datagrams with no delivery or ordering
guarantees. That is exactly what you want for fast-changing state where a lost
packet is stale a frame later anyway: the next update supersedes it, so
retransmitting would only add latency. Use it for real-time position and input
sync, and keep each packet small.

## Use Cases

- Fast position sync: broadcast player transforms every frame where a dropped
  packet is fine because the next one replaces it.
- Client input datagrams: send a compact input snapshot per tick with `send_to`.
- Unreliable world snapshots: a host sends state to all peers via a
  `NetworkWorld` UDP endpoint, dropping stragglers instead of stalling.
- Discovery / heartbeat pings: cheap fire-and-forget probes on a known port.
- Latency-first custom netcode: build your own reliability layer only over the
  packets that actually need it, leaving the rest lossy.

## Why UDP Here

Choose UDP when the next packet supersedes a lost one. Add sequence numbers,
validation, rate limits, and any reliability the mechanic needs. Use TCP or the
multiplayer session layer when the game cannot tolerate loss or reordering.

## Practical Example

```rust
use std::cell::RefCell;

thread_local! {
    static NET: RefCell<Option<NetworkWorld>> = RefCell::new(None);
}

lifecycle!({
    fn on_init(&self, _ctx: &mut ScriptContext<'_, API>) {
        let mut world = NetworkWorld::new();
        if world.bind_udp("127.0.0.1:7777").is_ok() {
            NET.with(|net| *net.borrow_mut() = Some(world));
        }
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        NET.with(|net| {
            let mut net = net.borrow_mut();
            let Some(world) = net.as_mut() else { return; };
            // Drain up to 16 packets per socket, 1200 bytes each.
            for net_event in world.poll_events(16, 1200) {
                if matches!(net_event.event, NetEvent::UdpPacket { .. }) {
                    emit_net_event!(ctx.run, net_event.event);
                }
            }
        });
    }
});
```

## Reference

UDP lives in `perro_api::networking`. Use it for small unreliable packets.

## Endpoint

```rust
let a = UdpEndpoint::bind("127.0.0.1:0")?;
let b = UdpEndpoint::bind("127.0.0.1:0")?;

a.send_to(b"ping", b.local_addr())?;
```

Receive with `recv_from`, which returns `None` when no packet is waiting:

```rust
if let Some(packet) = b.recv_from(1200)? {
    println!("{} {:?}", packet.peer, packet.bytes);
}
```

`max_bytes` bounds the read buffer; datagrams larger than it are truncated.

## NetworkWorld

`NetworkWorld` manages endpoints behind id handles and polls them together:

```rust
let mut world = NetworkWorld::new();
let endpoint = world.bind_udp("127.0.0.1:7777")?;

for event in world.poll_events(16, 1200) {
    if let NetEvent::UdpPacket { peer, bytes } = event.event {
        println!("{peer}: {bytes:?}");
    }
}
```

`poll_events(max_per_socket, max_bytes)` returns `NetworkEvent` values; read the
transport event from `event.event`. Send with `world.udp_send_to(endpoint,
bytes, addr)`.
