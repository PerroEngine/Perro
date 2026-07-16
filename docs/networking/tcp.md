# TCP

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Practical Example | [Practical Example](#practical-example) |
| Reference | [Reference](#reference) |
| Host | [Host](#host) |
| Client | [Client](#client) |
| Frames | [Frames](#frames) |
| Handshake | [Handshake](#handshake) |

## Purpose

TCP gives an ordered, reliable byte stream: everything you send arrives, in
order, or the connection reports a disconnect. Use it when correctness matters
more than latency, such as chat, turn-based moves, or lobby state. Perro adds
length-prefixed framing so you can send whole messages instead of parsing a raw
stream yourself, plus a version handshake and heartbeat helpers.

## Use Cases

- Reliable match relay: send framed turn-based moves with `write_frame` and read
  them as `NetEvent::TcpFrame`.
- In-game chat: push each message as a frame; ordering and delivery are
  guaranteed.
- Version gate on connect: exchange `NetHandshake` and reject mismatched
  app/protocol/version before gameplay traffic.
- Keepalive on idle connections: send `heartbeat_ping()` and reply to
  `NetEvent::HeartbeatPing` with `heartbeat_pong()` so dead links are detected.
- Simple request/response services: use raw `write`/`read_available` when you do
  not need message boundaries.

## Practical Example

```rust
use std::cell::RefCell;

thread_local! {
    static LINK: RefCell<Option<TcpConnection>> = RefCell::new(None);
}

lifecycle!({
    fn on_init(&self, _ctx: &mut ScriptContext<'_, API>) {
        if let Ok(conn) = TcpConnection::connect("127.0.0.1:7777") {
            LINK.with(|link| *link.borrow_mut() = Some(conn));
        }
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        LINK.with(|link| {
            let mut link = link.borrow_mut();
            let Some(conn) = link.as_mut() else { return; };
            // Drain one framed message and forward it as a signal.
            if let Ok(Some(event)) = conn.poll_frame_event(64 * 1024) {
                match event {
                    NetEvent::HeartbeatPing { .. } => {
                        let _ = conn.write_frame(heartbeat_pong());
                    }
                    other => emit_net_event!(ctx.run, other),
                }
            }
        });
    }
});
```

## Reference

TCP lives in `perro_api::networking`. Use raw bytes for simple streams; use
frames for message boundaries.

## Host

```rust
let host = TcpHost::bind("127.0.0.1:7777")?;

if let Some(mut connection) = host.accept()? {
    connection.write_frame(b"hello")?;
}
```

`bind` is non-blocking; `accept` returns `None` when no client is waiting.

## Client

```rust
let mut connection = TcpConnection::connect("127.0.0.1:7777")?;
connection.write(b"hello")?;
```

## Frames

Frames use a 4-byte big-endian length prefix.

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

`max_frame_bytes` caps the largest accepted frame; oversized frames fail with a
`FrameTooLarge` error. A clean disconnect surfaces once as
`NetEvent::TcpDisconnected`.

Standalone helpers:

- `encode_frame(bytes)`
- `decode_next_frame(buffer, max_frame_bytes)`

## Handshake

```rust
let handshake = NetHandshake::new("game", "match", 1);
connection.write_handshake(&handshake)?;
```

Read and validate the remote handshake:

```rust
if let Some(remote) = connection.poll_handshake(4096)? {
    remote.validate(&handshake)?;
}
```

`validate` returns a `Handshake` error when the app, protocol, or version does
not match, so you can refuse incompatible builds before exchanging game data.
