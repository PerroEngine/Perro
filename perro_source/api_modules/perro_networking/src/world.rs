use super::*;

pub struct NetworkWorld {
    tcp_hosts: Vec<Option<TcpHost>>,
    tcp_connections: Vec<Option<TcpConnection>>,
    udp_endpoints: Vec<Option<UdpEndpoint>>,
    websocket_hosts: Vec<Option<WebSocketHost>>,
    websocket_connections: Vec<Option<WebSocketConnection>>,
}

impl NetworkWorld {
    pub fn new() -> Self {
        Self {
            tcp_hosts: Vec::new(),
            tcp_connections: Vec::new(),
            udp_endpoints: Vec::new(),
            websocket_hosts: Vec::new(),
            websocket_connections: Vec::new(),
        }
    }

    pub fn bind_tcp_host<A: ToSocketAddrs>(&mut self, addr: A) -> NetResult<TcpHostId> {
        let host = TcpHost::bind(addr)?;
        Ok(TcpHostId(insert_slot(&mut self.tcp_hosts, host)))
    }

    pub fn connect_tcp<A: ToSocketAddrs>(&mut self, addr: A) -> NetResult<TcpConnectionId> {
        let connection = TcpConnection::connect(addr)?;
        Ok(TcpConnectionId(insert_slot(
            &mut self.tcp_connections,
            connection,
        )))
    }

    pub fn bind_udp<A: ToSocketAddrs>(&mut self, addr: A) -> NetResult<UdpEndpointId> {
        let endpoint = UdpEndpoint::bind(addr)?;
        Ok(UdpEndpointId(insert_slot(
            &mut self.udp_endpoints,
            endpoint,
        )))
    }

    pub fn bind_websocket_host<A: ToSocketAddrs>(&mut self, addr: A) -> NetResult<WebSocketHostId> {
        let host = WebSocketHost::bind(addr)?;
        Ok(WebSocketHostId(insert_slot(
            &mut self.websocket_hosts,
            host,
        )))
    }

    pub fn bind_websocket_host_with_options<A: ToSocketAddrs>(
        &mut self,
        addr: A,
        options: WebSocketHostOptions,
    ) -> NetResult<WebSocketHostId> {
        let host = WebSocketHost::bind_with_options(addr, options)?;
        Ok(WebSocketHostId(insert_slot(
            &mut self.websocket_hosts,
            host,
        )))
    }

    pub fn connect_websocket(&mut self, url: impl AsRef<str>) -> NetResult<WebSocketConnectionId> {
        let connection = WebSocketConnection::connect(url)?;
        Ok(WebSocketConnectionId(insert_slot(
            &mut self.websocket_connections,
            connection,
        )))
    }

    pub fn connect_websocket_with_options(
        &mut self,
        url: impl AsRef<str>,
        options: WebSocketConnectOptions,
    ) -> NetResult<WebSocketConnectionId> {
        let connection = WebSocketConnection::connect_with_options(url, options)?;
        Ok(WebSocketConnectionId(insert_slot(
            &mut self.websocket_connections,
            connection,
        )))
    }

    pub fn reconnect_websocket(
        &mut self,
        id: WebSocketConnectionId,
        url: impl AsRef<str>,
        options: WebSocketConnectOptions,
    ) -> NetResult<()> {
        let connection = WebSocketConnection::connect_with_options(url, options)?;
        let slot = self
            .websocket_connections
            .get_mut(id.0 as usize)
            .ok_or_else(|| {
                NetError::new(NetErrorKind::MissingHandle, "missing websocket connection")
            })?;
        *slot = Some(connection);
        Ok(())
    }

    pub fn tcp_host_addr(&self, id: TcpHostId) -> NetResult<SocketAddr> {
        Ok(self.tcp_host(id)?.local_addr())
    }

    pub fn tcp_peer_addr(&self, id: TcpConnectionId) -> NetResult<SocketAddr> {
        Ok(self.tcp_connection(id)?.peer_addr())
    }

    pub fn udp_addr(&self, id: UdpEndpointId) -> NetResult<SocketAddr> {
        Ok(self.udp_endpoint(id)?.local_addr())
    }

    pub fn websocket_host_addr(&self, id: WebSocketHostId) -> NetResult<SocketAddr> {
        Ok(self.websocket_host(id)?.local_addr())
    }

    pub fn tcp_send(&mut self, id: TcpConnectionId, bytes: &[u8]) -> NetResult<usize> {
        self.tcp_connection_mut(id)?.write(bytes)
    }

