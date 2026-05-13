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
