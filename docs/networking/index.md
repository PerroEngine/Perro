# Networking

> Native boundary: Perro transport/session APIs run on native builds. WASM
> builds do not expose these sockets or sessions; a native Perro peer may still
> communicate with an external browser client through a compatible server.

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Topics | [Topics](#topics) |
| NetworkWorld | [NetworkWorld](#networkworld) |
| Event Bridge | [Event Bridge](#event-bridge) |

## Purpose

Perro's networking gives games raw sockets and a lightweight event bridge without
pulling in a full netcode framework. You choose the transport that fits the
mechanic: TCP for ordered reliable streams, UDP for fast lossy packets, WebSocket
for browser and tool clients, HTTP for request/response services, and the
`multiplayer` layer for backend-agnostic LAN/Steam sessions. Networking state
lives in your game, you poll it each update, and incoming data becomes engine
signals when that is convenient.

## Use Cases

- Backend-agnostic co-op/versus: run the same session over LAN or Steam through
  `multiplayer` (see [Multiplayer: LAN + Steam](multiplayer.md)).
- Fast movement sync: stream positions over UDP where a dropped packet is fine.
- Reliable match/chat channel: send ordered framed messages over TCP.
- Browser and dev-tool clients: accept JSON over WebSocket.
- Online services: fetch news, submit scores, or check for patches over HTTP.
- Signal-driven reactions: forward received packets to script handlers with
  `emit_net_event!` / `emit_http_event!`.

Use through `perro_api::networking`. Low-level crate: `perro_networking`.

## Transport Choice

| Need | Choose | Why | Tradeoff |
| --- | --- | --- | --- |
| session API across LAN + Steam | multiplayer | one lobby/session model | less transport-specific control |
| ordered commands or chat | TCP frames | preserves order + delivery | a lost packet may delay later data |
| frequent replaceable snapshots | UDP | late/lost data does not block next snapshot | game owns loss, order, and recovery |
| browser/tool duplex channel | WebSocket | browser-native framed messages | not the lowest-overhead game transport |
| request/response service | HTTP | status/body lifecycle fits web APIs | not a continuous session channel |

Keep socket/world ownership in one long-lived game state. Drain events once per
update, apply authoritative state, and emit signals only for loose gameplay
reactions. Signals do not replace packet validation or session ownership.

## Topics

- [Multiplayer: LAN + Steam](multiplayer.md)
- [HTTP](http.md)
- [TCP](tcp.md)
- [UDP](udp.md)
- [WebSocket](websocket.md)

## NetworkWorld

`NetworkWorld` owns TCP hosts, TCP connections, UDP endpoints, WebSocket hosts,
and WebSocket connections behind stable id handles, so one object can drive every
socket a game uses.

- Keep the `NetworkWorld` (or individual sockets) in your game state.
- Poll it each update with `poll_events`, `poll_frame_events`, or
  `poll_variant_events`.
- Emit signals from the drained events when useful.

## Event Bridge

Every `NetEvent` maps to a signal so gameplay can react without matching bytes by
hand. Each event provides:

- `signal_name() -> &'static str`
- `signal_id() -> SignalID`
- `signal_params() -> Vec<Variant>`

Emit an event as a signal with the macro (the runtime window `ctx.run` supplies
`Signals()`):

```rust
emit_net_event!(ctx.run, event);
```
