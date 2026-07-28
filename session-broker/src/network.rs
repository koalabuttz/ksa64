use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use ksa64_presentation::{
    decode_typed_payload, encode_typed_payload, parse_kps1_frame, write_kps1_frame, CursorError,
    Kps1Header, Kps1SequenceCursor, PresentationCursors, PresentationErrorView,
    PresentationHandshake, PresentationMessageKind, PresentationPace, PresentationPayload,
    PresentationRole, SealedEvidenceChunk, SealedEvidenceMetadata,
    KPS1_CAPABILITY_GLOBAL_DISPLAY_V1, KPS1_EVIDENCE_CHUNK_DATA_MAX_LENGTH, KPS1_FLAG_RESPONSE,
    KPS1_HEADER_LENGTH, KPS1_MAX_PAYLOAD_LENGTH,
};
use tungstenite::{
    handshake::server::create_response,
    http::{
        header::{CONNECTION, ORIGIN, SEC_WEBSOCKET_PROTOCOL, UPGRADE},
        HeaderName, HeaderValue, Request, Response, StatusCode, Version,
    },
    protocol::{Role, WebSocket, WebSocketConfig},
    Message,
};

use crate::{
    BrokerError, BrokerPublication, BrowserAdmission, BrowserAdmissionController,
    BrowserAdmissionError, BrowserHandshake, BrowserLaunchToken, BrowserServiceConfig,
    SessionBrokerHandle, LAN_CLIENT_ID_MASK, LAN_CLIENT_ID_PREFIX,
};

pub const PRESENTATION_WEBSOCKET_PATH: &str = "/api/presentation/v1";
pub const MAX_HTTP_HEADER_LENGTH: usize = 16 * 1024;
pub const MAX_HTTP_HEADERS: usize = 64;
pub const MAX_STATIC_ASSETS: usize = 128;
pub const MAX_STATIC_ASSET_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_CONTROL_ADVANCE_RELEASES: u32 = 4_096;
pub const SOCKET_POLL_MILLIS: u64 = 100;
pub const PRESENTATION_ERROR_RESYNC_REQUIRED: u16 = 1_001;
pub const PRESENTATION_ERROR_CURSOR_AHEAD: u16 = 1_002;
pub const SUPPORTED_PRESENTATION_CAPABILITIES: u64 = KPS1_CAPABILITY_GLOBAL_DISPLAY_V1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkServiceError {
    Config,
    Bind,
    Thread,
    Io,
    Http,
    WebSocket,
    Admission(BrowserAdmissionError),
    Protocol,
    ProtocolFrame,
    ProtocolSequence,
    ProtocolPayload,
    ProtocolControl,
    ProtocolEncode(PresentationMessageKind),
    ProtocolFrameWrite,
    ProtocolSequenceOverflow,
    Broker(BrokerError),
    StaticAssets,
}

#[derive(Clone, Debug)]
pub struct StaticAsset {
    pub content_type: &'static str,
    pub cache_control: &'static str,
    pub body: Arc<[u8]>,
}

pub trait StaticAssetProvider: Send + Sync + 'static {
    fn lookup(&self, path: &str) -> Option<StaticAsset>;
}

#[derive(Clone, Debug, Default)]
pub struct EmbeddedStaticAssets {
    assets: Vec<(String, StaticAsset)>,
    total_bytes: usize,
}

impl EmbeddedStaticAssets {
    pub fn insert(
        &mut self,
        path: impl Into<String>,
        content_type: &'static str,
        cache_control: &'static str,
        body: impl Into<Arc<[u8]>>,
    ) -> Result<(), NetworkServiceError> {
        let path = path.into();
        let body = body.into();
        if !valid_static_path(&path)
            || !valid_static_header_value(content_type)
            || !valid_static_header_value(cache_control)
            || body.is_empty()
            || self.assets.len() >= MAX_STATIC_ASSETS
            || self.assets.iter().any(|(existing, _)| existing == &path)
        {
            return Err(NetworkServiceError::StaticAssets);
        }
        let total = self
            .total_bytes
            .checked_add(body.len())
            .ok_or(NetworkServiceError::StaticAssets)?;
        if total > MAX_STATIC_ASSET_BYTES {
            return Err(NetworkServiceError::StaticAssets);
        }
        self.total_bytes = total;
        self.assets.push((
            path,
            StaticAsset {
                content_type,
                cache_control,
                body,
            },
        ));
        self.assets.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(())
    }
}

impl StaticAssetProvider for EmbeddedStaticAssets {
    fn lookup(&self, path: &str) -> Option<StaticAsset> {
        self.assets
            .binary_search_by(|(candidate, _)| candidate.as_str().cmp(path))
            .ok()
            .map(|index| self.assets[index].1.clone())
    }
}

#[derive(Default)]
pub struct NoStaticAssets;

impl StaticAssetProvider for NoStaticAssets {
    fn lookup(&self, _path: &str) -> Option<StaticAsset> {
        None
    }
}

pub struct LoopbackWebService {
    local_addr: SocketAddr,
    launch_subprotocol: String,
    shutdown: Arc<AtomicBool>,
    raw_connections: Arc<AtomicUsize>,
    last_connection_error: Arc<Mutex<Option<NetworkServiceError>>>,
    listener: Option<JoinHandle<()>>,
}

impl LoopbackWebService {
    pub fn start(
        config: BrowserServiceConfig,
        token: BrowserLaunchToken,
        session_nonce: u64,
        broker: Arc<SessionBrokerHandle>,
        assets: Arc<dyn StaticAssetProvider>,
    ) -> Result<Self, NetworkServiceError> {
        config.validate().map_err(|_| NetworkServiceError::Config)?;
        let max_network_connections = usize::from(config.max_connections);
        if session_nonce == 0 {
            return Err(NetworkServiceError::Config);
        }
        let listener = TcpListener::bind(config.bind).map_err(|_| NetworkServiceError::Bind)?;
        let local_addr = listener
            .local_addr()
            .map_err(|_| NetworkServiceError::Bind)?;
        if !local_addr.ip().is_loopback() {
            return Err(NetworkServiceError::Config);
        }
        listener
            .set_nonblocking(true)
            .map_err(|_| NetworkServiceError::Io)?;
        let admission = BrowserAdmissionController::new(config, token)
            .map_err(|_| NetworkServiceError::Config)?;
        let launch_subprotocol = admission.launch_subprotocol();
        let admission = Arc::new(Mutex::new(admission));
        let shutdown = Arc::new(AtomicBool::new(false));
        let raw_connections = Arc::new(AtomicUsize::new(0));
        let service_started = Instant::now();
        let last_connection_error = Arc::new(Mutex::new(None));
        let thread_last_connection_error = last_connection_error.clone();
        let listener_shutdown = shutdown.clone();
        let thread_admission = admission.clone();
        let thread_raw_connections = raw_connections.clone();
        let listener_thread = thread::Builder::new()
            .name("ksa64-loopback-web".to_owned())
            .spawn(move || {
                run_listener(
                    listener,
                    ListenerContext {
                        shutdown: listener_shutdown,
                        admission: thread_admission,
                        raw_connections: thread_raw_connections,
                        max_network_connections,
                        service_started,
                        session_nonce,
                        broker,
                        assets,
                        last_connection_error: thread_last_connection_error,
                    },
                );
            })
            .map_err(|_| NetworkServiceError::Thread)?;
        Ok(Self {
            local_addr,
            launch_subprotocol,
            shutdown,
            raw_connections,
            last_connection_error,
            listener: Some(listener_thread),
        })
    }

    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn launch_subprotocol(&self) -> &str {
        &self.launch_subprotocol
    }

