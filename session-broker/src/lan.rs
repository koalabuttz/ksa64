use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
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
    KPS1_HEADER_LENGTH,
};

use crate::{
    AuthenticatedNoiseChannel, BrokerError, BrokerPublication, ComparisonCode, IkResponder,
    NoiseTransportError, PairedLanConfig, PairingWindow, PeerRegistry, SessionBrokerHandle,
    StaticNoiseKeypair, UnconfirmedNoiseChannel, XxResponder,
};

pub const PAIRED_LAN_MAGIC: [u8; 4] = *b"KSL1";
pub const PAIRED_LAN_VERSION: u16 = 1;
pub const PAIRED_LAN_MODE_XX: u8 = 1;
pub const PAIRED_LAN_MODE_IK: u8 = 2;
pub const PAIRED_LAN_SELECTOR_LENGTH: usize = 8;
pub const LAN_CLIENT_ID_PREFIX: u64 = 0x4c41_4e00_0000_0000;
pub const LAN_CLIENT_ID_MASK: u64 = 0xffff_ff00_0000_0000;
pub const LAN_SOCKET_POLL_MILLIS: u64 = 100;
pub const LAN_HANDSHAKE_TIMEOUT_MILLIS: u64 = 5_000;
pub const LAN_MAX_CONTROL_ADVANCE_RELEASES: u32 = 4_096;
pub const LAN_ERROR_RESYNC_REQUIRED: u16 = 1_001;
pub const LAN_ERROR_CURSOR_AHEAD: u16 = 1_002;
pub const LAN_SUPPORTED_PRESENTATION_CAPABILITIES: u64 = KPS1_CAPABILITY_GLOBAL_DISPLAY_V1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairedLanError {
    Config,
    Bind,
    Thread,
    Io,
    ConnectionClosed,
    ConnectionLimit,
    Selector,
    HandshakeLength,
    Pairing(NoiseTransportError),
    PairingUnavailable,
    PairingRejected,
    PairingExpired,
    Protocol,
    ProtocolFrame,
    ProtocolPayload,
    ProtocolSequence,
    ProtocolEncode(PresentationMessageKind),
    ProtocolControl,
    ProtocolSequenceOverflow,
    Evidence,
    Broker(BrokerError),
    Shutdown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingLanPairing {
    pub pairing_id: u64,
    pub remote_public_key: [u8; 32],
    pub comparison_code: ComparisonCode,
    pub assigned_role: PresentationRole,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PairingDecision {
    Confirmed(ComparisonCode),
    Rejected,
}

#[derive(Debug)]
struct PairingState {
    pending: Option<PendingLanPairing>,
    decision: Option<PairingDecision>,
}

#[derive(Debug)]
struct PairingCoordinator {
    state: Mutex<PairingState>,
    changed: Condvar,
    next_id: AtomicU64,
}

impl PairingCoordinator {
    fn new() -> Self {
        Self {
            state: Mutex::new(PairingState {
                pending: None,
                decision: None,
            }),
            changed: Condvar::new(),
            next_id: AtomicU64::new(1),
        }
    }

    fn publish(
        &self,
        unconfirmed: &UnconfirmedNoiseChannel,
        role: PresentationRole,
    ) -> Result<PendingLanPairing, PairedLanError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| PairedLanError::PairingUnavailable)?;
        if state.pending.is_some() {
            return Err(PairedLanError::PairingUnavailable);
        }
        let pairing_id = self.next_id.fetch_add(1, Ordering::AcqRel);
        if pairing_id == 0 {
            return Err(PairedLanError::PairingUnavailable);
        }
        let pending = PendingLanPairing {
            pairing_id,
            remote_public_key: unconfirmed.remote_static(),
            comparison_code: unconfirmed.comparison_code(),
            assigned_role: role,
        };
        state.pending = Some(pending);
        state.decision = None;
        self.changed.notify_all();
        Ok(pending)
    }

    fn pending(&self) -> Option<PendingLanPairing> {
        self.state.lock().ok().and_then(|state| state.pending)
    }

    fn confirm(&self, pairing_id: u64, code: ComparisonCode) -> Result<(), PairedLanError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| PairedLanError::PairingUnavailable)?;
        let pending = state.pending.ok_or(PairedLanError::PairingUnavailable)?;
        if pending.pairing_id != pairing_id {
            return Err(PairedLanError::PairingUnavailable);
        }
        if pending.comparison_code != code {
            return Err(PairedLanError::Pairing(
                NoiseTransportError::ComparisonMismatch,
            ));
        }
        if state.decision.is_some() {
            return Err(PairedLanError::PairingUnavailable);
        }
        state.decision = Some(PairingDecision::Confirmed(code));
        self.changed.notify_all();
        Ok(())
    }

    fn reject(&self, pairing_id: u64) -> Result<(), PairedLanError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| PairedLanError::PairingUnavailable)?;
        let pending = state.pending.ok_or(PairedLanError::PairingUnavailable)?;
        if pending.pairing_id != pairing_id || state.decision.is_some() {
            return Err(PairedLanError::PairingUnavailable);
        }
        state.decision = Some(PairingDecision::Rejected);
        self.changed.notify_all();
        Ok(())
    }

    fn wait(
        &self,
        pairing_id: u64,
        timeout: Duration,
        shutdown: &AtomicBool,
    ) -> Result<PairingDecision, PairedLanError> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(PairedLanError::PairingExpired)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| PairedLanError::PairingUnavailable)?;
        loop {
            if shutdown.load(Ordering::Acquire) {
                state.pending = None;
                state.decision = None;
                return Err(PairedLanError::Shutdown);
            }
            if state.pending.map(|pending| pending.pairing_id) != Some(pairing_id) {
                return Err(PairedLanError::PairingUnavailable);
            }
            if let Some(decision) = state.decision.take() {
                state.pending = None;
                return Ok(decision);
            }
            let now = Instant::now();
            if now >= deadline {
                state.pending = None;
                state.decision = None;
                return Err(PairedLanError::PairingExpired);
            }
            let remaining = deadline.saturating_duration_since(now);
            let waited = self
                .changed
                .wait_timeout(state, remaining)
                .map_err(|_| PairedLanError::PairingUnavailable)?;
            state = waited.0;
            if waited.1.timed_out() {
                state.pending = None;
                state.decision = None;
                return Err(PairedLanError::PairingExpired);
            }
        }
    }

    fn wake_shutdown(&self) {
        self.changed.notify_all();
    }
}

