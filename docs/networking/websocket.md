# WebSocket

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

Use `WebSocket` when this feature, type group, file format, or workflow appears in game code or assets.

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

# WebSocket

Use through `perro_api::networking`.

Low-level crate: `perro_networking`.

Use WebSockets for browser tools, local game servers, and JSON message streams.

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

Set message cap with `max_message_bytes`.

```rust
let opts = WebSocketConnectOptions::new().max_message_bytes(256 * 1024);
```

## Compression

Use zlib helper sends for Perro peers.

```rust
connection.send_compressed_text("large json payload")?;
connection.send_compressed_binary(bytes)?;
```

Compressed frames are binary frames with a Perro prefix.

Peers using `perro_api::networking` decode them back into normal text or binary events.

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

Close with code + reason:

```rust
connection.close_with_reason(1000, "done")?;
```

Close event includes peer, code, and reason.

## Reconnect Backoff

Use `WebSocketReconnectBackoff` to schedule retry delay.

```rust
let mut backoff = perro_api::networking::WebSocketReconnectBackoff::new(250, 5_000);
let delay_ms = backoff.next_delay_ms();
backoff.reset();
```