    pub fn tcp_send_frame(&mut self, id: TcpConnectionId, bytes: &[u8]) -> NetResult<()> {
        self.tcp_connection_mut(id)?.write_frame(bytes)
    }

    pub fn tcp_send_handshake(
        &mut self,
        id: TcpConnectionId,
        handshake: &NetHandshake,
    ) -> NetResult<()> {
        self.tcp_connection_mut(id)?.write_handshake(handshake)
    }

    pub fn tcp_send_heartbeat_ping(&mut self, id: TcpConnectionId) -> NetResult<()> {
        self.tcp_connection_mut(id)?.write_frame(heartbeat_ping())
    }

    pub fn tcp_send_heartbeat_pong(&mut self, id: TcpConnectionId) -> NetResult<()> {
        self.tcp_connection_mut(id)?.write_frame(heartbeat_pong())
    }

    pub fn udp_send_to<A: ToSocketAddrs>(
        &self,
        id: UdpEndpointId,
        bytes: &[u8],
        addr: A,
    ) -> NetResult<usize> {
        self.udp_endpoint(id)?.send_to(bytes, addr)
    }

    pub fn websocket_send_text(
        &mut self,
        id: WebSocketConnectionId,
        text: impl Into<String>,
    ) -> NetResult<()> {
        self.websocket_connection_mut(id)?.send_text(text)
    }

    pub fn websocket_send_binary(
        &mut self,
        id: WebSocketConnectionId,
        bytes: Vec<u8>,
    ) -> NetResult<()> {
        self.websocket_connection_mut(id)?.send_binary(bytes)
    }

    pub fn websocket_send_compressed_text(
        &mut self,
        id: WebSocketConnectionId,
        text: impl Into<String>,
    ) -> NetResult<()> {
        self.websocket_connection_mut(id)?
            .send_compressed_text(text)
    }

    pub fn websocket_send_compressed_binary(
        &mut self,
        id: WebSocketConnectionId,
        bytes: Vec<u8>,
    ) -> NetResult<()> {
        self.websocket_connection_mut(id)?
            .send_compressed_binary(bytes)
    }

    pub fn websocket_send_variant(
        &mut self,
        id: WebSocketConnectionId,
        value: &Variant,
    ) -> NetResult<()> {
        self.websocket_connection_mut(id)?.send_variant(value)
    }

    pub fn websocket_send_ping(
        &mut self,
        id: WebSocketConnectionId,
        bytes: Vec<u8>,
    ) -> NetResult<()> {
        self.websocket_connection_mut(id)?.send_ping(bytes)
    }

    pub fn websocket_send_heartbeat_ping(&mut self, id: WebSocketConnectionId) -> NetResult<()> {
        self.websocket_connection_mut(id)?.send_heartbeat_ping()
    }

    pub fn websocket_send_pong(
        &mut self,
        id: WebSocketConnectionId,
        bytes: Vec<u8>,
    ) -> NetResult<()> {
        self.websocket_connection_mut(id)?.send_pong(bytes)
    }

    pub fn websocket_close(&mut self, id: WebSocketConnectionId) -> NetResult<()> {
        self.websocket_connection_mut(id)?.close()
    }

    pub fn websocket_close_with_reason(
        &mut self,
        id: WebSocketConnectionId,
        code: u16,
        reason: impl Into<String>,
    ) -> NetResult<()> {
        self.websocket_connection_mut(id)?
            .close_with_reason(code, reason)
    }

    pub fn remove_tcp_host(&mut self, id: TcpHostId) -> bool {
        remove_slot(&mut self.tcp_hosts, id.0)
    }

    pub fn remove_tcp_connection(&mut self, id: TcpConnectionId) -> bool {
        remove_slot(&mut self.tcp_connections, id.0)
    }

    pub fn remove_udp(&mut self, id: UdpEndpointId) -> bool {
        remove_slot(&mut self.udp_endpoints, id.0)
    }

    pub fn remove_websocket_host(&mut self, id: WebSocketHostId) -> bool {
        remove_slot(&mut self.websocket_hosts, id.0)
    }

    pub fn remove_websocket_connection(&mut self, id: WebSocketConnectionId) -> bool {
        remove_slot(&mut self.websocket_connections, id.0)
    }

    pub fn poll_events(&mut self, max_per_socket: usize, max_bytes: usize) -> Vec<NetworkEvent> {
        let mut events = Vec::new();
        self.poll_accepts(max_per_socket, &mut events);
        self.poll_websocket_accepts(max_per_socket, &mut events);
        self.poll_tcp_data(max_per_socket, max_bytes, &mut events);
        self.poll_udp_packets(max_per_socket, max_bytes, &mut events);
        self.poll_websocket_messages(max_per_socket, max_bytes, &mut events);
        events
    }