#[derive(Debug)]
struct PeerClientMap {
    entries: Vec<([u8; 32], u64)>,
    next: u32,
}

impl PeerClientMap {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            next: 1,
        }
    }

    fn broker_client_id(&mut self, public_key: [u8; 32]) -> Result<u64, PairedLanError> {
        if let Some((_, id)) = self.entries.iter().find(|(key, _)| *key == public_key) {
            return Ok(*id);
        }
        let id = LAN_CLIENT_ID_PREFIX | u64::from(self.next);
        self.next = self
            .next
            .checked_add(1)
            .ok_or(PairedLanError::ConnectionLimit)?;
        self.entries.push((public_key, id));
        Ok(id)
    }
}

pub struct PairedLanService {
    local_addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    active_connections: Arc<AtomicUsize>,
    pairing: Arc<PairingCoordinator>,
    registry: Arc<Mutex<PeerRegistry>>,
    last_connection_error: Arc<Mutex<Option<PairedLanError>>>,
    listener: Option<JoinHandle<()>>,
}

impl PairedLanService {
    pub fn start(
        config: PairedLanConfig,
        server_keys: Arc<StaticNoiseKeypair>,
        registry: Arc<Mutex<PeerRegistry>>,
        session_nonce: u64,
        broker: Arc<SessionBrokerHandle>,
    ) -> Result<Self, PairedLanError> {
        config.validate().map_err(|_| PairedLanError::Config)?;
        Self::start_inner(config, server_keys, registry, session_nonce, broker, false)
    }

    fn start_inner(
        config: PairedLanConfig,
        server_keys: Arc<StaticNoiseKeypair>,
        registry: Arc<Mutex<PeerRegistry>>,
        session_nonce: u64,
        broker: Arc<SessionBrokerHandle>,
        allow_loopback_for_test: bool,
    ) -> Result<Self, PairedLanError> {
        if session_nonce == 0 || config.max_connections != 1 {
            return Err(PairedLanError::Config);
        }
        if allow_loopback_for_test {
            if !config.bind.ip().is_loopback() {
                return Err(PairedLanError::Config);
            }
        } else {
            config.validate().map_err(|_| PairedLanError::Config)?;
        }
        let listener = TcpListener::bind(config.bind).map_err(|_| PairedLanError::Bind)?;
        let local_addr = listener.local_addr().map_err(|_| PairedLanError::Bind)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| PairedLanError::Io)?;
        let shutdown = Arc::new(AtomicBool::new(false));
        let active_connections = Arc::new(AtomicUsize::new(0));
        let pairing = Arc::new(PairingCoordinator::new());
        let pairing_window = Arc::new(Mutex::new(
            PairingWindow::new(
                0,
                config.pairing_timeout_millis,
                config.max_pairing_attempts,
            )
            .map_err(PairedLanError::Pairing)?,
        ));
        let peer_clients = Arc::new(Mutex::new(PeerClientMap::new()));
        let last_connection_error = Arc::new(Mutex::new(None));
        let context = LanListenerContext {
            shutdown: shutdown.clone(),
            active_connections: active_connections.clone(),
            pairing: pairing.clone(),
            pairing_window,
            registry: registry.clone(),
            peer_clients,
            server_keys,
            session_nonce,
            broker,
            config,
            started: Instant::now(),
            last_connection_error: last_connection_error.clone(),
        };
        let listener_thread = thread::Builder::new()
            .name("ksa64-paired-lan".to_owned())
            .spawn(move || run_lan_listener(listener, context))
            .map_err(|_| PairedLanError::Thread)?;
        Ok(Self {
            local_addr,
            shutdown,
            active_connections,
            pairing,
            registry,
            last_connection_error,
            listener: Some(listener_thread),
        })
    }

    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn active_connections(&self) -> usize {
        self.active_connections.load(Ordering::Acquire)
    }

    pub fn pending_pairing(&self) -> Option<PendingLanPairing> {
        self.pairing.pending()
    }

    pub fn confirm_pairing(
        &self,
        pairing_id: u64,
        comparison_code: ComparisonCode,
    ) -> Result<(), PairedLanError> {
        self.pairing.confirm(pairing_id, comparison_code)
    }

    pub fn reject_pairing(&self, pairing_id: u64) -> Result<(), PairedLanError> {
        self.pairing.reject(pairing_id)
    }

    pub fn revoke_peer(&self, public_key: [u8; 32]) -> Result<(), PairedLanError> {
        self.registry
            .lock()
            .map_err(|_| PairedLanError::PairingUnavailable)?
            .revoke(&public_key)
            .map_err(PairedLanError::Pairing)
    }

    /// Exports the authenticated peer registry for the explicit host persistence
    /// boundary. These bytes are transport configuration, never mission evidence.
    pub fn export_peer_registry(&self) -> Result<Vec<u8>, PairedLanError> {
        self.registry
            .lock()
            .map_err(|_| PairedLanError::PairingUnavailable)?
            .export_bounded()
            .map_err(PairedLanError::Pairing)
    }

    pub fn last_connection_error(&self) -> Option<PairedLanError> {
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
        self.pairing.wake_shutdown();
        if let Some(listener) = self.listener.take() {
            let _ = listener.join();
        }
    }
}

impl Drop for PairedLanService {
    fn drop(&mut self) {
        self.stop();
    }
}

struct LanListenerContext {
    shutdown: Arc<AtomicBool>,
    active_connections: Arc<AtomicUsize>,
    pairing: Arc<PairingCoordinator>,
    pairing_window: Arc<Mutex<PairingWindow>>,
    registry: Arc<Mutex<PeerRegistry>>,
    peer_clients: Arc<Mutex<PeerClientMap>>,
    server_keys: Arc<StaticNoiseKeypair>,
    session_nonce: u64,
    broker: Arc<SessionBrokerHandle>,
    config: PairedLanConfig,
    started: Instant,
    last_connection_error: Arc<Mutex<Option<PairedLanError>>>,
}

fn run_lan_listener(listener: TcpListener, context: LanListenerContext) {
    while !context.shutdown.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, _)) => {
                if context
                    .active_connections
                    .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
                    .is_err()
                {
                    continue;
                }
                let result = handle_lan_connection(stream, &context);
                context.active_connections.store(0, Ordering::Release);
                if let Err(error) = result {
                    if let Ok(mut last) = context.last_connection_error.lock() {
                        *last = Some(error);
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(_) => break,
        }
    }
}

