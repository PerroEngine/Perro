use super::*;

pub enum WebRtcSignalKind {
    Offer,
    Answer,
    IceCandidate,
}

impl WebRtcSignalKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Offer => "offer",
            Self::Answer => "answer",
            Self::IceCandidate => "ice_candidate",
        }
    }

    pub fn parse(value: &str) -> NetResult<Self> {
        match value {
            "offer" => Ok(Self::Offer),
            "answer" => Ok(Self::Answer),
            "ice_candidate" | "candidate" => Ok(Self::IceCandidate),
            _ => Err(NetError::new(
                NetErrorKind::WebRtc,
                format!("unknown webrtc signal kind: {value}"),
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebRtcIceCandidate {
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_mline_index: Option<u16>,
    pub username_fragment: Option<String>,
}

impl WebRtcIceCandidate {
    pub fn new(candidate: impl Into<String>) -> Self {
        Self {
            candidate: candidate.into(),
            sdp_mid: None,
            sdp_mline_index: None,
            username_fragment: None,
        }
    }

    pub fn with_sdp_mid(mut self, sdp_mid: impl Into<String>) -> Self {
        self.sdp_mid = Some(sdp_mid.into());
        self
    }

    pub fn with_sdp_mline_index(mut self, sdp_mline_index: u16) -> Self {
        self.sdp_mline_index = Some(sdp_mline_index);
        self
    }

    pub fn with_username_fragment(mut self, username_fragment: impl Into<String>) -> Self {
        self.username_fragment = Some(username_fragment.into());
        self
    }

    pub fn to_variant(&self) -> Variant {
        let mut object = BTreeMap::new();
        insert_str(&mut object, "candidate", self.candidate.clone());
        if let Some(sdp_mid) = &self.sdp_mid {
            insert_str(&mut object, "sdp_mid", sdp_mid.clone());
        }
        if let Some(sdp_mline_index) = self.sdp_mline_index {
            object.insert(Arc::from("sdp_mline_index"), Variant::from(sdp_mline_index));
        }
        if let Some(username_fragment) = &self.username_fragment {
            insert_str(&mut object, "username_fragment", username_fragment.clone());
        }
        Variant::from(object)
    }

    pub fn from_variant(value: &Variant) -> NetResult<Self> {
        let object = value
            .as_object()
            .ok_or_else(|| NetError::new(NetErrorKind::WebRtc, "ice candidate must be object"))?;
        Ok(Self {
            candidate: required_str(object, "candidate")?,
            sdp_mid: optional_str(object, "sdp_mid"),
            sdp_mline_index: optional_u16(object, "sdp_mline_index")?,
            username_fragment: optional_str(object, "username_fragment"),
        })
    }

    fn to_webrtc(&self) -> RTCIceCandidateInit {
        RTCIceCandidateInit {
            candidate: self.candidate.clone(),
            sdp_mid: self.sdp_mid.clone(),
            sdp_mline_index: self.sdp_mline_index,
            username_fragment: self.username_fragment.clone(),
        }
    }

    fn from_webrtc(candidate: RTCIceCandidateInit) -> Self {
        Self {
            candidate: candidate.candidate,
            sdp_mid: candidate.sdp_mid,
            sdp_mline_index: candidate.sdp_mline_index,
            username_fragment: candidate.username_fragment,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebRtcSignal {
    Offer { sdp: String },
    Answer { sdp: String },
    IceCandidate(WebRtcIceCandidate),
}

impl WebRtcSignal {
    pub fn offer(sdp: impl Into<String>) -> Self {
        Self::Offer { sdp: sdp.into() }
    }

    pub fn answer(sdp: impl Into<String>) -> Self {
        Self::Answer { sdp: sdp.into() }
    }

    pub fn ice_candidate(candidate: WebRtcIceCandidate) -> Self {
        Self::IceCandidate(candidate)
    }

    pub fn kind(&self) -> WebRtcSignalKind {
        match self {
            Self::Offer { .. } => WebRtcSignalKind::Offer,
            Self::Answer { .. } => WebRtcSignalKind::Answer,
            Self::IceCandidate(_) => WebRtcSignalKind::IceCandidate,
        }
    }

    pub fn to_variant(&self) -> Variant {
        let mut object = BTreeMap::new();
        insert_str(&mut object, "type", self.kind().as_str());
        match self {
            Self::Offer { sdp } | Self::Answer { sdp } => insert_str(&mut object, "sdp", sdp),
            Self::IceCandidate(candidate) => {
                object.insert(Arc::from("candidate"), candidate.to_variant());
            }
        }
        Variant::from(object)
    }

    pub fn from_variant(value: &Variant) -> NetResult<Self> {
        let object = value
            .as_object()
            .ok_or_else(|| NetError::new(NetErrorKind::WebRtc, "webrtc signal must be object"))?;
        match WebRtcSignalKind::parse(&required_str(object, "type")?)? {
            WebRtcSignalKind::Offer => Ok(Self::offer(required_str(object, "sdp")?)),
            WebRtcSignalKind::Answer => Ok(Self::answer(required_str(object, "sdp")?)),
            WebRtcSignalKind::IceCandidate => {
                let candidate = object.get("candidate").ok_or_else(|| {
                    NetError::new(NetErrorKind::WebRtc, "missing webrtc candidate")
                })?;
                Ok(Self::ice_candidate(WebRtcIceCandidate::from_variant(
                    candidate,
                )?))
            }
        }
    }

    pub fn to_json_string(&self) -> NetResult<String> {
        serde_json::to_string(&self.to_variant().to_json_value())
            .map_err(|err| NetError::new(NetErrorKind::WebRtc, err.to_string()))
    }

    pub fn from_json_str(text: &str) -> NetResult<Self> {
        let value = serde_json::from_str(text)
            .map(Variant::from_json_value)
            .map_err(|err| NetError::new(NetErrorKind::WebRtc, err.to_string()))?;
        Self::from_variant(&value)
    }

    pub fn into_event(self, peer: String) -> NetEvent {
        match self {
            Self::Offer { sdp } => NetEvent::WebRtcOffer { peer, sdp },
            Self::Answer { sdp } => NetEvent::WebRtcAnswer { peer, sdp },
            Self::IceCandidate(candidate) => NetEvent::WebRtcIceCandidate { peer, candidate },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebRtcIceServer {
    pub urls: Vec<String>,
    pub username: String,
    pub credential: String,
}

impl WebRtcIceServer {
    pub fn stun(url: impl Into<String>) -> Self {
        Self {
            urls: vec![url.into()],
            username: String::new(),
            credential: String::new(),
        }
    }

    pub fn turn(
        url: impl Into<String>,
        username: impl Into<String>,
        credential: impl Into<String>,
    ) -> Self {
        Self {
            urls: vec![url.into()],
            username: username.into(),
            credential: credential.into(),
        }
    }

    fn to_webrtc(&self) -> RTCIceServer {
        RTCIceServer {
            urls: self.urls.clone(),
            username: self.username.clone(),
            credential: self.credential.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WebRtcPeerConfig {
    pub ice_servers: Vec<WebRtcIceServer>,
}

impl WebRtcPeerConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ice_server(mut self, server: WebRtcIceServer) -> Self {
        self.ice_servers.push(server);
        self
    }

    fn to_webrtc(&self) -> RTCConfiguration {
        RTCConfiguration {
            ice_servers: self
                .ice_servers
                .iter()
                .map(WebRtcIceServer::to_webrtc)
                .collect(),
            ..RTCConfiguration::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WebRtcDataChannelId(pub u32);

pub struct WebRtcPeer {
    runtime: Runtime,
    peer: Arc<RTCPeerConnection>,
    channels: Vec<Option<Arc<RTCDataChannel>>>,
    events: Arc<Mutex<VecDeque<NetEvent>>>,
}

impl WebRtcPeer {
    pub fn new(config: WebRtcPeerConfig) -> NetResult<Self> {
        let runtime =
            Runtime::new().map_err(|err| NetError::new(NetErrorKind::WebRtc, err.to_string()))?;
        let peer = runtime
            .block_on(async {
                APIBuilder::new()
                    .build()
                    .new_peer_connection(config.to_webrtc())
                    .await
            })
            .map_err(webrtc_err)?;
        let events = Arc::new(Mutex::new(VecDeque::new()));
        let mut out = Self {
            runtime,
            peer: Arc::new(peer),
            channels: Vec::new(),
            events,
        };
        out.install_peer_handlers();
        Ok(out)
    }

    pub fn create_data_channel(
        &mut self,
        label: impl Into<String>,
    ) -> NetResult<WebRtcDataChannelId> {
        let label = label.into();
        let channel = self
            .runtime
            .block_on(self.peer.create_data_channel(&label, None))
            .map_err(webrtc_err)?;
        self.install_channel_handlers(Arc::clone(&channel));
        Ok(WebRtcDataChannelId(insert_slot(
            &mut self.channels,
            channel,
        )))
    }

    pub fn create_offer(&self) -> NetResult<WebRtcSignal> {
        let offer = self
            .runtime
            .block_on(self.peer.create_offer(None))
            .map_err(webrtc_err)?;
        self.runtime
            .block_on(self.peer.set_local_description(offer.clone()))
            .map_err(webrtc_err)?;
        Ok(WebRtcSignal::offer(offer.sdp))
    }

    pub fn create_answer(&self) -> NetResult<WebRtcSignal> {
        let answer = self
            .runtime
            .block_on(self.peer.create_answer(None))
            .map_err(webrtc_err)?;
        self.runtime
            .block_on(self.peer.set_local_description(answer.clone()))
            .map_err(webrtc_err)?;
        Ok(WebRtcSignal::answer(answer.sdp))
    }

    pub fn accept_signal(&self, signal: &WebRtcSignal) -> NetResult<()> {
        match signal {
            WebRtcSignal::Offer { sdp } => {
                let desc = RTCSessionDescription::offer(sdp.clone()).map_err(webrtc_err)?;
                self.runtime
                    .block_on(self.peer.set_remote_description(desc))
                    .map_err(webrtc_err)
            }
            WebRtcSignal::Answer { sdp } => {
                let desc = RTCSessionDescription::answer(sdp.clone()).map_err(webrtc_err)?;
                self.runtime
                    .block_on(self.peer.set_remote_description(desc))
                    .map_err(webrtc_err)
            }
            WebRtcSignal::IceCandidate(candidate) => self.add_ice_candidate(candidate),
        }
    }

    pub fn add_ice_candidate(&self, candidate: &WebRtcIceCandidate) -> NetResult<()> {
        self.runtime
            .block_on(self.peer.add_ice_candidate(candidate.to_webrtc()))
            .map_err(webrtc_err)
    }

    pub fn send_data_channel_text(
        &self,
        id: WebRtcDataChannelId,
        text: impl Into<String>,
    ) -> NetResult<usize> {
        let channel = self.channel(id)?;
        self.runtime
            .block_on(channel.send_text(text.into()))
            .map_err(webrtc_err)
    }

    pub fn send_data_channel_binary(
        &self,
        id: WebRtcDataChannelId,
        bytes: Vec<u8>,
    ) -> NetResult<usize> {
        let channel = self.channel(id)?;
        self.runtime
            .block_on(channel.send(&Bytes::from(bytes)))
            .map_err(webrtc_err)
    }

    pub fn poll_event(&self) -> Option<NetEvent> {
        self.events.lock().ok()?.pop_front()
    }

    pub fn poll_events(&self, max_events: usize) -> Vec<NetEvent> {
        let Ok(mut events) = self.events.lock() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for _ in 0..max_events {
            let Some(event) = events.pop_front() else {
                break;
            };
            out.push(event);
        }
        out
    }

    fn channel(&self, id: WebRtcDataChannelId) -> NetResult<Arc<RTCDataChannel>> {
        self.channels
            .get(id.0 as usize)
            .and_then(Option::as_ref)
            .map(Arc::clone)
            .ok_or_else(|| {
                NetError::new(
                    NetErrorKind::MissingHandle,
                    format!("missing webrtc data channel {}", id.0),
                )
            })
    }

    fn install_peer_handlers(&mut self) {
        let events = Arc::clone(&self.events);
        self.peer.on_ice_candidate(Box::new(move |candidate| {
            let events = Arc::clone(&events);
            Box::pin(async move {
                let Some(candidate) = candidate else {
                    return;
                };
                if let Ok(json) = candidate.to_json() {
                    push_webrtc_event(
                        &events,
                        WebRtcSignal::ice_candidate(WebRtcIceCandidate::from_webrtc(json))
                            .into_event("local".to_string()),
                    );
                }
            })
        }));

        let events = Arc::clone(&self.events);
        self.peer.on_data_channel(Box::new(move |channel| {
            let events = Arc::clone(&events);
            Box::pin(async move {
                install_data_channel_handlers(&events, Arc::clone(&channel));
                push_webrtc_event(
                    &events,
                    NetEvent::WebRtcDataChannel {
                        label: channel.label().to_string(),
                    },
                );
            })
        }));
    }

    fn install_channel_handlers(&self, channel: Arc<RTCDataChannel>) {
        install_data_channel_handlers(&self.events, channel);
    }
}

fn webrtc_err(err: ::webrtc::Error) -> NetError {
    NetError::new(NetErrorKind::WebRtc, err.to_string())
}

fn push_webrtc_event(events: &Arc<Mutex<VecDeque<NetEvent>>>, event: NetEvent) {
    if let Ok(mut events) = events.lock() {
        events.push_back(event);
    }
}

fn install_data_channel_handlers(
    events: &Arc<Mutex<VecDeque<NetEvent>>>,
    channel: Arc<RTCDataChannel>,
) {
    let label = channel.label().to_string();
    let open_events = Arc::clone(events);
    let open_label = label.clone();
    channel.on_open(Box::new(move || {
        let events = Arc::clone(&open_events);
        let label = open_label.clone();
        Box::pin(async move {
            push_webrtc_event(&events, NetEvent::WebRtcDataChannelOpen { label });
        })
    }));

    let close_events = Arc::clone(events);
    let close_label = label.clone();
    channel.on_close(Box::new(move || {
        let events = Arc::clone(&close_events);
        let label = close_label.clone();
        Box::pin(async move {
            push_webrtc_event(&events, NetEvent::WebRtcDataChannelClosed { label });
        })
    }));

    let message_events = Arc::clone(events);
    channel.on_message(Box::new(move |message: DataChannelMessage| {
        let events = Arc::clone(&message_events);
        let label = label.clone();
        Box::pin(async move {
            let event = if message.is_string {
                NetEvent::WebRtcDataChannelText {
                    label,
                    text: String::from_utf8_lossy(&message.data).to_string(),
                }
            } else {
                NetEvent::WebRtcDataChannelBinary {
                    label,
                    bytes: message.data.to_vec(),
                }
            };
            push_webrtc_event(&events, event);
        })
    }));
}

fn insert_str(
    object: &mut BTreeMap<Arc<str>, Variant>,
    key: &'static str,
    value: impl Into<String>,
) {
    object.insert(Arc::from(key), Variant::from(value.into()));
}

fn required_str(object: &BTreeMap<Arc<str>, Variant>, key: &'static str) -> NetResult<String> {
    object
        .get(key)
        .and_then(Variant::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| NetError::new(NetErrorKind::WebRtc, format!("missing webrtc {key}")))
}

fn optional_str(object: &BTreeMap<Arc<str>, Variant>, key: &'static str) -> Option<String> {
    object
        .get(key)
        .and_then(Variant::as_str)
        .map(ToString::to_string)
}

fn optional_u16(object: &BTreeMap<Arc<str>, Variant>, key: &'static str) -> NetResult<Option<u16>> {
    let Some(value) = object.get(key) else {
        return Ok(None);
    };
    if let Some(value) = value.as_u16() {
        return Ok(Some(value));
    }
    value
        .as_u64()
        .and_then(|value| u16::try_from(value).ok())
        .or_else(|| value.as_i64().and_then(|value| u16::try_from(value).ok()))
        .map(Some)
        .ok_or_else(|| NetError::new(NetErrorKind::WebRtc, format!("invalid webrtc {key}")))
}
