// Session control framing. The crate owns connection lifecycle (slot
// assignment, join hello); game payloads pass through as opaque bytes.
//
// Frame layout:
//   [0xC1][ctrl_kind:u8][slot:i64 LE]  control
//   [0xC2][game bytes...]              payload
//
// Every packet on the wire is framed, so the leading byte is unambiguous and
// game codecs never need to avoid reserved values.

const FRAME_CONTROL: u8 = 0xC1;
const FRAME_PAYLOAD: u8 = 0xC2;
const CTRL_SLOT_ASSIGNED: u8 = 1;
const CTRL_CLIENT_READY: u8 = 2;
const CTRL_HEARTBEAT: u8 = 3;
const CTRL_CLIENT_DISCONNECT: u8 = 4;
const CTRL_HOST_DISCONNECT: u8 = 5;

#[derive(Debug, PartialEq, Eq)]
pub enum Frame<'a> {
    SlotAssigned(i64),
    ClientReady,
    ClientDisconnect,
    HostDisconnect,
    /// Liveness filler — carries no game data; only bumps the sender's last-seen.
    Heartbeat,
    Payload(&'a [u8]),
}

pub fn encode_slot_assigned(slot: i64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(10);
    encode_slot_assigned_into(&mut buf, slot);
    buf
}

pub fn encode_slot_assigned_into(buf: &mut Vec<u8>, slot: i64) {
    buf.clear();
    buf.push(FRAME_CONTROL);
    buf.push(CTRL_SLOT_ASSIGNED);
    buf.extend_from_slice(&slot.to_le_bytes());
}

pub fn encode_client_ready() -> Vec<u8> {
    vec![FRAME_CONTROL, CTRL_CLIENT_READY]
}

pub fn encode_heartbeat() -> Vec<u8> {
    vec![FRAME_CONTROL, CTRL_HEARTBEAT]
}

pub fn encode_client_disconnect() -> Vec<u8> {
    vec![FRAME_CONTROL, CTRL_CLIENT_DISCONNECT]
}

pub fn encode_host_disconnect_into(buf: &mut Vec<u8>) {
    buf.clear();
    buf.push(FRAME_CONTROL);
    buf.push(CTRL_HOST_DISCONNECT);
}

pub fn encode_heartbeat_into(buf: &mut Vec<u8>) {
    buf.clear();
    buf.push(FRAME_CONTROL);
    buf.push(CTRL_HEARTBEAT);
}

pub fn wrap_payload(bytes: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(bytes.len() + 1);
    wrap_payload_into(&mut buf, bytes);
    buf
}

/// Frame `bytes` into `buf` without allocating once `buf` has capacity.
/// Callers keep a scratch buffer alive across sends for an alloc-free path.
pub fn wrap_payload_into(buf: &mut Vec<u8>, bytes: &[u8]) {
    buf.clear();
    buf.reserve(bytes.len() + 1);
    buf.push(FRAME_PAYLOAD);
    buf.extend_from_slice(bytes);
}

/// If `bytes` is a payload frame, strip the 1-byte header in place — reusing
/// the packet's allocation instead of copying the game bytes out — and return
/// true. Control/garbage frames are left untouched for [`parse`].
pub fn strip_payload_in_place(bytes: &mut Vec<u8>) -> bool {
    if bytes.first() != Some(&FRAME_PAYLOAD) {
        return false;
    }
    bytes.copy_within(1.., 0);
    bytes.pop();
    true
}

pub fn parse(bytes: &[u8]) -> Option<Frame<'_>> {
    match *bytes.first()? {
        FRAME_CONTROL => match *bytes.get(1)? {
            CTRL_SLOT_ASSIGNED => {
                let slot = i64::from_le_bytes(bytes.get(2..10)?.try_into().ok()?);
                Some(Frame::SlotAssigned(slot))
            }
            CTRL_CLIENT_READY => Some(Frame::ClientReady),
            CTRL_HEARTBEAT => Some(Frame::Heartbeat),
            CTRL_CLIENT_DISCONNECT => Some(Frame::ClientDisconnect),
            CTRL_HOST_DISCONNECT => Some(Frame::HostDisconnect),
            _ => None,
        },
        FRAME_PAYLOAD => Some(Frame::Payload(&bytes[1..])),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_assigned_round_trips() {
        let bytes = encode_slot_assigned(7);
        assert_eq!(parse(&bytes), Some(Frame::SlotAssigned(7)));
    }

    #[test]
    fn client_ready_round_trips() {
        let bytes = encode_client_ready();
        assert_eq!(parse(&bytes), Some(Frame::ClientReady));
    }

    #[test]
    fn heartbeat_round_trips() {
        assert_eq!(parse(&encode_heartbeat()), Some(Frame::Heartbeat));
        let mut scratch = vec![0xFF; 8];
        encode_heartbeat_into(&mut scratch);
        assert_eq!(parse(&scratch), Some(Frame::Heartbeat));
    }

    #[test]
    fn client_disconnect_round_trips() {
        assert_eq!(
            parse(&encode_client_disconnect()),
            Some(Frame::ClientDisconnect)
        );
    }

    #[test]
    fn host_disconnect_round_trips() {
        let mut bytes = Vec::new();
        encode_host_disconnect_into(&mut bytes);
        assert_eq!(parse(&bytes), Some(Frame::HostDisconnect));
    }

    #[test]
    fn payload_round_trips_arbitrary_game_bytes() {
        let game = [0xC1, 0xC2, 0x00, 0xFF, 42];
        let bytes = wrap_payload(&game);
        assert_eq!(parse(&bytes), Some(Frame::Payload(&game[..])));
    }

    #[test]
    fn empty_payload_is_valid() {
        let bytes = wrap_payload(&[]);
        assert_eq!(parse(&bytes), Some(Frame::Payload(&[][..])));
    }

    #[test]
    fn into_variants_reset_dirty_scratch_buffers() {
        let mut scratch = vec![0xFF; 32];
        encode_slot_assigned_into(&mut scratch, -3);
        assert_eq!(parse(&scratch), Some(Frame::SlotAssigned(-3)));

        wrap_payload_into(&mut scratch, &[1, 2, 3]);
        assert_eq!(parse(&scratch), Some(Frame::Payload(&[1, 2, 3][..])));
    }

    #[test]
    fn strip_payload_in_place_matches_parse() {
        let game = [0xC1, 0xC2, 0x00, 0xFF, 42];
        let mut bytes = wrap_payload(&game);
        assert!(strip_payload_in_place(&mut bytes));
        assert_eq!(bytes, game);

        let mut empty = wrap_payload(&[]);
        assert!(strip_payload_in_place(&mut empty));
        assert!(empty.is_empty());

        let mut control = encode_slot_assigned(1);
        assert!(!strip_payload_in_place(&mut control));
        assert_eq!(parse(&control), Some(Frame::SlotAssigned(1)));
    }

    #[test]
    fn slot_round_trips_extreme_values() {
        for slot in [i64::MIN, -1, 0, i64::MAX] {
            let bytes = encode_slot_assigned(slot);
            assert_eq!(parse(&bytes), Some(Frame::SlotAssigned(slot)));
        }
    }

    #[test]
    fn unframed_bytes_rejected() {
        assert_eq!(parse(b"bad"), None);
        assert_eq!(parse(&[]), None);
        assert_eq!(parse(&[FRAME_CONTROL]), None);
        assert_eq!(parse(&[FRAME_CONTROL, 99]), None);
        assert_eq!(parse(&[FRAME_CONTROL, CTRL_SLOT_ASSIGNED, 1, 2]), None);
    }
}