fn handle_lan_connection(
    mut stream: TcpStream,
    context: &LanListenerContext,
) -> Result<(), PairedLanError> {
    stream
        .set_nonblocking(false)
        .map_err(|_| PairedLanError::Io)?;
    stream
        .set_read_timeout(Some(Duration::from_millis(LAN_HANDSHAKE_TIMEOUT_MILLIS)))
        .map_err(|_| PairedLanError::Io)?;
    stream
        .set_write_timeout(Some(Duration::from_millis(LAN_HANDSHAKE_TIMEOUT_MILLIS)))
        .map_err(|_| PairedLanError::Io)?;
    let mode = read_selector(&mut stream)?;
    let mut channel = match mode {
        PAIRED_LAN_MODE_XX => authenticate_xx(&mut stream, context)?,
        PAIRED_LAN_MODE_IK => authenticate_ik(&mut stream, context)?,
        _ => return Err(PairedLanError::Selector),
    };
    channel
        .bind_kps1_session(context.session_nonce, 1, 1)
        .map_err(PairedLanError::Pairing)?;
    stream
        .set_read_timeout(Some(Duration::from_millis(LAN_SOCKET_POLL_MILLIS)))
        .map_err(|_| PairedLanError::Io)?;
    run_encrypted_session(&mut stream, &mut channel, context)
}

fn read_selector(stream: &mut TcpStream) -> Result<u8, PairedLanError> {
    let mut selector = [0_u8; PAIRED_LAN_SELECTOR_LENGTH];
    stream
        .read_exact(&mut selector)
        .map_err(|_| PairedLanError::Io)?;
    if selector[..4] != PAIRED_LAN_MAGIC
        || u16::from_be_bytes([selector[4], selector[5]]) != PAIRED_LAN_VERSION
        || selector[7] != 0
        || !matches!(selector[6], PAIRED_LAN_MODE_XX | PAIRED_LAN_MODE_IK)
    {
        return Err(PairedLanError::Selector);
    }
    Ok(selector[6])
}

pub fn paired_lan_selector(mode: u8) -> Result<[u8; PAIRED_LAN_SELECTOR_LENGTH], PairedLanError> {
    if !matches!(mode, PAIRED_LAN_MODE_XX | PAIRED_LAN_MODE_IK) {
        return Err(PairedLanError::Selector);
    }
    let mut selector = [0_u8; PAIRED_LAN_SELECTOR_LENGTH];
    selector[..4].copy_from_slice(&PAIRED_LAN_MAGIC);
    selector[4..6].copy_from_slice(&PAIRED_LAN_VERSION.to_be_bytes());
    selector[6] = mode;
    Ok(selector)
}

fn authenticate_xx(
    stream: &mut TcpStream,
    context: &LanListenerContext,
) -> Result<AuthenticatedNoiseChannel, PairedLanError> {
    let now_millis = u64::try_from(context.started.elapsed().as_millis()).unwrap_or(u64::MAX);
    context
        .pairing_window
        .lock()
        .map_err(|_| PairedLanError::PairingUnavailable)?
        .register_attempt(now_millis)
        .map_err(PairedLanError::Pairing)?;
    let mut responder = XxResponder::new(&context.server_keys).map_err(PairedLanError::Pairing)?;
    let first = read_handshake_packet(stream)?;
    responder
        .read_first(&first)
        .map_err(PairedLanError::Pairing)?;
    let second = responder.write_second().map_err(PairedLanError::Pairing)?;
    write_handshake_packet(stream, &second)?;
    let third = read_handshake_packet(stream)?;
    let unconfirmed = responder
        .read_third_and_finish(&third)
        .map_err(PairedLanError::Pairing)?;
    let pending = context
        .pairing
        .publish(&unconfirmed, context.config.assigned_role)?;
    let decision = context.pairing.wait(
        pending.pairing_id,
        Duration::from_millis(context.config.pairing_timeout_millis),
        &context.shutdown,
    )?;
    let PairingDecision::Confirmed(code) = decision else {
        return Err(PairedLanError::PairingRejected);
    };
    context
        .registry
        .lock()
        .map_err(|_| PairedLanError::PairingUnavailable)?
        .confirm_pairing(unconfirmed, code, context.config.assigned_role)
        .map_err(PairedLanError::Pairing)
}

fn authenticate_ik(
    stream: &mut TcpStream,
    context: &LanListenerContext,
) -> Result<AuthenticatedNoiseChannel, PairedLanError> {
    let mut responder = IkResponder::new(&context.server_keys).map_err(PairedLanError::Pairing)?;
    let first = read_handshake_packet(stream)?;
    let peer = {
        let registry = context
            .registry
            .lock()
            .map_err(|_| PairedLanError::PairingUnavailable)?;
        responder
            .read_first(&registry, &first)
            .map_err(PairedLanError::Pairing)?
    };
    if peer.role != context.config.assigned_role {
        return Err(PairedLanError::Pairing(NoiseTransportError::RoleMismatch));
    }
    let (second, channel) = responder
        .write_second_and_finish()
        .map_err(PairedLanError::Pairing)?;
    write_handshake_packet(stream, &second)?;
    Ok(channel)
}

fn read_handshake_packet(stream: &mut TcpStream) -> Result<Vec<u8>, PairedLanError> {
    let mut length = [0_u8; 4];
    stream
        .read_exact(&mut length)
        .map_err(|_| PairedLanError::Io)?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > crate::MAX_HANDSHAKE_MESSAGE_LENGTH {
        return Err(PairedLanError::HandshakeLength);
    }
    let mut message = vec![0_u8; length];
    stream
        .read_exact(&mut message)
        .map_err(|_| PairedLanError::Io)?;
    Ok(message)
}

fn write_handshake_packet(stream: &mut TcpStream, message: &[u8]) -> Result<(), PairedLanError> {
    if message.is_empty() || message.len() > crate::MAX_HANDSHAKE_MESSAGE_LENGTH {
        return Err(PairedLanError::HandshakeLength);
    }
    stream
        .write_all(&(message.len() as u32).to_be_bytes())
        .and_then(|()| stream.write_all(message))
        .map_err(|_| PairedLanError::Io)
}