    pub fn active_transport_connections(&self) -> usize {
        self.raw_connections.load(Ordering::Acquire)
    }

    pub fn last_connection_error(&self) -> Option<NetworkServiceError> {
        self.last_connection_error
            .lock()
            .ok()
            .and_then(|error| *error)
    }

    pub fn shutdown(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(listener) = self.listener.take() {
            let _ = listener.join();
        }
    }
}

impl Drop for LoopbackWebService {
    fn drop(&mut self) {
        self.stop();
    }
}

struct ListenerContext {
    shutdown: Arc<AtomicBool>,
    admission: Arc<Mutex<BrowserAdmissionController>>,
    raw_connections: Arc<AtomicUsize>,
    max_network_connections: usize,
    service_started: Instant,
    session_nonce: u64,
    broker: Arc<SessionBrokerHandle>,
    assets: Arc<dyn StaticAssetProvider>,
    last_connection_error: Arc<Mutex<Option<NetworkServiceError>>>,
}

fn run_listener(listener: TcpListener, context: ListenerContext) {
    let mut connections: Vec<JoinHandle<()>> = Vec::new();
    while !context.shutdown.load(Ordering::Acquire) {
        let mut index = 0;
        while index < connections.len() {
            if connections[index].is_finished() {
                let handle = connections.swap_remove(index);
                let _ = handle.join();
            } else {
                index += 1;
            }
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                let reserved = context.raw_connections.fetch_update(
                    Ordering::AcqRel,
                    Ordering::Acquire,
                    |active| (active < context.max_network_connections).then_some(active + 1),
                );
                if reserved.is_err() {
                    let _ = stream.set_write_timeout(Some(Duration::from_millis(250)));
                    let _ = write_simple_response(
                        &mut stream,
                        StatusCode::SERVICE_UNAVAILABLE,
                        "text/plain",
                        b"connection limit",
                    );
                    continue;
                }
                let permit = RawConnectionPermit {
                    active: context.raw_connections.clone(),
                };
                let connection_shutdown = context.shutdown.clone();
                let connection_admission = context.admission.clone();
                let connection_broker = context.broker.clone();
                let connection_assets = context.assets.clone();
                let connection_last_error = context.last_connection_error.clone();
                if let Ok(handle) = thread::Builder::new()
                    .name("ksa64-loopback-client".to_owned())
                    .spawn(move || {
                        let _permit = permit;
                        if let Err(error) = handle_connection(
                            stream,
                            connection_shutdown,
                            connection_admission,
                            context.service_started,
                            context.session_nonce,
                            connection_broker,
                            connection_assets,
                        ) {
                            if let Ok(mut last_error) = connection_last_error.lock() {
                                *last_error = Some(error);
                            }
                        }
                    })
                {
                    connections.push(handle);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(_) => break,
        }
    }
    for connection in connections {
        let _ = connection.join();
    }
}

struct RawConnectionPermit {
    active: Arc<AtomicUsize>,
}

impl Drop for RawConnectionPermit {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::AcqRel);
    }
}

fn handle_connection(
    mut stream: TcpStream,
    shutdown: Arc<AtomicBool>,
    admission: Arc<Mutex<BrowserAdmissionController>>,
    service_started: Instant,
    session_nonce: u64,
    broker: Arc<SessionBrokerHandle>,
    assets: Arc<dyn StaticAssetProvider>,
) -> Result<(), NetworkServiceError> {
    stream
        .set_nonblocking(false)
        .map_err(|_| NetworkServiceError::Io)?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|_| NetworkServiceError::Io)?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|_| NetworkServiceError::Io)?;
    let parsed = read_http_request(&mut stream)?;
    if !is_websocket_upgrade(&parsed.request) {
        return serve_static(&mut stream, &parsed.request, assets.as_ref());
    }
    if parsed.path != PRESENTATION_WEBSOCKET_PATH {
        write_simple_response(
            &mut stream,
            StatusCode::NOT_FOUND,
            "text/plain",
            b"not found",
        )?;
        return Ok(());
    }
    let origin = parsed
        .request
        .headers()
        .get(ORIGIN)
        .and_then(|value| value.to_str().ok());
    let protocol_header = parsed
        .request
        .headers()
        .get(SEC_WEBSOCKET_PROTOCOL)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let offered: Vec<&str> = protocol_header
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect();
    let admission_result = admission
        .lock()
        .map_err(|_| NetworkServiceError::Admission(BrowserAdmissionError::ConnectionLimit))?
        .admit(BrowserHandshake {
            origin,
            request_target_query: parsed.query.as_deref(),
            offered_subprotocols: &offered,
        });
    let admitted = match admission_result {
        Ok(admitted) => admitted,
        Err(error) => {
            let status = match error {
                BrowserAdmissionError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
                BrowserAdmissionError::ConnectionLimit
                | BrowserAdmissionError::OutstandingLimit => StatusCode::SERVICE_UNAVAILABLE,
                _ => StatusCode::FORBIDDEN,
            };
            write_simple_response(&mut stream, status, "text/plain", b"websocket rejected")?;
            return Err(NetworkServiceError::Admission(error));
        }
    };
    let selected = offered[admitted.selected_subprotocol_index].to_owned();
    let result = upgrade_and_run(
        stream,
        parsed,
        &selected,
        admitted,
        shutdown,
        admission.clone(),
        service_started,
        session_nonce,
        broker,
    );
    if let Ok(mut guard) = admission.lock() {
        let _ = guard.disconnect(admitted.connection_id);
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn upgrade_and_run(
    mut stream: TcpStream,
    parsed: ParsedHttpRequest,
    selected_subprotocol: &str,
    admitted: BrowserAdmission,
    shutdown: Arc<AtomicBool>,
    admission: Arc<Mutex<BrowserAdmissionController>>,
    service_started: Instant,
    session_nonce: u64,
    broker: Arc<SessionBrokerHandle>,
) -> Result<(), NetworkServiceError> {
    let mut response = create_response(&parsed.request).map_err(|_| NetworkServiceError::Http)?;
    response.headers_mut().insert(
        SEC_WEBSOCKET_PROTOCOL,
        HeaderValue::from_str(selected_subprotocol).map_err(|_| NetworkServiceError::Http)?,
    );
    write_http_response(&mut stream, &response)?;
    stream
        .set_read_timeout(Some(Duration::from_millis(SOCKET_POLL_MILLIS)))
        .map_err(|_| NetworkServiceError::Io)?;
    let prefixed = PrefixedStream::new(stream, parsed.leftover);
    let mut websocket_config = WebSocketConfig::default();
    let maximum_message = KPS1_HEADER_LENGTH + KPS1_MAX_PAYLOAD_LENGTH;
    websocket_config.max_message_size = Some(maximum_message);
    websocket_config.max_frame_size = Some(maximum_message);
    let mut socket = WebSocket::from_raw_socket(prefixed, Role::Server, Some(websocket_config));
    run_websocket(
        &mut socket,
        shutdown,
        admission,
        service_started,
        session_nonce,
        broker,
        admitted,
    )
}

fn run_websocket(
    socket: &mut WebSocket<PrefixedStream>,
    shutdown: Arc<AtomicBool>,
    admission: Arc<Mutex<BrowserAdmissionController>>,
    service_started: Instant,
    session_nonce: u64,
    broker: Arc<SessionBrokerHandle>,
    _admitted: BrowserAdmission,
) -> Result<(), NetworkServiceError> {
    let first = read_binary(socket, &shutdown)?;
    account_command(&admission, service_started.elapsed())?;
    let handshake_result = (|| {
        let first_frame = parse_kps1_frame(&first).map_err(|_| NetworkServiceError::Protocol)?;
        if first_frame.header.kind != PresentationMessageKind::HandshakeRequest
            || first_frame.header.session_nonce != 0
            || first_frame.header.sequence != 1
        {
            return Err(NetworkServiceError::Protocol);
        }
        let PresentationPayload::HandshakeRequest(handshake) = decode_typed_payload(
            first_frame.header.kind,
            first_frame.payload,
            presentation_role_from_handshake(first_frame.payload)?,
        )
        .map_err(|_| NetworkServiceError::Protocol)?
        else {
            return Err(NetworkServiceError::Protocol);
        };
        if handshake.client_instance & LAN_CLIENT_ID_MASK == LAN_CLIENT_ID_PREFIX {
            return Err(NetworkServiceError::Protocol);
        }
        let attached = broker
            .attach(
                handshake.client_instance,
                session_nonce,
                handshake.role,
                handshake.cursors,
            )
            .map_err(NetworkServiceError::Broker)?;
        Ok((first_frame.header.correlation_id, handshake, attached))
    })();
    complete_command(&admission);
    let (handshake_correlation, handshake, attached) = handshake_result?;
    let _disconnect = BrokerDisconnectGuard {
        broker: broker.as_ref(),
        client_id: handshake.client_instance,
    };

    let capability_mask = handshake.capability_mask & SUPPORTED_PRESENTATION_CAPABILITIES;
    let mut outbound_sequence = 1_u64;
    let response = PresentationPayload::HandshakeResponse(PresentationHandshake {
        role: handshake.role,
        client_instance: handshake.client_instance,
        capability_mask,
        cursors: attached.cursors,
    });
    send_payload(
        socket,
        &response,
        handshake.role,
        session_nonce,
        &mut outbound_sequence,
        handshake_correlation,
        KPS1_FLAG_RESPONSE,
    )?;
    let mut inbound_sequence =
        Kps1SequenceCursor::new(session_nonce, 2).map_err(|_| NetworkServiceError::Protocol)?;
    let mut client_cursors = handshake.cursors;
    let mut evidence_sent = false;
    poll_and_send_websocket_publication(
        socket,
        handshake.client_instance,
        handshake.role,
        session_nonce,
        &broker,
        &mut outbound_sequence,
        &mut client_cursors,
        &mut evidence_sent,
        0,
    )?;

    while !shutdown.load(Ordering::Acquire) {
        let message = match socket.read() {
            Ok(message) => message,
            Err(tungstenite::Error::Io(error))
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => break,
            Err(_) => return Err(NetworkServiceError::WebSocket),
        };
        let bytes = match message {
            Message::Binary(bytes) => bytes,
            Message::Ping(payload) => {
                socket
                    .send(Message::Pong(payload))
                    .map_err(|_| NetworkServiceError::WebSocket)?;
                continue;
            }
            Message::Pong(_) => continue,
            Message::Close(_) => break,
            Message::Text(_) | Message::Frame(_) => {
                let _ = socket.close(None);
                return Err(NetworkServiceError::Admission(
                    BrowserAdmissionError::TextMessage,
                ));
            }
        };
        account_command(&admission, service_started.elapsed())?;
        let result = handle_client_frame(
            socket,
            bytes.as_ref(),
            handshake.client_instance,
            handshake.role,
            session_nonce,
            &broker,
            &mut inbound_sequence,
            &mut outbound_sequence,
            &mut client_cursors,
            &mut evidence_sent,
            capability_mask,
        );
        complete_command(&admission);
        result?;
    }
    Ok(())
}

struct BrokerDisconnectGuard<'a> {
    broker: &'a SessionBrokerHandle,
    client_id: u64,
}

