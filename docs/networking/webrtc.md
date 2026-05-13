# WebRTC

Use through `perro_api::networking`.

Low-level crate: `perro_networking`.

Use it for peer-to-peer DataChannel traffic.

Use WebSocket, HTTP, Steam lobbies, or your own service for signaling.

Current wrapper scope:

- peer connection
- ICE server config
- SDP offer/answer
- ICE candidate apply/poll
- DataChannel create/send
- DataChannel open/close/text/binary events

Not wrapped yet:

- media tracks
- RTP/RTCP stats
- custom codec/media engine config

## Peer

Create peer:

```rust
let mut peer = WebRtcPeer::new(
    WebRtcPeerConfig::new()
        .ice_server(WebRtcIceServer::stun("stun:stun.l.google.com:19302")),
)?;
```

Create DataChannel before offer:

```rust
let channel = peer.create_data_channel("game")?;
let offer = peer.create_offer()?;
```

Send `offer` to remote peer through signaling.

## Answer Flow

Remote side accepts offer, then creates answer:

```rust
let mut remote = WebRtcPeer::new(WebRtcPeerConfig::new())?;
remote.accept_signal(&offer)?;
let answer = remote.create_answer()?;
```

Send `answer` back to offer peer:

```rust
peer.accept_signal(&answer)?;
```

## ICE

Local ICE candidates arrive as normal `NetEvent`s.

```rust
for event in peer.poll_events(32) {
    if let NetEvent::WebRtcIceCandidate { candidate, .. } = event {
        // send candidate through signaling
    }
}
```

Apply remote ICE:

```rust
peer.accept_signal(&WebRtcSignal::ice_candidate(candidate))?;
```

Equivalent direct call:

```rust
peer.add_ice_candidate(&candidate)?;
```

## DataChannel

Send text:

```rust
peer.send_data_channel_text(channel, "hello")?;
```

Send bytes:

```rust
peer.send_data_channel_binary(channel, vec![1, 2, 3])?;
```

Poll received events:

```rust
for event in peer.poll_events(32) {
    match event {
        NetEvent::WebRtcDataChannelOpen { label } => println!("open {label}"),
        NetEvent::WebRtcDataChannelText { label, text } => println!("{label}: {text}"),
        NetEvent::WebRtcDataChannelBinary { label, bytes } => println!("{label}: {bytes:?}"),
        NetEvent::WebRtcDataChannelClosed { label } => println!("closed {label}"),
        _ => {}
    }
}
```

## Signaling JSON

`WebRtcSignal` converts offer, answer, and ICE candidate to JSON.

This JSON is Perro signaling shape.

The underlying SDP/ICE values come from the `webrtc` crate.

```rust
let text = offer.to_json_string()?;
let signal = WebRtcSignal::from_json_str(&text)?;
```

WebSocket helpers can send this directly:

```rust
socket.send_webrtc_signal(&offer)?;
```

Read from WebSocket:

```rust
if let Some(event) = socket.poll_webrtc_signal_event(256 * 1024)? {
    match event {
        NetEvent::WebRtcOffer { sdp, .. } => {
            peer.accept_signal(&WebRtcSignal::offer(sdp))?;
        }
        NetEvent::WebRtcAnswer { sdp, .. } => {
            peer.accept_signal(&WebRtcSignal::answer(sdp))?;
        }
        NetEvent::WebRtcIceCandidate { candidate, .. } => {
            peer.accept_signal(&WebRtcSignal::ice_candidate(candidate))?;
        }
        _ => {}
    }
}
```

## Events

- `WebRTC_Offer`
- `WebRTC_Answer`
- `WebRTC_IceCandidate`
- `WebRTC_DataChannel`
- `WebRTC_DataChannelOpen`
- `WebRTC_DataChannelClosed`
- `WebRTC_DataChannelText`
- `WebRTC_DataChannelBinary`

## Notes

This module owns a Tokio runtime per `WebRtcPeer`.

Keep `WebRtcPeer` in script or game state.

Poll each update.

Signal transport is separate from WebRTC itself.

Use `WebSocketConnection::send_webrtc_signal` only as one signaling option.

Use any transport that can move `WebRtcSignal` JSON.