fn read_encrypted_packet(stream: &mut TcpStream) -> Result<Option<Vec<u8>>, PairedLanError> {
    let mut length = [0_u8; 4];
    let first = match stream.read(&mut length) {
        Ok(0) => return Err(PairedLanError::ConnectionClosed),
        Ok(count) => count,
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
            ) =>
        {
            return Ok(None);
        }
        Err(_) => return Err(PairedLanError::Io),
    };
    if first < length.len() {
        stream
            .read_exact(&mut length[first..])
            .map_err(|_| PairedLanError::Io)?;
    }
    let payload_length = u32::from_be_bytes(length) as usize;
    if payload_length == 0 || payload_length > crate::MAX_NOISE_CIPHERTEXT_LENGTH {
        return Err(PairedLanError::HandshakeLength);
    }
    let mut packet = Vec::with_capacity(payload_length + 4);
    packet.extend_from_slice(&length);
    packet.resize(payload_length + 4, 0);
    stream
        .read_exact(&mut packet[4..])
        .map_err(|_| PairedLanError::Io)?;
    Ok(Some(packet))
}

fn read_encrypted_frame(
    stream: &mut TcpStream,
    channel: &mut AuthenticatedNoiseChannel,
) -> Result<Option<Vec<u8>>, PairedLanError> {
    let Some(first) = read_encrypted_packet(stream)? else {
        return Ok(None);
    };
    let mut packet = first;
    loop {
        if let Some(frame) = channel
            .open_packet(&packet)
            .map_err(PairedLanError::Pairing)?
        {
            return Ok(Some(frame));
        }
        packet = read_encrypted_packet(stream)?.ok_or(PairedLanError::Io)?;
    }
}

fn run_encrypted_session(
    stream: &mut TcpStream,
    channel: &mut AuthenticatedNoiseChannel,
    context: &LanListenerContext,
) -> Result<(), PairedLanError> {
    let handshake_deadline = Instant::now()
        .checked_add(Duration::from_millis(LAN_HANDSHAKE_TIMEOUT_MILLIS))
        .ok_or(PairedLanError::Protocol)?;
    let first = loop {
        if context.shutdown.load(Ordering::Acquire) {
            return Err(PairedLanError::Shutdown);
        }
        if let Some(frame) = read_encrypted_frame(stream, channel)? {
            break frame;
        }
        if Instant::now() >= handshake_deadline {
            return Err(PairedLanError::Protocol);
        }
    };
    let decoded = parse_kps1_frame(&first).map_err(|_| PairedLanError::ProtocolFrame)?;
    if decoded.header.kind != PresentationMessageKind::HandshakeRequest
        || decoded.header.session_nonce != context.session_nonce
        || decoded.header.sequence != 1
    {
        return Err(PairedLanError::Protocol);
    }
    let role = channel.peer().role;
    if role != context.config.assigned_role {
        return Err(PairedLanError::Pairing(NoiseTransportError::RoleMismatch));
    }
    let PresentationPayload::HandshakeRequest(handshake) =
        decode_typed_payload(decoded.header.kind, decoded.payload, role)
            .map_err(|_| PairedLanError::ProtocolPayload)?
    else {
        return Err(PairedLanError::Protocol);
    };
    if handshake.role != role {
        return Err(PairedLanError::Pairing(NoiseTransportError::RoleMismatch));
    }
    let client_id = context
        .peer_clients
        .lock()
        .map_err(|_| PairedLanError::PairingUnavailable)?
        .broker_client_id(channel.peer().public_key)?;
    let attached = context
        .broker
        .attach(client_id, context.session_nonce, role, handshake.cursors)
        .map_err(PairedLanError::Broker)?;
    let _disconnect = LanBrokerDisconnectGuard {
        broker: context.broker.as_ref(),
        client_id,
    };
    let capability_mask = handshake.capability_mask & LAN_SUPPORTED_PRESENTATION_CAPABILITIES;
    let mut outbound_sequence = 1_u64;
    write_secure_payload(
        stream,
        channel,
        &PresentationPayload::HandshakeResponse(PresentationHandshake {
            role,
            client_instance: client_id,
            capability_mask,
            cursors: attached.cursors,
        }),
        role,
        context.session_nonce,
        &mut outbound_sequence,
        decoded.header.correlation_id,
        KPS1_FLAG_RESPONSE,
    )?;
    let mut inbound_sequence = Kps1SequenceCursor::new(context.session_nonce, 2)
        .map_err(|_| PairedLanError::ProtocolSequence)?;
    let mut cursors = handshake.cursors;
    let mut evidence_sent = false;
    poll_and_send_secure_publication(
        stream,
        channel,
        client_id,
        role,
        context,
        &mut outbound_sequence,
        &mut cursors,
        &mut evidence_sent,
        0,
    )?;
    while !context.shutdown.load(Ordering::Acquire) {
        {
            let registry = context
                .registry
                .lock()
                .map_err(|_| PairedLanError::PairingUnavailable)?;
            registry
                .lookup(&channel.peer().public_key)
                .map_err(PairedLanError::Pairing)?;
        }
        let Some(frame) = read_encrypted_frame(stream, channel)? else {
            continue;
        };
        handle_encrypted_client_frame(
            stream,
            channel,
            &frame,
            client_id,
            role,
            context,
            &mut inbound_sequence,
            &mut outbound_sequence,
            &mut cursors,
            &mut evidence_sent,
            capability_mask,
        )?;
    }
    Ok(())
}

struct LanBrokerDisconnectGuard<'a> {
    broker: &'a SessionBrokerHandle,
    client_id: u64,
}

impl Drop for LanBrokerDisconnectGuard<'_> {
    fn drop(&mut self) {
        let _ = self.broker.disconnect(self.client_id);
    }
}