impl Drop for BrokerDisconnectGuard<'_> {
    fn drop(&mut self) {
        let _ = self.broker.disconnect(self.client_id);
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_client_frame(
    socket: &mut WebSocket<PrefixedStream>,
    bytes: &[u8],
    client_id: u64,
    role: PresentationRole,
    session_nonce: u64,
    broker: &SessionBrokerHandle,
    inbound_sequence: &mut Kps1SequenceCursor,
    outbound_sequence: &mut u64,
    client_cursors: &mut PresentationCursors,
    evidence_sent: &mut bool,
    capability_mask: u64,
) -> Result<(), NetworkServiceError> {
    let frame = parse_kps1_frame(bytes).map_err(|_| NetworkServiceError::ProtocolFrame)?;
    if !frame.header.kind.is_negotiated_by(capability_mask) {
        return Err(NetworkServiceError::ProtocolControl);
    }
    inbound_sequence
        .accept(frame.header)
        .map_err(|_| NetworkServiceError::ProtocolSequence)?;
    let payload = decode_typed_payload(frame.header.kind, frame.payload, role)
        .map_err(|_| NetworkServiceError::ProtocolPayload)?;
    let correlation = frame.header.correlation_id;
    match payload {
        PresentationPayload::ReplayControl(cursors) => {
            *client_cursors = cursors;
        }
        PresentationPayload::LifecycleControl(control) => {
            broker
                .set_lifecycle(client_id, control.requested)
                .map_err(NetworkServiceError::Broker)?;
            if control.bounded_releases > 0 {
                if control.bounded_releases > MAX_CONTROL_ADVANCE_RELEASES {
                    return Err(NetworkServiceError::ProtocolControl);
                }
                broker
                    .advance(client_id, control.bounded_releases)
                    .map_err(NetworkServiceError::Broker)?;
            }
        }
        PresentationPayload::PaceControl(control) => {
            if control.bounded_releases > MAX_CONTROL_ADVANCE_RELEASES {
                return Err(NetworkServiceError::ProtocolControl);
            }
            if control.requested == PresentationPace::SingleStep {
                broker
                    .step_one_release(client_id)
                    .map_err(NetworkServiceError::Broker)?;
            } else {
                broker
                    .set_pace(client_id, control.requested)
                    .map_err(NetworkServiceError::Broker)?;
                if control.bounded_releases > 0 {
                    broker
                        .advance(client_id, control.bounded_releases)
                        .map_err(NetworkServiceError::Broker)?;
                }
            }
        }
        PresentationPayload::ActionIntent(intent) => {
            let receipt = broker
                .submit_action(client_id, intent)
                .map_err(NetworkServiceError::Broker)?;
            send_payload(
                socket,
                &PresentationPayload::ActionReceipt(receipt),
                role,
                session_nonce,
                outbound_sequence,
                correlation,
                KPS1_FLAG_RESPONSE,
            )?;
        }
        PresentationPayload::GlobalDisplayRangeRequest(request) => {
            let publication = broker
                .global_display(client_id, request)
                .map_err(NetworkServiceError::Broker)?
                .ok_or(NetworkServiceError::ProtocolControl)?;
            send_global_display_publication(
                socket,
                publication,
                role,
                session_nonce,
                outbound_sequence,
                correlation,
            )?;
            return Ok(());
        }
        _ => return Err(NetworkServiceError::ProtocolControl),
    }
    poll_and_send_websocket_publication(
        socket,
        client_id,
        role,
        session_nonce,
        broker,
        outbound_sequence,
        client_cursors,
        evidence_sent,
        correlation,
    )
}

#[allow(clippy::too_many_arguments)]
fn poll_and_send_websocket_publication(
    socket: &mut WebSocket<PrefixedStream>,
    client_id: u64,
    role: PresentationRole,
    session_nonce: u64,
    broker: &SessionBrokerHandle,
    outbound_sequence: &mut u64,
    client_cursors: &mut PresentationCursors,
    evidence_sent: &mut bool,
    correlation: u64,
) -> Result<(), NetworkServiceError> {
    let response_flags = if correlation == 0 {
        0
    } else {
        KPS1_FLAG_RESPONSE
    };
    let publication = match broker.poll(client_id, *client_cursors, 256) {
        Ok(publication) => publication,
        Err(BrokerError::Cursor(CursorError::ResyncRequired { oldest_available })) => {
            send_payload(
                socket,
                &PresentationPayload::Error(PresentationErrorView {
                    code: PRESENTATION_ERROR_RESYNC_REQUIRED,
                    fatal: false,
                    detail_identity: u32::try_from(oldest_available).unwrap_or(u32::MAX),
                    message: format!("ResyncRequired: oldest cursor {oldest_available}"),
                }),
                role,
                session_nonce,
                outbound_sequence,
                correlation,
                response_flags,
            )?;
            return Ok(());
        }
        Err(BrokerError::Cursor(CursorError::Ahead { next_available })) => {
            send_payload(
                socket,
                &PresentationPayload::Error(PresentationErrorView {
                    code: PRESENTATION_ERROR_CURSOR_AHEAD,
                    fatal: false,
                    detail_identity: u32::try_from(next_available).unwrap_or(u32::MAX),
                    message: format!("cursor ahead: next cursor {next_available}"),
                }),
                role,
                session_nonce,
                outbound_sequence,
                correlation,
                response_flags,
            )?;
            return Ok(());
        }
        Err(error) => return Err(NetworkServiceError::Broker(error)),
    };
    let evidence = publication.evidence;
    send_publication(
        socket,
        publication,
        role,
        session_nonce,
        outbound_sequence,
        client_cursors,
    )?;
    if !*evidence_sent {
        if let Some(metadata) = evidence {
            if metadata.complete {
                send_websocket_evidence(
                    socket,
                    broker,
                    client_id,
                    role,
                    session_nonce,
                    outbound_sequence,
                    metadata,
                )?;
                *evidence_sent = true;
            }
        }
    }
    Ok(())
}

fn send_global_display_publication(
    socket: &mut WebSocket<PrefixedStream>,
    publication: crate::GlobalDisplayPublication,
    role: PresentationRole,
    session_nonce: u64,
    sequence: &mut u64,
    correlation: u64,
) -> Result<(), NetworkServiceError> {
    send_payload(
        socket,
        &PresentationPayload::GlobalDisplayDefinition(publication.definition),
        role,
        session_nonce,
        sequence,
        0,
        0,
    )?;
    if !publication.samples.is_empty() {
        send_payload(
            socket,
            &PresentationPayload::GlobalDisplaySampleBatch(publication.samples),
            role,
            session_nonce,
            sequence,
            0,
            0,
        )?;
    }
    for path in publication.paths {
        send_payload(
            socket,
            &PresentationPayload::GlobalDisplayPathChunk(path),
            role,
            session_nonce,
            sequence,
            0,
            0,
        )?;
    }
    for transition in publication.transitions {
        send_payload(
            socket,
            &PresentationPayload::GlobalDisplayTransition(transition),
            role,
            session_nonce,
            sequence,
            0,
            0,
        )?;
    }
    send_payload(
        socket,
        &PresentationPayload::GlobalReplayIndex(publication.replay_index),
        role,
        session_nonce,
        sequence,
        0,
        0,
    )?;
    send_payload(
        socket,
        &PresentationPayload::GlobalDisplayCursorState(publication.cursor),
        role,
        session_nonce,
        sequence,
        correlation,
        KPS1_FLAG_RESPONSE,
    )
}

fn send_publication(
    socket: &mut WebSocket<PrefixedStream>,
    publication: BrokerPublication,
    role: PresentationRole,
    session_nonce: u64,
    sequence: &mut u64,
    cursors: &mut PresentationCursors,
) -> Result<(), NetworkServiceError> {
    send_payload(
        socket,
        &PresentationPayload::Snapshot(publication.snapshot.clone()),
        role,
        session_nonce,
        sequence,
        0,
        0,
    )?;
    if let Some(procedure) = publication.procedure {
        send_payload(
            socket,
            &PresentationPayload::Procedure(procedure),
            role,
            session_nonce,
            sequence,
            0,
            0,
        )?;
    }
    if let Some(disposition) = publication.disposition {
        send_payload(
            socket,
            &PresentationPayload::Disposition(disposition),
            role,
            session_nonce,
            sequence,
            0,
            0,
        )?;
    }
    for path in publication.prediction_paths {
        send_payload(
            socket,
            &PresentationPayload::PredictionPath(path),
            role,
            session_nonce,
            sequence,
            0,
            0,
        )?;
    }
    if !publication.events.records.is_empty() {
        send_payload(
            socket,
            &PresentationPayload::EventBatch(
                publication
                    .events
                    .records
                    .iter()
                    .map(|record| record.value)
                    .collect(),
            ),
            role,
            session_nonce,
            sequence,
            0,
            0,
        )?;
    }
    for event in &publication.timeline.records {
        send_payload(
            socket,
            &PresentationPayload::TimelineEvent(event.value.clone()),
            role,
            session_nonce,
            sequence,
            0,
            0,
        )?;
    }
    for receipt in &publication.action_receipts.records {
        send_payload(
            socket,
            &PresentationPayload::ActionReceipt(receipt.value),
            role,
            session_nonce,
            sequence,
            0,
            0,
        )?;
    }
    if !publication.release_samples.records.is_empty() {
        send_payload(
            socket,
            &PresentationPayload::ReleaseSampleBatch(
                publication
                    .release_samples
                    .records
                    .iter()
                    .map(|record| record.value)
                    .collect(),
            ),
            role,
            session_nonce,
            sequence,
            0,
            0,
        )?;
    }
    send_payload(
        socket,
        &PresentationPayload::TransportStatus(publication.transport),
        role,
        session_nonce,
        sequence,
        0,
        0,
    )?;
    if let Some(evidence) = publication.evidence {
        send_payload(
            socket,
            &PresentationPayload::EvidenceMetadata(evidence),
            role,
            session_nonce,
            sequence,
            0,
            0,
        )?;
    }
    cursors.snapshots = publication.snapshot.publication_sequence;
    cursors.events = publication.events.next_cursor;
    cursors.timeline = publication.timeline.next_cursor;
    cursors.action_receipts = publication.action_receipts.next_cursor;
    cursors.release_samples = publication.release_samples.next_cursor;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn send_websocket_evidence(
    socket: &mut WebSocket<PrefixedStream>,
    broker: &SessionBrokerHandle,
    client_id: u64,
    role: PresentationRole,
    session_nonce: u64,
    sequence: &mut u64,
    metadata: SealedEvidenceMetadata,
) -> Result<(), NetworkServiceError> {
    metadata
        .validate()
        .map_err(|_| NetworkServiceError::ProtocolPayload)?;
    let bytes = broker
        .sealed_evidence(client_id)
        .map_err(NetworkServiceError::Broker)?
        .ok_or(NetworkServiceError::ProtocolPayload)?;
    if bytes.len() as u64 != metadata.total_length || crc32_ieee(&bytes) != metadata.evidence_crc32
    {
        return Err(NetworkServiceError::ProtocolPayload);
    }
    let chunk_length =
        usize::try_from(metadata.chunk_length).map_err(|_| NetworkServiceError::ProtocolPayload)?;
    if chunk_length == 0 || chunk_length > KPS1_EVIDENCE_CHUNK_DATA_MAX_LENGTH {
        return Err(NetworkServiceError::ProtocolPayload);
    }
    let chunks = bytes.chunks(chunk_length);
    if chunks.len() != metadata.chunk_count as usize {
        return Err(NetworkServiceError::ProtocolPayload);
    }
    for (index, chunk) in chunks.enumerate() {
        send_payload(
            socket,
            &PresentationPayload::EvidenceChunk(SealedEvidenceChunk {
                evidence_identity: metadata.evidence_identity,
                chunk_index: index as u32,
                chunk_count: metadata.chunk_count,
                logical_offset: (index * chunk_length) as u64,
                bytes: chunk.to_vec(),
            }),
            role,
            session_nonce,
            sequence,
            0,
            0,
        )?;
    }
    Ok(())
}

fn crc32_ieee(input: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in input {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn send_payload(
    socket: &mut WebSocket<PrefixedStream>,
    payload: &PresentationPayload,
    role: PresentationRole,
    session_nonce: u64,
    sequence: &mut u64,
    correlation_id: u64,
    flags: u32,
) -> Result<(), NetworkServiceError> {
    let bytes = encode_typed_payload(payload, role)
        .map_err(|_| NetworkServiceError::ProtocolEncode(payload.kind()))?;
    let header = Kps1Header {
        kind: payload.kind(),
        flags,
        session_nonce,
        sequence: *sequence,
        correlation_id,
        payload_length: bytes.len() as u32,
    };
    let mut frame = vec![0_u8; KPS1_HEADER_LENGTH + bytes.len()];
    write_kps1_frame(header, &bytes, &mut frame)
        .map_err(|_| NetworkServiceError::ProtocolFrameWrite)?;
    socket
        .send(Message::Binary(frame.into()))
        .map_err(|_| NetworkServiceError::WebSocket)?;
    *sequence = sequence
        .checked_add(1)
        .ok_or(NetworkServiceError::ProtocolSequenceOverflow)?;
    Ok(())
}

fn read_binary(
    socket: &mut WebSocket<PrefixedStream>,
    shutdown: &AtomicBool,
) -> Result<Vec<u8>, NetworkServiceError> {
    loop {
        if shutdown.load(Ordering::Acquire) {
            return Err(NetworkServiceError::WebSocket);
        }
        match socket.read() {
            Ok(Message::Binary(bytes)) => return Ok(bytes.to_vec()),
            Ok(Message::Ping(payload)) => socket
                .send(Message::Pong(payload))
                .map_err(|_| NetworkServiceError::WebSocket)?,
            Ok(Message::Pong(_)) => {}
            Ok(Message::Close(_)) => return Err(NetworkServiceError::WebSocket),
            Ok(Message::Text(_) | Message::Frame(_)) => {
                return Err(NetworkServiceError::Admission(
                    BrowserAdmissionError::TextMessage,
                ));
            }
            Err(tungstenite::Error::Io(error))
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return Err(NetworkServiceError::WebSocket),
        }
    }
}

fn account_command(
    admission: &Mutex<BrowserAdmissionController>,
    elapsed: Duration,
) -> Result<(), NetworkServiceError> {
    admission
        .lock()
        .map_err(|_| NetworkServiceError::Admission(BrowserAdmissionError::OutstandingLimit))?
        .begin_command(elapsed.as_millis().min(u128::from(u64::MAX)) as u64)
        .map_err(NetworkServiceError::Admission)
}

fn complete_command(admission: &Mutex<BrowserAdmissionController>) {
    if let Ok(mut admission) = admission.lock() {
        admission.complete_command();
    }
}

fn presentation_role_from_handshake(
    payload: &[u8],
) -> Result<PresentationRole, NetworkServiceError> {
    // PHS1 payload header occupies 12 bytes and role is its first field.
    PresentationRole::from_raw(*payload.get(12).ok_or(NetworkServiceError::Protocol)?)
        .ok_or(NetworkServiceError::Protocol)
}

struct ParsedHttpRequest {
    request: Request<()>,
    path: String,
    query: Option<String>,
    leftover: Vec<u8>,
}

fn read_http_request(stream: &mut TcpStream) -> Result<ParsedHttpRequest, NetworkServiceError> {
    let mut bytes = Vec::with_capacity(2_048);
    let header_end = loop {
        if bytes.len() >= MAX_HTTP_HEADER_LENGTH {
            return Err(NetworkServiceError::Http);
        }
        let mut chunk = [0_u8; 1_024];
        let count = stream
            .read(&mut chunk)
            .map_err(|_| NetworkServiceError::Io)?;
        if count == 0 {
            return Err(NetworkServiceError::Http);
        }
        bytes.extend_from_slice(&chunk[..count]);
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let mut headers = [httparse::EMPTY_HEADER; MAX_HTTP_HEADERS];
    let mut parsed = httparse::Request::new(&mut headers);
    let status = parsed
        .parse(&bytes[..header_end])
        .map_err(|_| NetworkServiceError::Http)?;
    if !status.is_complete() {
        return Err(NetworkServiceError::Http);
    }
    let method = parsed.method.ok_or(NetworkServiceError::Http)?;
    let target = parsed.path.ok_or(NetworkServiceError::Http)?;
    let version = match parsed.version {
        Some(1) => Version::HTTP_11,
        Some(0) => Version::HTTP_10,
        _ => return Err(NetworkServiceError::Http),
    };
    let mut builder = Request::builder()
        .method(method)
        .uri(target)
        .version(version);
    {
        let destination = builder.headers_mut().ok_or(NetworkServiceError::Http)?;
        for header in parsed.headers {
            let name = HeaderName::from_bytes(header.name.as_bytes())
                .map_err(|_| NetworkServiceError::Http)?;
            let value =
                HeaderValue::from_bytes(header.value).map_err(|_| NetworkServiceError::Http)?;
            destination.append(name, value);
        }
    }
    let request = builder.body(()).map_err(|_| NetworkServiceError::Http)?;
    let (path, query) = target
        .split_once('?')
        .map_or((target.to_owned(), None), |(path, query)| {
            (path.to_owned(), Some(query.to_owned()))
        });
    Ok(ParsedHttpRequest {
        request,
        path,
        query,
        leftover: bytes[header_end..].to_vec(),
    })
}

fn is_websocket_upgrade(request: &Request<()>) -> bool {
    let upgrade = request
        .headers()
        .get(UPGRADE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("websocket"));
    let connection = request
        .headers()
        .get(CONNECTION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .any(|token| token.trim().eq_ignore_ascii_case("upgrade"))
        });
    upgrade && connection
}

fn serve_static(
    stream: &mut TcpStream,
    request: &Request<()>,
    assets: &dyn StaticAssetProvider,
) -> Result<(), NetworkServiceError> {
    if request.method() != "GET" && request.method() != "HEAD" {
        write_simple_response(
            stream,
            StatusCode::METHOD_NOT_ALLOWED,
            "text/plain",
            b"method not allowed",
        )?;
        return Ok(());
    }
    let target = request.uri().path();
    if !valid_static_path(target) {
        write_simple_response(stream, StatusCode::BAD_REQUEST, "text/plain", b"bad path")?;
        return Ok(());
    }
    let path = if target == "/" { "/index.html" } else { target };
    let Some(asset) = assets.lookup(path) else {
        write_simple_response(stream, StatusCode::NOT_FOUND, "text/plain", b"not found")?;
        return Ok(());
    };
    let body = if request.method() == "HEAD" {
        &[][..]
    } else {
        asset.body.as_ref()
    };
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: {}\r\nContent-Security-Policy: default-src 'self'; connect-src 'self' ws://127.0.0.1:* ws://[::1]:*; object-src 'none'; base-uri 'none'\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        asset.content_type,
        asset.body.len(),
        asset.cache_control,
    );
    stream
        .write_all(header.as_bytes())
        .and_then(|()| stream.write_all(body))
        .map_err(|_| NetworkServiceError::Io)
}

fn valid_static_header_value(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| matches!(byte, 0x20..=0x7e) && byte != b'\r' && byte != b'\n')
}

fn valid_static_path(path: &str) -> bool {
    path.starts_with('/')
        && !path.contains("..")
        && !path.contains('\\')
        && !path.contains('%')
        && !path.contains('?')
        && !path.contains('#')
}

fn write_simple_response(
    stream: &mut TcpStream,
    status: StatusCode,
    content_type: &str,
    body: &[u8],
) -> Result<(), NetworkServiceError> {
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status.as_u16(),
        status.canonical_reason().unwrap_or("Error"),
        content_type,
        body.len(),
    );
    stream
        .write_all(response.as_bytes())
        .and_then(|()| stream.write_all(body))
        .map_err(|_| NetworkServiceError::Io)
}

fn write_http_response(
    stream: &mut TcpStream,
    response: &Response<()>,
) -> Result<(), NetworkServiceError> {
    let status = response.status();
    let mut bytes = format!(
        "HTTP/1.1 {} {}\r\n",
        status.as_u16(),
        status.canonical_reason().unwrap_or("Switching Protocols")
    )
    .into_bytes();
    for (name, value) in response.headers() {
        bytes.extend_from_slice(name.as_str().as_bytes());
        bytes.extend_from_slice(b": ");
        bytes.extend_from_slice(value.as_bytes());
        bytes.extend_from_slice(b"\r\n");
    }
    bytes.extend_from_slice(b"\r\n");
    stream
        .write_all(&bytes)
        .map_err(|_| NetworkServiceError::Io)
}

struct PrefixedStream {
    stream: TcpStream,
    prefix: Vec<u8>,
    position: usize,
}

impl PrefixedStream {
    fn new(stream: TcpStream, prefix: Vec<u8>) -> Self {
        Self {
            stream,
            prefix,
            position: 0,
        }
    }
}

impl Read for PrefixedStream {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if self.position < self.prefix.len() {
            let count = output.len().min(self.prefix.len() - self.position);
            output[..count].copy_from_slice(&self.prefix[self.position..self.position + count]);
            self.position += count;
            return Ok(count);
        }
        self.stream.read(output)
    }
}

