# Networking

Use through `perro_api::networking`.

Low-level crate: `perro_networking`.

Topics:

- [HTTP](http.md)
- [TCP](tcp.md)
- [UDP](udp.md)
- [WebSocket](websocket.md)
- [WebRTC](webrtc.md)

`NetworkWorld` owns TCP hosts, TCP connections, UDP endpoints, WebSocket hosts, and WebSocket connections.

Keep networking state in script or game state.

Poll each update.

Emit signals when useful.

## Event Bridge

Every `NetEvent` gives:

- `signal_name() -> &'static str`
- `signal_id() -> SignalID`
- `signal_params() -> Vec<Variant>`

Use macro:

```rust
emit_net_event!(ctx, event)
```
