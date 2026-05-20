# UDP

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

Use `UDP` when this feature, type group, file format, or workflow appears in game code or assets.

## Use Cases

Use the types, APIs, file formats, and workflows in this doc when the feature matches the game system you are building. Prefer `ctx.run` for runtime state, `ctx.res` for resource/data access, and `ctx.ipt` for input state.

## Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let _ = dt;
    }
});
```

## Reference

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
