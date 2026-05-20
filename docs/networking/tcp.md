# TCP

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

Use `TCP` when this feature, type group, file format, or workflow appears in game code or assets.

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

# TCP

TCP lives in `perro_api::networking`.

Use raw bytes for simple streams.

Use frames for message boundaries.

## Host

```rust
let host = TcpHost::bind("127.0.0.1:7777")?;

if let Some(mut connection) = host.accept()? {
    connection.write_frame(b"hello")?;
}
```

## Client

```rust
let mut connection = TcpConnection::connect("127.0.0.1:7777")?;
connection.write(b"hello")?;
```

## Frames

Frames use 4 byte big-endian length prefix.

```rust
connection.write_frame(b"hello")?;

if let Some(event) = connection.poll_frame_event(64 * 1024)? {
    match event {
        NetEvent::TcpFrame { peer, bytes } => println!("{peer}: {bytes:?}"),
        NetEvent::HeartbeatPing { .. } => connection.write_frame(heartbeat_pong())?,
        _ => {}
    }
}
```

Helpers:

- `encode_frame(bytes)`
- `decode_next_frame(buffer, max_frame_bytes)`

## Handshake

```rust
let handshake = NetHandshake::new("game", "match", 1);
connection.write_handshake(&handshake)?;
```

Read:

```rust
if let Some(remote) = connection.poll_handshake(4096)? {
    remote.validate(&handshake)?;
}
```
