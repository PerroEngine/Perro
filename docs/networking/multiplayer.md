# Multiplayer: LAN + Steam

## Page Map

| Header | Link |
| --- | --- |
| Abstraction | [Abstraction](#abstraction) |
| LAN | [LAN](#lan) |
| Steam | [Steam](#steam) |
| Frame Loop | [Frame Loop](#frame-loop) |
| Limits | [Limits](#limits) |

## Abstraction

Use `perro_api::networking::multiplayer`.

Game messages stay as opaque bytes.

Session, slots, heartbeat, payload events, and disconnect behavior stay the same across both backends.

Select only the backend:

```rust
use perro_api::networking::multiplayer::{
    self as net, LobbyPrivacy, NetworkBackend,
};

net::host(NetworkBackend::Lan, 4, LobbyPrivacy::Public)?;
// or:
net::host(NetworkBackend::Steam, 4, LobbyPrivacy::Friends)?;
```

Enable the Perro `steamworks` feature to compile the Steam backend.

LAN builds do not compile the Steam runtime.

## LAN

Perro stores the game's LAN choice at `user://networking/lan_consent`.

Prompt only while the choice is unknown:

```rust
match net::lan_consent() {
    net::LanConsent::Unknown => {
        // Show game UI once, then save its result.
        net::set_lan_consent(net::LanConsent::Allowed)?;
        // or: net::set_lan_consent(net::LanConsent::Denied)?;
    }
    net::LanConsent::Allowed => {}
    net::LanConsent::Denied => return Ok(()),
}
```

`Denied` blocks LAN host, discovery, and direct join before socket bind.

`Unknown` keeps old games compatible and permits LAN.

New game UI should check `lan_consent()` and save a choice before its first LAN call.

`host_lan()` binds UDP `0.0.0.0:7777`.

This accepts:

- same PC through localhost
- same Ethernet or Wi-Fi network
- routed VPN adapters

Discover hosts on the same LAN:

```rust
net::refresh_lobbies(NetworkBackend::Lan, Default::default())?;

// Poll until NetEvent::LobbyRowsChanged, display net::friends(), then:
net::join(NetworkBackend::Lan, 0)?;
```

Discovery sends IPv4 broadcast on UDP port `7777` and also probes localhost.

VPN software may block or not route broadcast discovery.

Join a known LAN or routed VPN address directly when discovery does not find it:

```rust
net::join_lan_at("192.168.1.42")?;
net::join_lan_at("25.10.20.30:7777")?;
```

Missing port defaults to `7777`.

Allow inbound and outbound UDP port `7777` in the OS firewall.

The OS firewall decision is separate and stays in the OS firewall rule store.

`refresh_lobbies(NetworkBackend::Steam, ...)` does not open a LAN socket.

Legacy `refresh_steam_lobbies(...)` keeps its old combined Steam + LAN row behavior.

## Steam

Set game metadata before host or browse:

```rust
net::set_game_name("My Game");
net::set_game_tag("my-game-v1");
```

Host or browse:

```rust
net::host(NetworkBackend::Steam, 4, LobbyPrivacy::Public)?;

net::refresh_lobbies(NetworkBackend::Steam, Default::default())?;
let rows = net::lobbies();
net::join(NetworkBackend::Steam, rows[0].lobby_id)?;
```

Steam handles lobby discovery, invites, identity, and P2P routing.

## Frame Loop

```rust
net::poll();

for event in net::drain_events() {
    match event {
        net::NetEvent::Payload { from_slot, bytes } => {
            // Decode game-owned bytes.
        }
        net::NetEvent::SlotAssigned { slot } => {}
        net::NetEvent::PeerReady { slot, .. } => {}
        net::NetEvent::PeerLeft { slot } => {}
        net::NetEvent::Disconnected => {}
        _ => {}
    }
}

net::send(b"game payload", true);
```

Call `poll()` once per frame.

Call `disconnect()` when leaving the session.

## Limits

LAN transport uses UDP.

LAN `reliable` currently does not add retransmit or ordering.

Steam maps `reliable` to Steam P2P delivery modes.

LAN traffic has no built-in encryption or authentication.

Internet LAN hosting needs router port forwarding or a VPN.

WASM does not expose this native multiplayer layer.
