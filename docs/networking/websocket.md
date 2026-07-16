# WebSocket

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Practical Example | [Practical Example](#practical-example) |
| Reference | [Reference](#reference) |
| Variant Protocol | [Variant Protocol](#variant-protocol) |
| Browser Shape | [Browser Shape](#browser-shape) |
| Events | [Events](#events) |
| Limits | [Limits](#limits) |
| Compression | [Compression](#compression) |
| Async | [Async](#async) |
| Heartbeat | [Heartbeat](#heartbeat) |
| Close | [Close](#close) |
| Reconnect Backoff | [Reconnect Backoff](#reconnect-backoff) |

## Purpose

WebSocket is a message-oriented connection that browsers can open directly, so it
is the bridge between a Perro game or server and web clients and tooling. It
carries text, binary, or `Variant`/JSON messages over one long-lived connection,
with optional compression, heartbeats, and reconnect backoff built in. Reach for
it when the other end is a browser, a live dev tool, or any JSON message stream.

## Use Cases

- Browser client: a web page connects with plain JSON while the game speaks the
  `perro.variant.v1` protocol on the same socket.
- Live dev tools / remote tuning: stream `Variant` messages between the editor
  and a running game with `send_variant` and `WebSocket_Variant` events.
- Local game server: host with `WebSocketHost::bind_with_options(...)` and accept
  browser or native clients into a shared session.
- Bandwidth-heavy state: compress large JSON with `send_compressed_text`; Perro
  peers decode it back into normal text/binary events.
- Resilient connections: keep links alive with heartbeats and reconnect with
  `WebSocketReconnectBackoff` after a drop.

## Practical Example

```rust
use std::cell::RefCell;

thread_local! {
    static WS: RefCell<Option<WebSocketConnection>> = RefCell::new(None);
}

lifecycle!({
    fn on_init(&self, _ctx: &mut ScriptContext<'_, API>) {
        if let Ok(conn) = WebSocketConnection::connect_with_options(
            "ws://127.0.0.1:7777",
            WebSocketConnectOptions::new().variant_protocol(),
        ) {
            WS.with(|ws| *ws.borrow_mut() = Some(conn));
        }
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        WS.with(|ws| {
            let mut ws = ws.borrow_mut();
            let Some(conn) = ws.as_mut() else { return; };
            if let Ok(Some(event)) = conn.poll_variant_event_default() {
                emit_net_event!(ctx.run, event);
            }
        });
    }
});
```

## Reference

Use through `perro_api::networking`. Low-level crate: `perro_networking`. Use
WebSockets for browser tools, local game servers, and JSON message streams.

## Variant Protocol

Use `WEBSOCKET_VARIANT_PROTOCOL` for Perro JSON messages.

```rust
let host = WebSocketHost::bind_with_options(
    "127.0.0.1:7777",
    WebSocketHostOptions::new().variant_protocol(),
)?;

let mut client = WebSocketConnection::connect_with_options(
    "ws://127.0.0.1:7777",
    WebSocketConnectOptions::new()
        .header("Authorization", "Bearer token")
        .variant_protocol(),
)?;

client.send_variant(&Variant::from("hello"))?;

let mut server = loop {
    if let Some(conn) = host.accept()? {
        break conn;
    }
};

if let Some(NetEvent::WebSocketVariant { peer, value }) = server.poll_variant_event_default()? {
    println!("{peer}: {value:?}");
}
```

## Browser Shape

Browser code can use plain JSON.

```js
const ws = new WebSocket("ws://127.0.0.1:7777", "perro.variant.v1");
ws.send(JSON.stringify({ op: "join", name: "player" }));
```

## Events

- `WebSocket_Connected`
- `WebSocket_ClientConnected`
- `WebSocket_Text`
- `WebSocket_Binary`
- `WebSocket_Variant`
- `WebSocket_InvalidJson`
- `WebSocket_Ping`
- `WebSocket_Pong`
- `WebSocket_Closed`

## Limits

Set the message cap with `max_message_bytes`.

```rust
let opts = WebSocketConnectOptions::new().max_message_bytes(256 * 1024);
```

## Compression

Use zlib helper sends for Perro peers.

```rust
connection.send_compressed_text("large json payload")?;
connection.send_compressed_binary(bytes)?;
```

Compressed frames are binary frames with a Perro prefix. Peers using
`perro_api::networking` decode them back into normal text or binary events.

## Async

Use async wrappers when running inside Tokio.

```rust
let host = WebSocketAsyncHost::bind("127.0.0.1:7777").await?;
let client = WebSocketAsyncConnection::connect("ws://127.0.0.1:7777").await?;

client.send_text("hello").await?;

if let Some(conn) = host.accept().await? {
    if let Some(event) = conn.poll_event_default().await? {
        println!("{event:?}");
    }
}
```

Async wrappers run blocking socket work on Tokio blocking tasks.

## Heartbeat

Send ping:

```rust
connection.send_heartbeat_ping()?;
```

Receive:

```rust
if let Some(NetEvent::WebSocketPing { bytes, .. }) = connection.poll_event_default()? {
    assert_eq!(bytes, perro_api::networking::WEBSOCKET_HEARTBEAT_BYTES);
}
```

## Close

Close with code and reason:

```rust
connection.close_with_reason(1000, "done")?;
```

The close event includes peer, code, and reason.

## Reconnect Backoff

Use `WebSocketReconnectBackoff` to schedule retry delay after a drop.

```rust
let mut backoff = perro_api::networking::WebSocketReconnectBackoff::new(250, 5_000);
let delay_ms = backoff.next_delay_ms();
backoff.reset();
```

`new(min_delay_ms, max_delay_ms)` bounds the delay; call `reset()` after a
successful reconnect.
