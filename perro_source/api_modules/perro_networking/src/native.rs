use std::{
    fmt,
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket},
    string::FromUtf8Error,
    sync::{Arc, Mutex},
};

use perro_ids::SignalID;
use perro_io::{compress_zlib_best, decompress_zlib};
use perro_variant::Variant;
use tokio::task;
use tungstenite::{
    ClientRequestBuilder, Message, WebSocket,
    http::Uri,
    protocol::{
        CloseFrame, WebSocketConfig,
        frame::{Utf8Bytes, coding::CloseCode},
    },
    stream::MaybeTlsStream,
};

#[path = "error.rs"]
mod error;
#[path = "event.rs"]
mod event;
#[path = "ids.rs"]
mod ids;
#[path = "slot.rs"]
mod slot;
#[path = "tcp.rs"]
mod tcp;
#[path = "udp.rs"]
mod udp;
#[path = "util.rs"]
mod util;
#[path = "websocket.rs"]
mod websocket;
#[path = "world.rs"]
mod world;

#[path = "http.rs"]
pub mod http;

pub use error::*;
pub use event::*;
pub use http::*;
pub use ids::*;
pub use tcp::*;
pub use udp::*;
pub use websocket::*;
pub use world::*;

pub(crate) use slot::*;
pub(crate) use util::*;

#[macro_export]
macro_rules! emit_net_event {
    ($ctx:expr, $event:expr) => {{
        let event = $event;
        let params = event.signal_params();
        $ctx.Signals()
            .signal_emit(event.signal_id(), params.as_slice())
    }};
}

#[cfg(test)]
#[path = "../tests/unit/net_tests.rs"]
mod tests;