    pub fn poll_variant_events(
        &mut self,
        max_per_socket: usize,
        max_bytes: usize,
    ) -> Vec<NetworkEvent> {
        let mut events = Vec::new();
        self.poll_accepts(max_per_socket, &mut events);
        self.poll_websocket_accepts(max_per_socket, &mut events);
        self.poll_tcp_data(max_per_socket, max_bytes, &mut events);
        self.poll_udp_packets(max_per_socket, max_bytes, &mut events);
        self.poll_websocket_variants(max_per_socket, max_bytes, &mut events);
        events
    }

    pub fn poll_frame_events(
        &mut self,
        max_per_socket: usize,
        max_frame_bytes: usize,
    ) -> Vec<NetworkEvent> {
        let mut events = Vec::new();
        self.poll_accepts(max_per_socket, &mut events);
        self.poll_websocket_accepts(max_per_socket, &mut events);
        self.poll_tcp_frames(max_per_socket, max_frame_bytes, &mut events);
        self.poll_udp_packets(max_per_socket, max_frame_bytes, &mut events);
        self.poll_websocket_messages(max_per_socket, max_frame_bytes, &mut events);
        events
    }

    fn poll_accepts(&mut self, max_per_socket: usize, events: &mut Vec<NetworkEvent>) {
        for host_index in 0..self.tcp_hosts.len() {
            let Some(host) = self.tcp_hosts[host_index].as_ref() else {
                continue;
            };
            for _ in 0..max_per_socket {
                match host.accept_event() {
                    Ok(Some((connection, event))) => {
                        let id =
                            TcpConnectionId(insert_slot(&mut self.tcp_connections, connection));
                        events.push(NetworkEvent {
                            source: NetSource::TcpConnection(id),
                            event,
                        });
                    }
                    Ok(None) => break,
                    Err(err) => {
                        events.push(net_error_event(
                            NetSource::TcpHost(TcpHostId(host_index as u32)),
                            "tcp_accept",
                            err,
                        ));
                        break;
                    }
                }
            }
        }
    }