impl Write for PrefixedStream {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        self.stream.write(input)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AuthorityError, BrokerAuthority, WorkerConfig};
    use ksa64_presentation::{
        ActionReceiptView, CursorError, DispositionView, NavigationView, PredictionPathView,
        PredictionSummaryView, PresentationActionIntent, PresentationBatch, PresentationEventView,
        PresentationLifecycle, PresentationQueueStatus, PresentationSession, PresentationStaleness,
        ProcedureView, ReleaseSampleView, RetainedStream, TimelineEventView, TransportStatusView,
        PRESENTATION_MODEL_ID,
    };
    use std::sync::atomic::AtomicU32;
    use tungstenite::client::IntoClientRequest;

    struct NetworkAuthority {
        snapshot: ksa64_presentation::OperationalSnapshot,
        advances: Arc<AtomicU32>,
        events: RetainedStream<PresentationEventView>,
        timeline: RetainedStream<TimelineEventView>,
        receipts: RetainedStream<ActionReceiptView>,
        samples: RetainedStream<ReleaseSampleView>,
    }

    impl NetworkAuthority {
        fn new(advances: Arc<AtomicU32>) -> Self {
            Self {
                snapshot: ksa64_presentation::OperationalSnapshot {
                    presentation_model_identity: PRESENTATION_MODEL_ID,
                    session_definition_identity: 7,
                    publication_sequence: 1,
                    validity_mask: 0,
                    role: PresentationRole::GuidedOperator,
                    lifecycle: PresentationLifecycle::Running,
                    pace: PresentationPace::Fast,
                    release_epoch: 0,
                    release_period_micros: 31_250,
                    frame_identity: 1,
                    mission_time_q16: 0,
                    onboard: NavigationView::default(),
                    ground: NavigationView::default(),
                    prediction: PredictionSummaryView::default(),
                    flight_checksum: 1,
                    command_checksum: 2,
                    procedure_chain: 3,
                    journal_chain: 4,
                    action_chain: 5,
                    staged_load_identity: 0,
                    action_count: 0,
                    rejected_loads: 0,
                    gnss_state: 1,
                    safe: false,
                    truth: None,
                },
                advances,
                events: RetainedStream::new(16).unwrap(),
                timeline: RetainedStream::new(16).unwrap(),
                receipts: RetainedStream::new(16).unwrap(),
                samples: RetainedStream::new(16).unwrap(),
            }
        }
    }

    impl PresentationSession for NetworkAuthority {
        type Error = AuthorityError;
        fn role(&self) -> PresentationRole {
            self.snapshot.role
        }
        fn lifecycle(&self) -> PresentationLifecycle {
            self.snapshot.lifecycle
        }
        fn latest_snapshot(&self) -> ksa64_presentation::OperationalSnapshot {
            self.snapshot.clone()
        }
        fn current_procedure(&self) -> Option<ProcedureView> {
            None
        }
        fn current_disposition(&self) -> Option<DispositionView> {
            None
        }
        fn current_prediction_paths(&self) -> Vec<PredictionPathView> {
            Vec::new()
        }
        fn transport_status(&self) -> TransportStatusView {
            TransportStatusView {
                staleness: PresentationStaleness::Current,
                worker_state: 1,
                finalization_state: 0,
                queue: PresentationQueueStatus::default(),
                last_command_result: 0,
            }
        }
        fn finalization_evidence(&self) -> Option<ksa64_presentation::SealedEvidenceMetadata> {
            None
        }
        fn cursors(&self) -> PresentationCursors {
            PresentationCursors {
                snapshots: self.snapshot.publication_sequence,
                events: self.events.next_cursor(),
                timeline: self.timeline.next_cursor(),
                action_receipts: self.receipts.next_cursor(),
                release_samples: self.samples.next_cursor(),
            }
        }
        fn read_events(
            &self,
            cursor: u64,
            limit: usize,
        ) -> Result<PresentationBatch<PresentationEventView>, CursorError> {
            self.events.read(cursor, limit)
        }
        fn read_timeline(
            &self,
            cursor: u64,
            limit: usize,
        ) -> Result<PresentationBatch<TimelineEventView>, CursorError> {
            self.timeline.read(cursor, limit)
        }
        fn read_action_receipts(
            &self,
            cursor: u64,
            limit: usize,
        ) -> Result<PresentationBatch<ActionReceiptView>, CursorError> {
            self.receipts.read(cursor, limit)
        }
        fn read_release_samples(
            &self,
            cursor: u64,
            limit: usize,
        ) -> Result<PresentationBatch<ReleaseSampleView>, CursorError> {
            self.samples.read(cursor, limit)
        }
        fn submit_action(
            &mut self,
            _intent: PresentationActionIntent,
        ) -> Result<ActionReceiptView, Self::Error> {
            Err(AuthorityError { code: 1 })
        }
    }

    impl BrokerAuthority for NetworkAuthority {
        fn session_nonce(&self) -> u64 {
            77
        }
        fn advance_bounded(&mut self, maximum: u32) -> Result<u32, AuthorityError> {
            self.snapshot.release_epoch = self.snapshot.release_epoch.saturating_add(maximum);
            self.snapshot.publication_sequence = self
                .snapshot
                .publication_sequence
                .saturating_add(u64::from(maximum));
            self.advances.fetch_add(maximum, Ordering::Relaxed);
            Ok(maximum)
        }
        fn set_pace(&mut self, pace: PresentationPace) -> Result<(), AuthorityError> {
            self.snapshot.pace = pace;
            Ok(())
        }
        fn request_lifecycle(
            &mut self,
            requested: PresentationLifecycle,
        ) -> Result<(), AuthorityError> {
            self.snapshot.lifecycle = requested;
            Ok(())
        }
        fn step_one_release(&mut self) -> Result<u32, AuthorityError> {
            self.advance_bounded(1)
        }
    }

    fn service() -> (LoopbackWebService, Arc<SessionBrokerHandle>, Arc<AtomicU32>) {
        let advances = Arc::new(AtomicU32::new(0));
        let authority = NetworkAuthority::new(advances.clone());
        let (service, broker) = service_with_authority(authority, WorkerConfig::default());
        (service, broker, advances)
    }

    fn service_with_authority(
        authority: NetworkAuthority,
        worker_config: WorkerConfig,
    ) -> (LoopbackWebService, Arc<SessionBrokerHandle>) {
        let broker = Arc::new(SessionBrokerHandle::spawn(authority, worker_config).unwrap());
        let config = BrowserServiceConfig::loopback(
            0,
            ["http://127.0.0.1:4173".to_owned()],
            PresentationRole::GuidedOperator,
        )
        .unwrap();
        let mut assets = EmbeddedStaticAssets::default();
        assets
            .insert(
                "/index.html",
                "text/html; charset=utf-8",
                "no-store",
                Arc::<[u8]>::from(&b"<!doctype html>ksa64"[..]),
            )
            .unwrap();
        let service = LoopbackWebService::start(
            config,
            BrowserLaunchToken::from_bytes([0x33; 32]),
            77,
            broker.clone(),
            Arc::new(assets),
        )
        .unwrap();
        (service, broker)
    }

    fn connect(
        service: &LoopbackWebService,
        origin: &str,
        protocol: &str,
    ) -> Option<WebSocket<TcpStream>> {
        let mut request = format!(
            "ws://{}{}",
            service.local_addr(),
            PRESENTATION_WEBSOCKET_PATH
        )
        .into_client_request()
        .unwrap();
        request
            .headers_mut()
            .insert(ORIGIN, HeaderValue::from_str(origin).unwrap());
        request.headers_mut().insert(
            SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_str(protocol).unwrap(),
        );
        let stream = TcpStream::connect(service.local_addr()).unwrap();
        tungstenite::client(request, stream)
            .ok()
            .map(|(socket, _)| socket)
    }

    fn send_handshake(
        socket: &mut WebSocket<TcpStream>,
        client_instance: u64,
        cursors: PresentationCursors,
    ) -> PresentationHandshake {
        send_handshake_request(socket, client_instance, cursors);
        let response = socket.read().unwrap().into_data();
        let decoded = parse_kps1_frame(&response).unwrap();
        assert_eq!(
            decoded.header.kind,
            PresentationMessageKind::HandshakeResponse
        );
        assert_eq!(decoded.header.session_nonce, 77);
        let PresentationPayload::HandshakeResponse(handshake) = decode_typed_payload(
            decoded.header.kind,
            decoded.payload,
            PresentationRole::GuidedOperator,
        )
        .unwrap() else {
            panic!("expected handshake response");
        };
        assert_eq!(handshake.cursors, cursors);

        // A successful attach immediately replays retained records. Drain this
        // initial publication through its required transport-status terminator
        // so callers begin at the next client command boundary.
        loop {
            let publication = socket.read().unwrap().into_data();
            let frame = parse_kps1_frame(&publication).unwrap();
            if frame.header.kind == PresentationMessageKind::TransportStatus {
                break;
            }
        }
        handshake
    }

    fn send_handshake_request(
        socket: &mut WebSocket<TcpStream>,
        client_instance: u64,
        cursors: PresentationCursors,
    ) {
        let payload = encode_typed_payload(
            &PresentationPayload::HandshakeRequest(PresentationHandshake {
                role: PresentationRole::GuidedOperator,
                client_instance,
                capability_mask: 1,
                cursors,
            }),
            PresentationRole::GuidedOperator,
        )
        .unwrap();
        let mut frame = vec![0_u8; KPS1_HEADER_LENGTH + payload.len()];
        write_kps1_frame(
            Kps1Header {
                kind: PresentationMessageKind::HandshakeRequest,
                flags: 0,
                session_nonce: 0,
                sequence: 1,
                correlation_id: 1,
                payload_length: payload.len() as u32,
            },
            &payload,
            &mut frame,
        )
        .unwrap();
        socket.send(Message::Binary(frame.into())).unwrap();
    }

    fn send_client_payload(
        socket: &mut WebSocket<TcpStream>,
        payload: PresentationPayload,
        sequence: u64,
        correlation_id: u64,
    ) {
        let payload_bytes =
            encode_typed_payload(&payload, PresentationRole::GuidedOperator).unwrap();
        let mut frame = vec![0_u8; KPS1_HEADER_LENGTH + payload_bytes.len()];
        write_kps1_frame(
            Kps1Header {
                kind: payload.kind(),
                flags: 0,
                session_nonce: 77,
                sequence,
                correlation_id,
                payload_length: payload_bytes.len() as u32,
            },
            &payload_bytes,
            &mut frame,
        )
        .unwrap();
        socket.send(Message::Binary(frame.into())).unwrap();
    }

    fn raw_status(service: &LoopbackWebService, origin: &str, protocol: &str) -> String {
        let mut stream = TcpStream::connect(service.local_addr()).unwrap();
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nOrigin: {}\r\nSec-WebSocket-Protocol: {}\r\n\r\n",
            PRESENTATION_WEBSOCKET_PATH,
            service.local_addr(),
            origin,
            protocol,
        );
        stream.write_all(request.as_bytes()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut output = [0_u8; 512];
        let count = stream.read(&mut output).unwrap_or(0);
        String::from_utf8_lossy(&output[..count]).into_owned()
    }

    #[test]
    fn static_hook_serves_pwa_shell_and_rejects_traversal() {
        let (service, _, _) = service();
        let mut stream = TcpStream::connect(service.local_addr()).unwrap();
        let request = format!("GET / HTTP/1.1\r\nHost: {}\r\n\r\n", service.local_addr());
        stream.write_all(request.as_bytes()).unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.starts_with("HTTP/1.1 200"));
        assert!(text.ends_with("<!doctype html>ksa64"));

        let mut traversal = TcpStream::connect(service.local_addr()).unwrap();
        let request = format!(
            "GET /../private HTTP/1.1\r\nHost: {}\r\n\r\n",
            service.local_addr()
        );
        traversal.write_all(request.as_bytes()).unwrap();
        let mut response = [0_u8; 256];
        let count = traversal.read(&mut response).unwrap();
        assert!(String::from_utf8_lossy(&response[..count]).starts_with("HTTP/1.1 400"));
    }

    #[test]
    fn static_asset_headers_reject_injection() {
        let mut assets = EmbeddedStaticAssets::default();
        assert_eq!(
            assets.insert(
                "/bad.html",
                "text/html\r\nX-Evil: yes",
                "no-store",
                Arc::<[u8]>::from(&b"x"[..]),
            ),
            Err(NetworkServiceError::StaticAssets)
        );
        assert_eq!(
            assets.insert(
                "/bad-cache.html",
                "text/html",
                "no-store\nX-Evil: yes",
                Arc::<[u8]>::from(&b"x"[..]),
            ),
            Err(NetworkServiceError::StaticAssets)
        );
    }

    #[test]
    fn real_upgrade_rejects_wrong_origin_and_wrong_subprotocol() {
        let (service, _, _) = service();
        assert!(!raw_status(
            &service,
            "http://wrong.invalid",
            service.launch_subprotocol()
        )
        .starts_with("HTTP/1.1 101"));
        assert!(!raw_status(
            &service,
            "http://127.0.0.1:4173",
            "ksa64.presentation.v1.token.bad"
        )
        .starts_with("HTTP/1.1 101"));
    }

    #[test]
    fn binary_session_disconnect_continues_and_same_client_reconnects() {
        let (service, _broker, advances) = service();
        let mut socket = connect(
            &service,
            "http://127.0.0.1:4173",
            service.launch_subprotocol(),
        )
        .unwrap();
        send_handshake(&mut socket, 55, PresentationCursors::default());
        socket.close(None).unwrap();
        let before = advances.load(Ordering::Relaxed);
        let deadline = Instant::now() + Duration::from_millis(500);
        while advances.load(Ordering::Relaxed) <= before && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(advances.load(Ordering::Relaxed) > before);

        let mut reconnected = connect(
            &service,
            "http://127.0.0.1:4173",
            service.launch_subprotocol(),
        )
        .unwrap();
        send_handshake(&mut reconnected, 55, PresentationCursors::default());
    }

    #[test]
    fn reconnect_replays_requested_retained_records_or_reports_resync() {
        let advances = Arc::new(AtomicU32::new(0));
        let mut authority = NetworkAuthority::new(advances);
        for sequence in 1..=3_u64 {
            authority
                .events
                .push(PresentationEventView {
                    sequence,
                    release_epoch: sequence as u32,
                    kind: 1,
                    detail_identity: sequence as u32,
                })
                .unwrap();
        }
        let (service, _broker) = service_with_authority(
            authority,
            WorkerConfig {
                autonomous_pacing: false,
                ..WorkerConfig::default()
            },
        );
        let mut socket = connect(
            &service,
            "http://127.0.0.1:4173",
            service.launch_subprotocol(),
        )
        .unwrap();
        let requested = PresentationCursors {
            events: 2,
            ..PresentationCursors::default()
        };
        send_handshake_request(&mut socket, 0x5151, requested);
        let response = socket.read().unwrap().into_data();
        let frame = parse_kps1_frame(&response).unwrap();
        let PresentationPayload::HandshakeResponse(handshake) = decode_typed_payload(
            frame.header.kind,
            frame.payload,
            PresentationRole::GuidedOperator,
        )
        .unwrap() else {
            panic!("expected handshake response");
        };
        assert_eq!(handshake.cursors, requested);
        let mut replayed = Vec::new();
        loop {
            let bytes = socket.read().unwrap().into_data();
            let frame = parse_kps1_frame(&bytes).unwrap();
            let payload = decode_typed_payload(
                frame.header.kind,
                frame.payload,
                PresentationRole::GuidedOperator,
            )
            .unwrap();
            if let PresentationPayload::EventBatch(events) = payload {
                replayed.extend(events.into_iter().map(|event| event.sequence));
            }
            if frame.header.kind == PresentationMessageKind::TransportStatus {
                break;
            }
        }
        assert_eq!(replayed, vec![2, 3]);
        drop(socket);
        drop(service);

        let advances = Arc::new(AtomicU32::new(0));
        let mut overflowed = NetworkAuthority::new(advances);
        for sequence in 1..=20_u64 {
            overflowed
                .events
                .push(PresentationEventView {
                    sequence,
                    release_epoch: sequence as u32,
                    kind: 1,
                    detail_identity: sequence as u32,
                })
                .unwrap();
        }
        let (service, _broker) = service_with_authority(
            overflowed,
            WorkerConfig {
                autonomous_pacing: false,
                ..WorkerConfig::default()
            },
        );
        let mut socket = connect(
            &service,
            "http://127.0.0.1:4173",
            service.launch_subprotocol(),
        )
        .unwrap();
        send_handshake_request(&mut socket, 0x5252, PresentationCursors::default());
        let _handshake = socket.read().unwrap();
        let error = socket.read().unwrap().into_data();
        let frame = parse_kps1_frame(&error).unwrap();
        let PresentationPayload::Error(error) = decode_typed_payload(
            frame.header.kind,
            frame.payload,
            PresentationRole::GuidedOperator,
        )
        .unwrap() else {
            panic!("expected explicit resynchronization error");
        };
        assert_eq!(error.code, PRESENTATION_ERROR_RESYNC_REQUIRED);
        assert_eq!(error.detail_identity, 5);
        assert!(!error.fatal);
    }

    #[test]
    fn stale_snapshot_cursor_receives_the_latest_coalesced_snapshot() {
        let (service, _, advances) = service();
        let mut socket = connect(
            &service,
            "http://127.0.0.1:4173",
            service.launch_subprotocol(),
        )
        .unwrap();
        send_handshake(&mut socket, 66, PresentationCursors::default());
        let deadline = Instant::now() + Duration::from_millis(500);
        while advances.load(Ordering::Relaxed) < 2 && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(advances.load(Ordering::Relaxed) >= 2);
        send_client_payload(
            &mut socket,
            PresentationPayload::ReplayControl(PresentationCursors::default()),
            2,
            991,
        );
        let response = socket.read().unwrap().into_data();
        let decoded = parse_kps1_frame(&response).unwrap();
        assert_eq!(decoded.header.kind, PresentationMessageKind::Snapshot);
        assert_eq!(decoded.header.correlation_id, 0);
        let PresentationPayload::Snapshot(snapshot) = decode_typed_payload(
            decoded.header.kind,
            decoded.payload,
            PresentationRole::GuidedOperator,
        )
        .unwrap() else {
            panic!("expected the latest coalesced snapshot");
        };
        assert!(snapshot.release_epoch >= 2);
    }

    #[test]
    fn raw_socket_limit_rejects_before_spawning_an_unbounded_reader() {
        let advances = Arc::new(AtomicU32::new(0));
        let broker = Arc::new(
            SessionBrokerHandle::spawn(NetworkAuthority::new(advances), WorkerConfig::default())
                .unwrap(),
        );
        let mut config = BrowserServiceConfig::loopback(
            0,
            ["http://127.0.0.1:4173".to_owned()],
            PresentationRole::GuidedOperator,
        )
        .unwrap();
        config.max_connections = 1;
        let service = LoopbackWebService::start(
            config,
            BrowserLaunchToken::from_bytes([5; 32]),
            77,
            broker,
            Arc::new(NoStaticAssets),
        )
        .unwrap();
        let mut slow = TcpStream::connect(service.local_addr()).unwrap();
        slow.write_all(b"GET / HTTP/1.1\r\nHost: local\r\n")
            .unwrap();
        let deadline = Instant::now() + Duration::from_millis(500);
        while service.active_transport_connections() != 1 && Instant::now() < deadline {
            thread::yield_now();
        }
        assert_eq!(service.active_transport_connections(), 1);

        let mut rejected = TcpStream::connect(service.local_addr()).unwrap();
        rejected
            .write_all(b"GET / HTTP/1.1\r\nHost: local\r\n\r\n")
            .unwrap();
        rejected
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut response = [0_u8; 256];
        let count = rejected.read(&mut response).unwrap();
        let response_text = String::from_utf8_lossy(&response[..count]);
        assert!(
            response_text.starts_with("HTTP/1.1 503"),
            "unexpected response {response_text:?}; server={:?}",
            service.last_connection_error()
        );
        drop(slow);
    }

    #[test]
    fn successful_upgrade_closes_on_text_application_message() {
        let (service, _, _) = service();
        let mut socket = connect(
            &service,
            "http://127.0.0.1:4173",
            service.launch_subprotocol(),
        )
        .unwrap();
        send_handshake(&mut socket, 88, PresentationCursors::default());
        socket.send(Message::Text("not binary".into())).unwrap();
        let result = socket.read();
        assert!(matches!(
            result,
            Ok(Message::Close(_))
                | Err(tungstenite::Error::ConnectionClosed)
                | Err(tungstenite::Error::Protocol(_))
                | Err(tungstenite::Error::Io(_))
        ));
    }
}