fn validate_encrypted_client_header(
    header: Kps1Header,
    capability_mask: u64,
    inbound_sequence: &mut Kps1SequenceCursor,
) -> Result<(), PairedLanError> {
    inbound_sequence
        .accept(header)
        .map_err(|_| PairedLanError::ProtocolSequence)?;
    if !header.kind.is_negotiated_by(capability_mask) {
        return Err(PairedLanError::ProtocolControl);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn handle_encrypted_client_frame(
    stream: &mut TcpStream,
    channel: &mut AuthenticatedNoiseChannel,
    bytes: &[u8],
    client_id: u64,
    role: PresentationRole,
    context: &LanListenerContext,
    inbound_sequence: &mut Kps1SequenceCursor,
    outbound_sequence: &mut u64,
    client_cursors: &mut PresentationCursors,
    evidence_sent: &mut bool,
    capability_mask: u64,
) -> Result<(), PairedLanError> {
    let frame = parse_kps1_frame(bytes).map_err(|_| PairedLanError::ProtocolFrame)?;
    validate_encrypted_client_header(frame.header, capability_mask, inbound_sequence)?;
    let payload = decode_typed_payload(frame.header.kind, frame.payload, role)
        .map_err(|_| PairedLanError::ProtocolPayload)?;
    let correlation = frame.header.correlation_id;
    match payload {
        PresentationPayload::ReplayControl(cursors) => {
            *client_cursors = cursors;
        }
        PresentationPayload::LifecycleControl(control) => {
            context
                .broker
                .set_lifecycle(client_id, control.requested)
                .map_err(PairedLanError::Broker)?;
            if control.bounded_releases > LAN_MAX_CONTROL_ADVANCE_RELEASES {
                return Err(PairedLanError::ProtocolControl);
            }
            if control.bounded_releases > 0 {
                context
                    .broker
                    .advance(client_id, control.bounded_releases)
                    .map_err(PairedLanError::Broker)?;
            }
        }
        PresentationPayload::PaceControl(control) => {
            if control.bounded_releases > LAN_MAX_CONTROL_ADVANCE_RELEASES {
                return Err(PairedLanError::ProtocolControl);
            }
            if control.requested == PresentationPace::SingleStep {
                context
                    .broker
                    .step_one_release(client_id)
                    .map_err(PairedLanError::Broker)?;
            } else {
                context
                    .broker
                    .set_pace(client_id, control.requested)
                    .map_err(PairedLanError::Broker)?;
                if control.bounded_releases > 0 {
                    context
                        .broker
                        .advance(client_id, control.bounded_releases)
                        .map_err(PairedLanError::Broker)?;
                }
            }
        }
        PresentationPayload::ActionIntent(intent) => {
            let receipt = context
                .broker
                .submit_action(client_id, intent)
                .map_err(PairedLanError::Broker)?;
            write_secure_payload(
                stream,
                channel,
                &PresentationPayload::ActionReceipt(receipt),
                role,
                context.session_nonce,
                outbound_sequence,
                correlation,
                KPS1_FLAG_RESPONSE,
            )?;
        }
        PresentationPayload::GlobalDisplayRangeRequest(request) => {
            let publication = context
                .broker
                .global_display(client_id, request)
                .map_err(PairedLanError::Broker)?
                .ok_or(PairedLanError::ProtocolControl)?;
            send_secure_global_display_publication(
                stream,
                channel,
                publication,
                role,
                context.session_nonce,
                outbound_sequence,
                correlation,
            )?;
            return Ok(());
        }
        _ => return Err(PairedLanError::ProtocolControl),
    }
    poll_and_send_secure_publication(
        stream,
        channel,
        client_id,
        role,
        context,
        outbound_sequence,
        client_cursors,
        evidence_sent,
        correlation,
    )
}

#[allow(clippy::too_many_arguments)]
fn poll_and_send_secure_publication(
    stream: &mut TcpStream,
    channel: &mut AuthenticatedNoiseChannel,
    client_id: u64,
    role: PresentationRole,
    context: &LanListenerContext,
    outbound_sequence: &mut u64,
    client_cursors: &mut PresentationCursors,
    evidence_sent: &mut bool,
    correlation: u64,
) -> Result<(), PairedLanError> {
    let response_flags = if correlation == 0 {
        0
    } else {
        KPS1_FLAG_RESPONSE
    };
    let publication = match context.broker.poll(client_id, *client_cursors, 256) {
        Ok(publication) => publication,
        Err(BrokerError::Cursor(CursorError::ResyncRequired { oldest_available })) => {
            write_secure_payload(
                stream,
                channel,
                &PresentationPayload::Error(PresentationErrorView {
                    code: LAN_ERROR_RESYNC_REQUIRED,
                    fatal: false,
                    detail_identity: u32::try_from(oldest_available).unwrap_or(u32::MAX),
                    message: format!("ResyncRequired: oldest cursor {oldest_available}"),
                }),
                role,
                context.session_nonce,
                outbound_sequence,
                correlation,
                response_flags,
            )?;
            return Ok(());
        }
        Err(BrokerError::Cursor(CursorError::Ahead { next_available })) => {
            write_secure_payload(
                stream,
                channel,
                &PresentationPayload::Error(PresentationErrorView {
                    code: LAN_ERROR_CURSOR_AHEAD,
                    fatal: false,
                    detail_identity: u32::try_from(next_available).unwrap_or(u32::MAX),
                    message: format!("cursor ahead: next cursor {next_available}"),
                }),
                role,
                context.session_nonce,
                outbound_sequence,
                correlation,
                response_flags,
            )?;
            return Ok(());
        }
        Err(error) => return Err(PairedLanError::Broker(error)),
    };
    let evidence = publication.evidence;
    send_secure_publication(
        stream,
        channel,
        publication,
        role,
        context.session_nonce,
        outbound_sequence,
        client_cursors,
    )?;
    if !*evidence_sent {
        if let Some(metadata) = evidence {
            send_secure_evidence(
                stream,
                channel,
                context.broker.as_ref(),
                client_id,
                role,
                context.session_nonce,
                outbound_sequence,
                metadata,
            )?;
            *evidence_sent = true;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn send_secure_global_display_publication(
    stream: &mut TcpStream,
    channel: &mut AuthenticatedNoiseChannel,
    publication: crate::GlobalDisplayPublication,
    role: PresentationRole,
    session_nonce: u64,
    sequence: &mut u64,
    correlation: u64,
) -> Result<(), PairedLanError> {
    write_secure_payload(
        stream,
        channel,
        &PresentationPayload::GlobalDisplayDefinition(publication.definition),
        role,
        session_nonce,
        sequence,
        0,
        0,
    )?;
    if !publication.samples.is_empty() {
        write_secure_payload(
            stream,
            channel,
            &PresentationPayload::GlobalDisplaySampleBatch(publication.samples),
            role,
            session_nonce,
            sequence,
            0,
            0,
        )?;
    }
    for path in publication.paths {
        write_secure_payload(
            stream,
            channel,
            &PresentationPayload::GlobalDisplayPathChunk(path),
            role,
            session_nonce,
            sequence,
            0,
            0,
        )?;
    }
    for transition in publication.transitions {
        write_secure_payload(
            stream,
            channel,
            &PresentationPayload::GlobalDisplayTransition(transition),
            role,
            session_nonce,
            sequence,
            0,
            0,
        )?;
    }
    write_secure_payload(
        stream,
        channel,
        &PresentationPayload::GlobalReplayIndex(publication.replay_index),
        role,
        session_nonce,
        sequence,
        0,
        0,
    )?;
    write_secure_payload(
        stream,
        channel,
        &PresentationPayload::GlobalDisplayCursorState(publication.cursor),
        role,
        session_nonce,
        sequence,
        correlation,
        KPS1_FLAG_RESPONSE,
    )
}

#[allow(clippy::too_many_arguments)]
fn send_secure_publication(
    stream: &mut TcpStream,
    channel: &mut AuthenticatedNoiseChannel,
    publication: BrokerPublication,
    role: PresentationRole,
    session_nonce: u64,
    sequence: &mut u64,
    cursors: &mut PresentationCursors,
) -> Result<(), PairedLanError> {
    write_secure_payload(
        stream,
        channel,
        &PresentationPayload::Snapshot(publication.snapshot.clone()),
        role,
        session_nonce,
        sequence,
        0,
        0,
    )?;
    if let Some(procedure) = publication.procedure {
        write_secure_payload(
            stream,
            channel,
            &PresentationPayload::Procedure(procedure),
            role,
            session_nonce,
            sequence,
            0,
            0,
        )?;
    }
    if let Some(disposition) = publication.disposition {
        write_secure_payload(
            stream,
            channel,
            &PresentationPayload::Disposition(disposition),
            role,
            session_nonce,
            sequence,
            0,
            0,
        )?;
    }
    for path in publication.prediction_paths {
        write_secure_payload(
            stream,
            channel,
            &PresentationPayload::PredictionPath(path),
            role,
            session_nonce,
            sequence,
            0,
            0,
        )?;
    }
    if !publication.events.records.is_empty() {
        write_secure_payload(
            stream,
            channel,
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
        write_secure_payload(
            stream,
            channel,
            &PresentationPayload::TimelineEvent(event.value.clone()),
            role,
            session_nonce,
            sequence,
            0,
            0,
        )?;
    }
    for receipt in &publication.action_receipts.records {
        write_secure_payload(
            stream,
            channel,
            &PresentationPayload::ActionReceipt(receipt.value),
            role,
            session_nonce,
            sequence,
            0,
            0,
        )?;
    }
    if !publication.release_samples.records.is_empty() {
        write_secure_payload(
            stream,
            channel,
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
    write_secure_payload(
        stream,
        channel,
        &PresentationPayload::TransportStatus(publication.transport),
        role,
        session_nonce,
        sequence,
        0,
        0,
    )?;
    if let Some(evidence) = publication.evidence {
        write_secure_payload(
            stream,
            channel,
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
fn send_secure_evidence(
    stream: &mut TcpStream,
    channel: &mut AuthenticatedNoiseChannel,
    broker: &SessionBrokerHandle,
    client_id: u64,
    role: PresentationRole,
    session_nonce: u64,
    sequence: &mut u64,
    metadata: SealedEvidenceMetadata,
) -> Result<(), PairedLanError> {
    metadata.validate().map_err(|_| PairedLanError::Evidence)?;
    if !metadata.complete {
        return Ok(());
    }
    let bytes = broker
        .sealed_evidence(client_id)
        .map_err(PairedLanError::Broker)?
        .ok_or(PairedLanError::Evidence)?;
    if bytes.len() as u64 != metadata.total_length || crc32_ieee(&bytes) != metadata.evidence_crc32
    {
        return Err(PairedLanError::Evidence);
    }
    let chunk_length =
        usize::try_from(metadata.chunk_length).map_err(|_| PairedLanError::Evidence)?;
    if chunk_length == 0 || chunk_length > KPS1_EVIDENCE_CHUNK_DATA_MAX_LENGTH {
        return Err(PairedLanError::Evidence);
    }
    let chunks = bytes.chunks(chunk_length);
    if chunks.len() != metadata.chunk_count as usize {
        return Err(PairedLanError::Evidence);
    }
    for (index, chunk) in chunks.enumerate() {
        write_secure_payload(
            stream,
            channel,
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

#[allow(clippy::too_many_arguments)]
fn write_secure_payload(
    stream: &mut TcpStream,
    channel: &mut AuthenticatedNoiseChannel,
    payload: &PresentationPayload,
    role: PresentationRole,
    session_nonce: u64,
    sequence: &mut u64,
    correlation_id: u64,
    flags: u32,
) -> Result<(), PairedLanError> {
    let payload_bytes = encode_typed_payload(payload, role)
        .map_err(|_| PairedLanError::ProtocolEncode(payload.kind()))?;
    let header = Kps1Header {
        kind: payload.kind(),
        flags,
        session_nonce,
        sequence: *sequence,
        correlation_id,
        payload_length: payload_bytes.len() as u32,
    };
    let mut frame = vec![0_u8; KPS1_HEADER_LENGTH + payload_bytes.len()];
    write_kps1_frame(header, &payload_bytes, &mut frame)
        .map_err(|_| PairedLanError::ProtocolFrame)?;
    for packet in channel.seal_kps1(&frame).map_err(PairedLanError::Pairing)? {
        stream.write_all(&packet).map_err(|_| PairedLanError::Io)?;
    }
    *sequence = sequence
        .checked_add(1)
        .ok_or(PairedLanError::ProtocolSequenceOverflow)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AuthorityError, BrokerAuthority, IkInitiator, PeerRecord, WorkerConfig, XxInitiator,
    };
    use ksa64_presentation::{
        ActionReceiptView, DispositionView, NavigationView, OperationalSnapshot,
        OverallDisposition, PredictionPathView, PredictionSummaryView, PresentationActionIntent,
        PresentationBatch, PresentationEventView, PresentationLifecycle, PresentationQueueStatus,
        PresentationSession, PresentationStaleness, ProcedureView, ReleaseSampleView,
        RetainedStream, TimelineEventView, TransportStatusView, PRESENTATION_MODEL_ID,
    };
    use std::sync::mpsc;

    struct MockAuthority {
        role: PresentationRole,
        snapshot: OperationalSnapshot,
        events: RetainedStream<PresentationEventView>,
        timeline: RetainedStream<TimelineEventView>,
        receipts: RetainedStream<ActionReceiptView>,
        samples: RetainedStream<ReleaseSampleView>,
    }

    impl MockAuthority {
        fn new(role: PresentationRole) -> Self {
            Self {
                role,
                snapshot: OperationalSnapshot {
                    presentation_model_identity: PRESENTATION_MODEL_ID,
                    session_definition_identity: 1,
                    publication_sequence: 1,
                    validity_mask: 0,
                    role,
                    lifecycle: PresentationLifecycle::Paused,
                    pace: PresentationPace::Paused,
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
                events: RetainedStream::new(4).unwrap(),
                timeline: RetainedStream::new(4).unwrap(),
                receipts: RetainedStream::new(4).unwrap(),
                samples: RetainedStream::new(4).unwrap(),
            }
        }
    }

    impl PresentationSession for MockAuthority {
        type Error = AuthorityError;

        fn role(&self) -> PresentationRole {
            self.role
        }

        fn lifecycle(&self) -> PresentationLifecycle {
            self.snapshot.lifecycle
        }

        fn latest_snapshot(&self) -> OperationalSnapshot {
            self.snapshot.clone()
        }

        fn current_procedure(&self) -> Option<ProcedureView> {
            None
        }

        fn current_disposition(&self) -> Option<DispositionView> {
            Some(DispositionView {
                overall: OverallDisposition::DegradedSuccess,
                axes: ksa64_presentation::DispositionAxes {
                    objective: 1,
                    vehicle: 1,
                    procedure: 1,
                    operator: 1,
                    avionics: 1,
                    evidence: 1,
                },
                reason_identity: 1,
            })
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

        fn finalization_evidence(&self) -> Option<SealedEvidenceMetadata> {
            None
        }

        fn cursors(&self) -> PresentationCursors {
            PresentationCursors {
                snapshots: 1,
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
            intent: PresentationActionIntent,
        ) -> Result<ActionReceiptView, Self::Error> {
            intent
                .validate(self.role)
                .map_err(|_| AuthorityError { code: 7 })?;
            Ok(ActionReceiptView {
                publication_sequence: 1,
                proposal_identity: intent.proposal_identity,
                load_identity: intent.expected_load_identity,
                control_identity: 1,
                receipt_epoch: self.snapshot.release_epoch,
                effective_epoch: intent.requested_activation_epoch,
                state: 1,
                reason: 0,
                accepted: true,
                operation: intent.operation,
                receipt_checksum: 8,
            })
        }
    }

    impl BrokerAuthority for MockAuthority {
        fn session_nonce(&self) -> u64 {
            99
        }

        fn advance_bounded(&mut self, releases: u32) -> Result<u32, AuthorityError> {
            self.snapshot.release_epoch = self.snapshot.release_epoch.saturating_add(releases);
            Ok(releases)
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

    #[test]
    fn post_handshake_kps1_nonce_and_sequence_are_strict() {
        let header = Kps1Header {
            kind: PresentationMessageKind::ReplayControl,
            flags: 0,
            session_nonce: 99,
            sequence: 2,
            correlation_id: 7,
            payload_length: 0,
        };
        let mut cursor = Kps1SequenceCursor::new(99, 2).unwrap();
        assert_eq!(
            validate_encrypted_client_header(header, 0, &mut cursor),
            Ok(())
        );
        assert_eq!(
            validate_encrypted_client_header(header, 0, &mut cursor),
            Err(PairedLanError::ProtocolSequence)
        );

        let mut wrong_nonce = Kps1SequenceCursor::new(99, 2).unwrap();
        assert_eq!(
            validate_encrypted_client_header(
                Kps1Header {
                    session_nonce: 100,
                    ..header
                },
                0,
                &mut wrong_nonce,
            ),
            Err(PairedLanError::ProtocolSequence)
        );
        let mut reordered = Kps1SequenceCursor::new(99, 2).unwrap();
        assert_eq!(
            validate_encrypted_client_header(
                Kps1Header {
                    sequence: 3,
                    ..header
                },
                0,
                &mut reordered,
            ),
            Err(PairedLanError::ProtocolSequence)
        );
    }

    #[test]
    fn selector_is_strict() {
        let xx = paired_lan_selector(PAIRED_LAN_MODE_XX).unwrap();
        assert_eq!(&xx[..4], &PAIRED_LAN_MAGIC);
        assert_eq!(u16::from_be_bytes([xx[4], xx[5]]), PAIRED_LAN_VERSION);
        assert_eq!(xx[6], PAIRED_LAN_MODE_XX);
        assert_eq!(xx[7], 0);
        assert_eq!(paired_lan_selector(9), Err(PairedLanError::Selector));
    }

    #[test]
    fn real_socket_xx_confirmation_and_ik_reconnect_preserve_server_identity() {
        let role = PresentationRole::GuidedOperator;
        let broker = Arc::new(
            SessionBrokerHandle::spawn(
                MockAuthority::new(role),
                WorkerConfig {
                    autonomous_pacing: false,
                    ..WorkerConfig::default()
                },
            )
            .unwrap(),
        );
        let server_keys = Arc::new(StaticNoiseKeypair::generate().unwrap());
        let server_public = server_keys.public_key();
        let server_registry = Arc::new(Mutex::new(PeerRegistry::default()));
        let config = PairedLanConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            assigned_role: role,
            pairing_timeout_millis: 2_000,
            max_pairing_attempts: 3,
            max_connections: 1,
        };
        let service =
            PairedLanService::start_inner(config, server_keys, server_registry, 99, broker, true)
                .unwrap();
        let address = service.local_addr();
        let client_keys = Arc::new(StaticNoiseKeypair::generate().unwrap());
        let thread_keys = client_keys.clone();
        let (pairing_sender, pairing_receiver) = mpsc::sync_channel(1);
        let xx_client = thread::spawn(move || {
            client_xx_round_trip(
                address,
                thread_keys.as_ref(),
                server_public,
                role,
                pairing_sender,
            )
        });
        let client_code = match pairing_receiver.recv_timeout(Duration::from_secs(2)) {
            Ok(code) => code,
            Err(error) => panic!(
                "client failed before pairing publication: {error:?}; result={:?}; server={:?}",
                xx_client.join(),
                service.last_connection_error()
            ),
        };
        let deadline = Instant::now() + Duration::from_secs(2);
        let pending = loop {
            if let Some(pending) = service.pending_pairing() {
                break pending;
            }
            assert!(Instant::now() < deadline, "server did not publish pairing");
            thread::yield_now();
        };
        assert_eq!(pending.comparison_code, client_code);
        service
            .confirm_pairing(pending.pairing_id, pending.comparison_code)
            .unwrap();
        let first_result = xx_client.join().unwrap();
        assert!(
            first_result.is_ok(),
            "XX client failed: {first_result:?}; server={:?}",
            service.last_connection_error()
        );
        let (first_client_id, server_peer) = first_result.unwrap();
        assert_eq!(first_client_id & LAN_CLIENT_ID_MASK, LAN_CLIENT_ID_PREFIX);
        wait_for_disconnect(&service);

        let second_client_id =
            client_ik_round_trip(address, client_keys.as_ref(), server_peer, role).unwrap();
        assert_eq!(second_client_id, first_client_id);
        wait_for_disconnect(&service);
        service.shutdown();
    }

    fn client_xx_round_trip(
        address: SocketAddr,
        client_keys: &StaticNoiseKeypair,
        server_public: [u8; 32],
        role: PresentationRole,
        pairing_sender: mpsc::SyncSender<ComparisonCode>,
    ) -> Result<(u64, PeerRecord), PairedLanError> {
        let mut stream = connect_client(address)?;
        stream
            .write_all(&paired_lan_selector(PAIRED_LAN_MODE_XX)?)
            .map_err(|_| PairedLanError::Io)?;
        let mut initiator = XxInitiator::new(client_keys).map_err(PairedLanError::Pairing)?;
        let first = initiator.write_first().map_err(PairedLanError::Pairing)?;
        write_handshake_packet(&mut stream, &first)?;
        let second = read_handshake_packet(&mut stream)?;
        initiator
            .read_second(&second)
            .map_err(PairedLanError::Pairing)?;
        let (third, unconfirmed) = initiator
            .write_third_and_finish()
            .map_err(PairedLanError::Pairing)?;
        write_handshake_packet(&mut stream, &third)?;
        let code = unconfirmed.comparison_code();
        pairing_sender.send(code).map_err(|_| PairedLanError::Io)?;
        let mut registry = PeerRegistry::default();
        let mut channel = registry
            .confirm_pairing(unconfirmed, code, role)
            .map_err(PairedLanError::Pairing)?;
        let server_peer = registry
            .lookup(&server_public)
            .map_err(PairedLanError::Pairing)?;
        let assigned = complete_client_session(&mut stream, &mut channel, role, 0xdead_beef)?;
        Ok((assigned, server_peer))
    }

    fn client_ik_round_trip(
        address: SocketAddr,
        client_keys: &StaticNoiseKeypair,
        server_peer: PeerRecord,
        role: PresentationRole,
    ) -> Result<u64, PairedLanError> {
        let mut stream = connect_client(address)?;
        stream
            .write_all(&paired_lan_selector(PAIRED_LAN_MODE_IK)?)
            .map_err(|_| PairedLanError::Io)?;
        let mut initiator =
            IkInitiator::new(client_keys, server_peer).map_err(PairedLanError::Pairing)?;
        let first = initiator.write_first().map_err(PairedLanError::Pairing)?;
        write_handshake_packet(&mut stream, &first)?;
        let second = read_handshake_packet(&mut stream)?;
        let mut channel = initiator
            .read_second_and_finish(&second)
            .map_err(PairedLanError::Pairing)?;
        complete_client_session(&mut stream, &mut channel, role, u64::MAX)
    }

    fn connect_client(address: SocketAddr) -> Result<TcpStream, PairedLanError> {
        let stream = TcpStream::connect(address).map_err(|_| PairedLanError::Io)?;
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(|_| PairedLanError::Io)?;
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(|_| PairedLanError::Io)?;
        Ok(stream)
    }

    fn complete_client_session(
        stream: &mut TcpStream,
        channel: &mut AuthenticatedNoiseChannel,
        role: PresentationRole,
        claimed_client_id: u64,
    ) -> Result<u64, PairedLanError> {
        channel
            .bind_kps1_session(99, 1, 1)
            .map_err(PairedLanError::Pairing)?;
        let mut sequence = 1_u64;
        write_secure_payload(
            stream,
            channel,
            &PresentationPayload::HandshakeRequest(PresentationHandshake {
                role,
                client_instance: claimed_client_id,
                capability_mask: 1,
                cursors: PresentationCursors::default(),
            }),
            role,
            99,
            &mut sequence,
            9,
            0,
        )?;
        let response = read_one_secure_frame(stream, channel)?;
        let frame = parse_kps1_frame(&response).map_err(|_| PairedLanError::ProtocolFrame)?;
        assert_eq!(
            frame.header.kind,
            PresentationMessageKind::HandshakeResponse
        );
        let PresentationPayload::HandshakeResponse(handshake) =
            decode_typed_payload(frame.header.kind, frame.payload, role)
                .map_err(|_| PairedLanError::ProtocolPayload)?
        else {
            return Err(PairedLanError::Protocol);
        };
        assert_ne!(handshake.client_instance, claimed_client_id);
        assert_eq!(handshake.cursors, PresentationCursors::default());

        // A successful reconnect immediately receives the retained publication
        // requested by the handshake cursors. Drain it through the transport
        // status boundary before issuing another control frame.
        let mut saw_initial_snapshot = false;
        loop {
            let publication = read_one_secure_frame(stream, channel)?;
            let frame =
                parse_kps1_frame(&publication).map_err(|_| PairedLanError::ProtocolFrame)?;
            saw_initial_snapshot |= frame.header.kind == PresentationMessageKind::Snapshot;
            if frame.header.kind == PresentationMessageKind::TransportStatus {
                break;
            }
        }
        assert!(saw_initial_snapshot);

        write_secure_payload(
            stream,
            channel,
            &PresentationPayload::ReplayControl(PresentationCursors::default()),
            role,
            99,
            &mut sequence,
            10,
            0,
        )?;
        let mut saw_snapshot = false;
        loop {
            let publication = read_one_secure_frame(stream, channel)?;
            let frame =
                parse_kps1_frame(&publication).map_err(|_| PairedLanError::ProtocolFrame)?;
            saw_snapshot |= frame.header.kind == PresentationMessageKind::Snapshot;
            if frame.header.kind == PresentationMessageKind::TransportStatus {
                break;
            }
        }
        assert!(saw_snapshot);
        Ok(handshake.client_instance)
    }

    fn read_one_secure_frame(
        stream: &mut TcpStream,
        channel: &mut AuthenticatedNoiseChannel,
    ) -> Result<Vec<u8>, PairedLanError> {
        loop {
            if let Some(frame) = read_encrypted_frame(stream, channel)? {
                return Ok(frame);
            }
        }
    }

    fn wait_for_disconnect(service: &PairedLanService) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while service.active_connections() != 0 {
            assert!(
                Instant::now() < deadline,
                "server did not release connection"
            );
            thread::yield_now();
        }
    }
}