    fn poll_tcp_data(
        &mut self,
        max_per_socket: usize,
        max_bytes: usize,
        events: &mut Vec<NetworkEvent>,
    ) {
        for i in 0..self.tcp_connections.len() {
            let Some(connection) = self.tcp_connections[i].as_mut() else {
                continue;
            };
            let id = TcpConnectionId(i as u32);
            for _ in 0..max_per_socket {
                match connection.poll_event(max_bytes) {
                    Ok(Some(event)) => {
                        let disconnected = matches!(event, NetEvent::TcpDisconnected { .. });
                        events.push(NetworkEvent {
                            source: NetSource::TcpConnection(id),
                            event,
                        });
                        if disconnected {
                            self.tcp_connections[i] = None;
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        events.push(net_error_event(
                            NetSource::TcpConnection(id),
                            "tcp_recv",
                            err,
                        ));
                        break;
                    }
                }
            }
        }
    }

    fn poll_tcp_frames(
        &mut self,
        max_per_socket: usize,
        max_frame_bytes: usize,
        events: &mut Vec<NetworkEvent>,
    ) {
        for i in 0..self.tcp_connections.len() {
            let Some(connection) = self.tcp_connections[i].as_mut() else {
                continue;
            };
            let id = TcpConnectionId(i as u32);
            for _ in 0..max_per_socket {
                match connection.poll_frame_event(max_frame_bytes) {
                    Ok(Some(event)) => {
                        let disconnected = matches!(event, NetEvent::TcpDisconnected { .. });
                        events.push(NetworkEvent {
                            source: NetSource::TcpConnection(id),
                            event,
                        });
                        if disconnected {
                            self.tcp_connections[i] = None;
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        events.push(net_error_event(
                            NetSource::TcpConnection(id),
                            "tcp_frame",
                            err,
                        ));
                        break;
                    }
                }
            }
        }
    }

    fn poll_udp_packets(
        &mut self,
        max_per_socket: usize,
        max_bytes: usize,
        events: &mut Vec<NetworkEvent>,
    ) {
        for i in 0..self.udp_endpoints.len() {
            let Some(endpoint) = self.udp_endpoints[i].as_ref() else {
                continue;
            };
            let id = UdpEndpointId(i as u32);
            for _ in 0..max_per_socket {
                match endpoint.poll_event(max_bytes) {
                    Ok(Some(event)) => events.push(NetworkEvent {
                        source: NetSource::UdpEndpoint(id),
                        event,
                    }),
                    Ok(None) => break,
                    Err(err) => {
                        events.push(net_error_event(NetSource::UdpEndpoint(id), "udp_recv", err));
                        break;
                    }
                }
            }
        }
    }

    fn poll_websocket_accepts(&mut self, max_per_socket: usize, events: &mut Vec<NetworkEvent>) {
        for host_index in 0..self.websocket_hosts.len() {
            let Some(host) = self.websocket_hosts[host_index].as_ref() else {
                continue;
            };
            for _ in 0..max_per_socket {
                match host.accept_event() {
                    Ok(Some((connection, event))) => {
                        let id = WebSocketConnectionId(insert_slot(
                            &mut self.websocket_connections,
                            connection,
                        ));
                        events.push(NetworkEvent {
                            source: NetSource::WebSocketConnection(id),
                            event,
                        });
                    }
                    Ok(None) => break,
                    Err(err) => {
                        events.push(net_error_event(
                            NetSource::WebSocketHost(WebSocketHostId(host_index as u32)),
                            "websocket_accept",
                            err,
                        ));
                        break;
                    }
                }
            }
        }
    }

    fn poll_websocket_messages(
        &mut self,
        max_per_socket: usize,
        max_bytes: usize,
        events: &mut Vec<NetworkEvent>,
    ) {
        for i in 0..self.websocket_connections.len() {
            let Some(connection) = self.websocket_connections[i].as_mut() else {
                continue;
            };
            let id = WebSocketConnectionId(i as u32);
            for _ in 0..max_per_socket {
                match connection.poll_event(max_bytes) {
                    Ok(Some(event)) => {
                        let disconnected = matches!(event, NetEvent::WebSocketClosed { .. });
                        events.push(NetworkEvent {
                            source: NetSource::WebSocketConnection(id),
                            event,
                        });
                        if disconnected {
                            self.websocket_connections[i] = None;
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        events.push(net_error_event(
                            NetSource::WebSocketConnection(id),
                            "websocket_recv",
                            err,
                        ));
                        break;
                    }
                }
            }
        }
    }

    fn poll_websocket_variants(
        &mut self,
        max_per_socket: usize,
        max_bytes: usize,
        events: &mut Vec<NetworkEvent>,
    ) {
        for i in 0..self.websocket_connections.len() {
            let Some(connection) = self.websocket_connections[i].as_mut() else {
                continue;
            };
            let id = WebSocketConnectionId(i as u32);
            for _ in 0..max_per_socket {
                match connection.poll_variant_event(max_bytes) {
                    Ok(Some(event)) => {
                        let disconnected = matches!(event, NetEvent::WebSocketClosed { .. });
                        events.push(NetworkEvent {
                            source: NetSource::WebSocketConnection(id),
                            event,
                        });
                        if disconnected {
                            self.websocket_connections[i] = None;
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        events.push(net_error_event(
                            NetSource::WebSocketConnection(id),
                            "websocket_variant",
                            err,
                        ));
                        break;
                    }
                }
            }
        }
    }

    fn tcp_host(&self, id: TcpHostId) -> NetResult<&TcpHost> {
        get_slot(&self.tcp_hosts, id.0, "tcp host")
    }

    fn tcp_connection(&self, id: TcpConnectionId) -> NetResult<&TcpConnection> {
        get_slot(&self.tcp_connections, id.0, "tcp connection")
    }

    fn tcp_connection_mut(&mut self, id: TcpConnectionId) -> NetResult<&mut TcpConnection> {
        get_slot_mut(&mut self.tcp_connections, id.0, "tcp connection")
    }

    fn udp_endpoint(&self, id: UdpEndpointId) -> NetResult<&UdpEndpoint> {
        get_slot(&self.udp_endpoints, id.0, "udp endpoint")
    }

    fn websocket_host(&self, id: WebSocketHostId) -> NetResult<&WebSocketHost> {
        get_slot(&self.websocket_hosts, id.0, "websocket host")
    }

    fn websocket_connection_mut(
        &mut self,
        id: WebSocketConnectionId,
    ) -> NetResult<&mut WebSocketConnection> {
        get_slot_mut(
            &mut self.websocket_connections,
            id.0,
            "websocket connection",
        )
    }
}

impl Default for NetworkWorld {
    fn default() -> Self {
        Self::new()
    }
}

fn net_error_event(source: NetSource, op: &str, err: NetError) -> NetworkEvent {
    NetworkEvent {
        source,
        event: NetEvent::NetError {
            op: op.to_string(),
            message: err.to_string(),
        },
    }
}
