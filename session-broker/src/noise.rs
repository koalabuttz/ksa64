use alloc::boxed::Box;
#[cfg(test)]
use alloc::format;
use alloc::{vec, vec::Vec};
use core::fmt;

use blake2::{Blake2s256, Digest};
use ksa64_presentation::{
    parse_kps1_frame, Kps1Error, Kps1SequenceCursor, PresentationRole, KPS1_HEADER_LENGTH,
    KPS1_MAX_PAYLOAD_LENGTH,
};
use snow::{
    params::{CipherChoice, DHChoice, HashChoice, NoiseParams},
    resolvers::{CryptoResolver, DefaultResolver},
    types::{Cipher, Dh, Hash, Random},
    Builder, Error as SnowError, HandshakeState, TransportState,
};

use crate::MAX_PAIRED_PEERS;

pub const NOISE_XX_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
pub const NOISE_IK_PATTERN: &str = "Noise_IK_25519_ChaChaPoly_BLAKE2s";
pub const NOISE_PROLOGUE: &[u8] = b"KSA64 paired LAN KPS1 v1";
pub const MAX_HANDSHAKE_MESSAGE_LENGTH: usize = 1_024;
pub const PEER_REGISTRY_HEADER_LENGTH: usize = 16;
pub const PEER_REGISTRY_RECORD_LENGTH: usize = 40;
pub const PEER_REGISTRY_TRAILER_LENGTH: usize = 4;
pub const PEER_REGISTRY_MAX_LENGTH: usize = PEER_REGISTRY_HEADER_LENGTH
    + MAX_PAIRED_PEERS * PEER_REGISTRY_RECORD_LENGTH
    + PEER_REGISTRY_TRAILER_LENGTH;
const PEER_REGISTRY_MAGIC: [u8; 4] = *b"PPR1";
const PEER_REGISTRY_VERSION: u16 = 1;
const PEER_FLAG_REVOKED: u8 = 1;
pub const MAX_NOISE_CIPHERTEXT_LENGTH: usize = 65_535;
pub const SECURE_FRAGMENT_HEADER_LENGTH: usize = 24;
pub const SECURE_FRAGMENT_PLAINTEXT_LENGTH: usize = 60 * 1_024;
pub const SECURE_FRAGMENT_DATA_LENGTH: usize =
    SECURE_FRAGMENT_PLAINTEXT_LENGTH - SECURE_FRAGMENT_HEADER_LENGTH;
pub const MAX_ENCRYPTED_KPS1_LENGTH: usize = KPS1_HEADER_LENGTH + KPS1_MAX_PAYLOAD_LENGTH;
const SECURE_FRAGMENT_MAGIC: [u8; 4] = *b"KSN1";
const SECURE_FRAGMENT_FINAL: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoiseTransportError {
    Entropy,
    Parameters,
    Handshake,
    HandshakeOrder,
    HandshakeLength,
    RemoteStaticMissing,
    RemoteStaticLength,
    ComparisonMismatch,
    PairingExpired,
    PairingRateLimited,
    PeerLimit,
    UnknownPeer,
    RevokedPeer,
    RoleMismatch,
    RegistryLength,
    RegistryMagic,
    RegistryVersion,
    RegistryReserved,
    RegistryCrc,
    RegistryDuplicate,
    RegistryKey,
    FrameLength,
    FragmentHeader,
    FragmentOrder,
    FragmentFlags,
    MessageOverflow,
    Noise,
    ChannelPoisoned,
    Kps1(Kps1Error),
}

#[derive(PartialEq, Eq)]
pub struct HandshakeEntropy([u8; 32]);

impl HandshakeEntropy {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[cfg(feature = "std")]
    pub fn generate() -> Result<Self, NoiseTransportError> {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| NoiseTransportError::Entropy)?;
        Ok(Self(bytes))
    }

    const fn bytes(&self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for HandshakeEntropy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HandshakeEntropy(<redacted>)")
    }
}

impl Drop for HandshakeEntropy {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

struct SeedRandom {
    seed: [u8; 32],
    counter: u64,
}

impl Random for SeedRandom {
    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), SnowError> {
        let mut offset = 0_usize;
        while offset < destination.len() {
            let mut hash = Blake2s256::new();
            hash.update(b"KSA64 Noise ephemeral v1");
            hash.update(self.seed);
            hash.update(self.counter.to_le_bytes());
            let block = hash.finalize();
            let count = core::cmp::min(block.len(), destination.len() - offset);
            destination[offset..offset + count].copy_from_slice(&block[..count]);
            self.counter = self.counter.checked_add(1).ok_or(SnowError::Rng)?;
            offset += count;
        }
        Ok(())
    }
}

impl Drop for SeedRandom {
    fn drop(&mut self) {
        self.seed.fill(0);
        self.counter = 0;
    }
}

struct SeedResolver {
    seed: [u8; 32],
    primitives: DefaultResolver,
}

impl SeedResolver {
    const fn new(entropy: &HandshakeEntropy) -> Self {
        Self {
            seed: entropy.bytes(),
            primitives: DefaultResolver,
        }
    }
}

impl Drop for SeedResolver {
    fn drop(&mut self) {
        self.seed.fill(0);
    }
}

impl CryptoResolver for SeedResolver {
    fn resolve_rng(&self) -> Option<Box<dyn Random>> {
        Some(Box::new(SeedRandom {
            seed: self.seed,
            counter: 0,
        }))
    }

    fn resolve_dh(&self, choice: &DHChoice) -> Option<Box<dyn Dh>> {
        self.primitives.resolve_dh(choice)
    }

    fn resolve_hash(&self, choice: &HashChoice) -> Option<Box<dyn Hash>> {
        self.primitives.resolve_hash(choice)
    }

    fn resolve_cipher(&self, choice: &CipherChoice) -> Option<Box<dyn Cipher>> {
        self.primitives.resolve_cipher(choice)
    }
}

fn builder_with_entropy(
    pattern: &str,
    entropy: HandshakeEntropy,
) -> Result<Builder<'static>, NoiseTransportError> {
    Ok(Builder::with_resolver(
        parse_params(pattern)?,
        Box::new(SeedResolver::new(&entropy)),
    ))
}

pub struct StaticNoiseKeypair {
    private: [u8; 32],
    public: [u8; 32],
}

impl StaticNoiseKeypair {
    #[cfg(feature = "std")]
    pub fn generate() -> Result<Self, NoiseTransportError> {
        let params = parse_params(NOISE_XX_PATTERN)?;
        let keypair = Builder::new(params)
            .generate_keypair()
            .map_err(|_| NoiseTransportError::Entropy)?;
        if keypair.private.len() != 32 || keypair.public.len() != 32 {
            return Err(NoiseTransportError::RemoteStaticLength);
        }
        let mut private = [0_u8; 32];
        let mut public = [0_u8; 32];
        private.copy_from_slice(&keypair.private);
        public.copy_from_slice(&keypair.public);
        Ok(Self { private, public })
    }

    pub const fn from_parts(private: [u8; 32], public: [u8; 32]) -> Self {
        Self { private, public }
    }

    /// Generates a static keypair from caller-provided platform entropy.
    ///
    /// This keeps constrained targets from silently falling back to
    /// deterministic or non-cryptographic randomness.
    pub fn generate_with_entropy(entropy: HandshakeEntropy) -> Result<Self, NoiseTransportError> {
        let keypair = builder_with_entropy(NOISE_XX_PATTERN, entropy)?
            .generate_keypair()
            .map_err(|_| NoiseTransportError::Entropy)?;
        if keypair.private.len() != 32 || keypair.public.len() != 32 {
            return Err(NoiseTransportError::RemoteStaticLength);
        }
        let mut private = [0_u8; 32];
        let mut public = [0_u8; 32];
        private.copy_from_slice(&keypair.private);
        public.copy_from_slice(&keypair.public);
        Ok(Self { private, public })
    }

    /// Returns private material only for a platform-owned secure persistence
    /// boundary. It must never enter evidence or presentation records.
    pub const fn private_key_for_secure_store(&self) -> [u8; 32] {
        self.private
    }

    pub const fn public_key(&self) -> [u8; 32] {
        self.public
    }
}

impl fmt::Debug for StaticNoiseKeypair {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StaticNoiseKeypair")
            .field("private", &"[REDACTED]")
            .field("public", &self.public)
            .finish()
    }
}

impl Drop for StaticNoiseKeypair {
    fn drop(&mut self) {
        self.private.fill(0);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComparisonCode(u32);

impl ComparisonCode {
    pub const fn from_value(value: u32) -> Option<Self> {
        if value <= 999_999 {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

impl fmt::Display for ComparisonCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:06}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeerRecord {
    pub public_key: [u8; 32],
    pub role: PresentationRole,
    pub revoked: bool,
}

#[derive(Clone, Debug, Default)]
pub struct PeerRegistry {
    peers: Vec<PeerRecord>,
}

impl PeerRegistry {
    pub fn records(&self) -> &[PeerRecord] {
        &self.peers
    }

    pub fn export_bounded(&self) -> Result<Vec<u8>, NoiseTransportError> {
        if self.peers.len() > MAX_PAIRED_PEERS {
            return Err(NoiseTransportError::PeerLimit);
        }
        let length = PEER_REGISTRY_HEADER_LENGTH
            + self.peers.len() * PEER_REGISTRY_RECORD_LENGTH
            + PEER_REGISTRY_TRAILER_LENGTH;
        let mut output = vec![0_u8; length];
        output[..4].copy_from_slice(&PEER_REGISTRY_MAGIC);
        put_u16(&mut output, 4, PEER_REGISTRY_VERSION);
        put_u16(&mut output, 6, PEER_REGISTRY_HEADER_LENGTH as u16);
        put_u16(&mut output, 8, self.peers.len() as u16);
        put_u16(&mut output, 10, PEER_REGISTRY_RECORD_LENGTH as u16);
        for (index, peer) in self.peers.iter().enumerate() {
            if self.peers[..index]
                .iter()
                .any(|prior| prior.public_key == peer.public_key)
            {
                return Err(NoiseTransportError::RegistryDuplicate);
            }
            if peer.public_key.iter().all(|byte| *byte == 0)
                || matches!(peer.role, PresentationRole::ScriptedOperator)
            {
                return Err(NoiseTransportError::RegistryKey);
            }
            let offset = PEER_REGISTRY_HEADER_LENGTH + index * PEER_REGISTRY_RECORD_LENGTH;
            output[offset..offset + 32].copy_from_slice(&peer.public_key);
            output[offset + 32] = peer.role as u8;
            output[offset + 33] = if peer.revoked { PEER_FLAG_REVOKED } else { 0 };
            let record_crc = registry_crc32(&output[offset..offset + 36]);
            put_u32(&mut output, offset + 36, record_crc);
        }
        let trailer = length - PEER_REGISTRY_TRAILER_LENGTH;
        let archive_crc = registry_crc32(&output[..trailer]);
        put_u32(&mut output, trailer, archive_crc);
        Ok(output)
    }

    pub fn import_bounded(input: &[u8]) -> Result<Self, NoiseTransportError> {
        if input.len() < PEER_REGISTRY_HEADER_LENGTH + PEER_REGISTRY_TRAILER_LENGTH
            || input.len() > PEER_REGISTRY_MAX_LENGTH
        {
            return Err(NoiseTransportError::RegistryLength);
        }
        if input[..4] != PEER_REGISTRY_MAGIC {
            return Err(NoiseTransportError::RegistryMagic);
        }
        if get_u16(input, 4) != PEER_REGISTRY_VERSION
            || usize::from(get_u16(input, 6)) != PEER_REGISTRY_HEADER_LENGTH
            || usize::from(get_u16(input, 10)) != PEER_REGISTRY_RECORD_LENGTH
        {
            return Err(NoiseTransportError::RegistryVersion);
        }
        if input[12..16].iter().any(|byte| *byte != 0) {
            return Err(NoiseTransportError::RegistryReserved);
        }
        let count = usize::from(get_u16(input, 8));
        if count > MAX_PAIRED_PEERS {
            return Err(NoiseTransportError::PeerLimit);
        }
        let expected = PEER_REGISTRY_HEADER_LENGTH
            + count * PEER_REGISTRY_RECORD_LENGTH
            + PEER_REGISTRY_TRAILER_LENGTH;
        if input.len() != expected {
            return Err(NoiseTransportError::RegistryLength);
        }
        let trailer = expected - PEER_REGISTRY_TRAILER_LENGTH;
        if get_u32(input, trailer) != registry_crc32(&input[..trailer]) {
            return Err(NoiseTransportError::RegistryCrc);
        }
        let mut peers = Vec::with_capacity(count);
        for index in 0..count {
            let offset = PEER_REGISTRY_HEADER_LENGTH + index * PEER_REGISTRY_RECORD_LENGTH;
            if get_u32(input, offset + 36) != registry_crc32(&input[offset..offset + 36]) {
                return Err(NoiseTransportError::RegistryCrc);
            }
            if input[offset + 34] != 0 || input[offset + 35] != 0 {
                return Err(NoiseTransportError::RegistryReserved);
            }
            let flags = input[offset + 33];
            if flags & !PEER_FLAG_REVOKED != 0 {
                return Err(NoiseTransportError::RegistryReserved);
            }
            let mut public_key = [0_u8; 32];
            public_key.copy_from_slice(&input[offset..offset + 32]);
            if public_key.iter().all(|byte| *byte == 0)
                || peers
                    .iter()
                    .any(|peer: &PeerRecord| peer.public_key == public_key)
            {
                return Err(if public_key.iter().all(|byte| *byte == 0) {
                    NoiseTransportError::RegistryKey
                } else {
                    NoiseTransportError::RegistryDuplicate
                });
            }
            let role = PresentationRole::from_raw(input[offset + 32])
                .ok_or(NoiseTransportError::RoleMismatch)?;
            if matches!(role, PresentationRole::ScriptedOperator) {
                return Err(NoiseTransportError::RoleMismatch);
            }
            peers.push(PeerRecord {
                public_key,
                role,
                revoked: flags & PEER_FLAG_REVOKED != 0,
            });
        }
        Ok(Self { peers })
    }

    pub fn confirm_pairing(
        &mut self,
        pairing: UnconfirmedNoiseChannel,
        locally_confirmed_code: ComparisonCode,
        assigned_role: PresentationRole,
    ) -> Result<AuthenticatedNoiseChannel, NoiseTransportError> {
        if pairing.comparison_code != locally_confirmed_code {
            return Err(NoiseTransportError::ComparisonMismatch);
        }
        if let Some(existing) = self
            .peers
            .iter()
            .find(|peer| peer.public_key == pairing.remote_static)
            .copied()
        {
            if existing.revoked {
                return Err(NoiseTransportError::RevokedPeer);
            }
            if existing.role != assigned_role {
                return Err(NoiseTransportError::RoleMismatch);
            }
            return Ok(AuthenticatedNoiseChannel::new(pairing.transport, existing));
        }
        if self.peers.len() >= MAX_PAIRED_PEERS {
            return Err(NoiseTransportError::PeerLimit);
        }
        let record = PeerRecord {
            public_key: pairing.remote_static,
            role: assigned_role,
            revoked: false,
        };
        self.peers.push(record);
        Ok(AuthenticatedNoiseChannel::new(pairing.transport, record))
    }

    pub fn lookup(&self, public_key: &[u8]) -> Result<PeerRecord, NoiseTransportError> {
        if public_key.len() != 32 {
            return Err(NoiseTransportError::RemoteStaticLength);
        }
        let record = self
            .peers
            .iter()
            .find(|peer| peer.public_key.as_slice() == public_key)
            .copied()
            .ok_or(NoiseTransportError::UnknownPeer)?;
        if record.revoked {
            return Err(NoiseTransportError::RevokedPeer);
        }
        Ok(record)
    }

    pub fn revoke(&mut self, public_key: &[u8; 32]) -> Result<(), NoiseTransportError> {
        let record = self
            .peers
            .iter_mut()
            .find(|peer| &peer.public_key == public_key)
            .ok_or(NoiseTransportError::UnknownPeer)?;
        record.revoked = true;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PairingWindow {
    started_millis: u64,
    timeout_millis: u64,
    max_attempts: u8,
    attempts: u8,
}

impl PairingWindow {
    pub fn new(
        started_millis: u64,
        timeout_millis: u64,
        max_attempts: u8,
    ) -> Result<Self, NoiseTransportError> {
        if timeout_millis == 0 || max_attempts == 0 {
            return Err(NoiseTransportError::PairingExpired);
        }
        Ok(Self {
            started_millis,
            timeout_millis,
            max_attempts,
            attempts: 0,
        })
    }

    pub fn register_attempt(&mut self, now_millis: u64) -> Result<(), NoiseTransportError> {
        if now_millis < self.started_millis
            || now_millis.saturating_sub(self.started_millis) > self.timeout_millis
        {
            return Err(NoiseTransportError::PairingExpired);
        }
        if self.attempts >= self.max_attempts {
            return Err(NoiseTransportError::PairingRateLimited);
        }
        self.attempts += 1;
        Ok(())
    }
}

pub struct XxInitiator {
    state: Option<HandshakeState>,
    stage: u8,
}

impl XxInitiator {
    #[cfg(feature = "std")]
    pub fn new(keys: &StaticNoiseKeypair) -> Result<Self, NoiseTransportError> {
        Self::with_entropy(keys, HandshakeEntropy::generate()?)
    }

    pub fn with_entropy(
        keys: &StaticNoiseKeypair,
        entropy: HandshakeEntropy,
    ) -> Result<Self, NoiseTransportError> {
        let state = builder_with_entropy(NOISE_XX_PATTERN, entropy)?
            .local_private_key(&keys.private)
            .map_err(|_| NoiseTransportError::Handshake)?
            .prologue(NOISE_PROLOGUE)
            .map_err(|_| NoiseTransportError::Handshake)?
            .build_initiator()
            .map_err(|_| NoiseTransportError::Handshake)?;
        Ok(Self {
            state: Some(state),
            stage: 0,
        })
    }

    pub fn write_first(&mut self) -> Result<Vec<u8>, NoiseTransportError> {
        if self.stage != 0 {
            return Err(NoiseTransportError::HandshakeOrder);
        }
        let message = write_handshake(self.state.as_mut().unwrap())?;
        self.stage = 1;
        Ok(message)
    }

    pub fn read_second(&mut self, message: &[u8]) -> Result<(), NoiseTransportError> {
        if self.stage != 1 {
            return Err(NoiseTransportError::HandshakeOrder);
        }
        read_handshake(self.state.as_mut().unwrap(), message)?;
        self.stage = 2;
        Ok(())
    }

    pub fn write_third_and_finish(
        mut self,
    ) -> Result<(Vec<u8>, UnconfirmedNoiseChannel), NoiseTransportError> {
        if self.stage != 2 {
            return Err(NoiseTransportError::HandshakeOrder);
        }
        let mut state = self.state.take().unwrap();
        let message = write_handshake(&mut state)?;
        let unconfirmed = finish_xx(state)?;
        self.stage = 3;
        Ok((message, unconfirmed))
    }
}

pub struct XxResponder {
    state: Option<HandshakeState>,
    stage: u8,
}

impl XxResponder {
    #[cfg(feature = "std")]
    pub fn new(keys: &StaticNoiseKeypair) -> Result<Self, NoiseTransportError> {
        Self::with_entropy(keys, HandshakeEntropy::generate()?)
    }

    pub fn with_entropy(
        keys: &StaticNoiseKeypair,
        entropy: HandshakeEntropy,
    ) -> Result<Self, NoiseTransportError> {
        let state = builder_with_entropy(NOISE_XX_PATTERN, entropy)?
            .local_private_key(&keys.private)
            .map_err(|_| NoiseTransportError::Handshake)?
            .prologue(NOISE_PROLOGUE)
            .map_err(|_| NoiseTransportError::Handshake)?
            .build_responder()
            .map_err(|_| NoiseTransportError::Handshake)?;
        Ok(Self {
            state: Some(state),
            stage: 0,
        })
    }

    pub fn read_first(&mut self, message: &[u8]) -> Result<(), NoiseTransportError> {
        if self.stage != 0 {
            return Err(NoiseTransportError::HandshakeOrder);
        }
        read_handshake(self.state.as_mut().unwrap(), message)?;
        self.stage = 1;
        Ok(())
    }

    pub fn write_second(&mut self) -> Result<Vec<u8>, NoiseTransportError> {
        if self.stage != 1 {
            return Err(NoiseTransportError::HandshakeOrder);
        }
        let message = write_handshake(self.state.as_mut().unwrap())?;
        self.stage = 2;
        Ok(message)
    }

    pub fn read_third_and_finish(
        mut self,
        message: &[u8],
    ) -> Result<UnconfirmedNoiseChannel, NoiseTransportError> {
        if self.stage != 2 {
            return Err(NoiseTransportError::HandshakeOrder);
        }
        let mut state = self.state.take().unwrap();
        read_handshake(&mut state, message)?;
        self.stage = 3;
        finish_xx(state)
    }
}

pub struct UnconfirmedNoiseChannel {
    transport: TransportState,
    remote_static: [u8; 32],
    handshake_hash: [u8; 32],
    comparison_code: ComparisonCode,
}

impl UnconfirmedNoiseChannel {
    pub const fn comparison_code(&self) -> ComparisonCode {
        self.comparison_code
    }

    pub const fn remote_static(&self) -> [u8; 32] {
        self.remote_static
    }

    pub const fn handshake_hash(&self) -> [u8; 32] {
        self.handshake_hash
    }
}

pub struct IkInitiator {
    state: Option<HandshakeState>,
    peer: PeerRecord,
    stage: u8,
}

impl IkInitiator {
    #[cfg(feature = "std")]
    pub fn new(keys: &StaticNoiseKeypair, peer: PeerRecord) -> Result<Self, NoiseTransportError> {
        Self::with_entropy(keys, peer, HandshakeEntropy::generate()?)
    }

    pub fn with_entropy(
        keys: &StaticNoiseKeypair,
        peer: PeerRecord,
        entropy: HandshakeEntropy,
    ) -> Result<Self, NoiseTransportError> {
        if peer.revoked {
            return Err(NoiseTransportError::RevokedPeer);
        }
        let state = builder_with_entropy(NOISE_IK_PATTERN, entropy)?
            .local_private_key(&keys.private)
            .map_err(|_| NoiseTransportError::Handshake)?
            .remote_public_key(&peer.public_key)
            .map_err(|_| NoiseTransportError::Handshake)?
            .prologue(NOISE_PROLOGUE)
            .map_err(|_| NoiseTransportError::Handshake)?
            .build_initiator()
            .map_err(|_| NoiseTransportError::Handshake)?;
        Ok(Self {
            state: Some(state),
            peer,
            stage: 0,
        })
    }

    pub fn write_first(&mut self) -> Result<Vec<u8>, NoiseTransportError> {
        if self.stage != 0 {
            return Err(NoiseTransportError::HandshakeOrder);
        }
        let message = write_handshake(self.state.as_mut().unwrap())?;
        self.stage = 1;
        Ok(message)
    }

    pub fn read_second_and_finish(
        mut self,
        message: &[u8],
    ) -> Result<AuthenticatedNoiseChannel, NoiseTransportError> {
        if self.stage != 1 {
            return Err(NoiseTransportError::HandshakeOrder);
        }
        let mut state = self.state.take().unwrap();
        read_handshake(&mut state, message)?;
        if !state.is_handshake_finished() {
            return Err(NoiseTransportError::HandshakeOrder);
        }
        let remote = copy_remote_static(&state)?;
        if remote != self.peer.public_key {
            return Err(NoiseTransportError::UnknownPeer);
        }
        let transport = state
            .into_transport_mode()
            .map_err(|_| NoiseTransportError::Noise)?;
        Ok(AuthenticatedNoiseChannel::new(transport, self.peer))
    }
}

pub struct IkResponder {
    state: Option<HandshakeState>,
    peer: Option<PeerRecord>,
    stage: u8,
}

impl IkResponder {
    #[cfg(feature = "std")]
    pub fn new(keys: &StaticNoiseKeypair) -> Result<Self, NoiseTransportError> {
        Self::with_entropy(keys, HandshakeEntropy::generate()?)
    }

    pub fn with_entropy(
        keys: &StaticNoiseKeypair,
        entropy: HandshakeEntropy,
    ) -> Result<Self, NoiseTransportError> {
        let state = builder_with_entropy(NOISE_IK_PATTERN, entropy)?
            .local_private_key(&keys.private)
            .map_err(|_| NoiseTransportError::Handshake)?
            .prologue(NOISE_PROLOGUE)
            .map_err(|_| NoiseTransportError::Handshake)?
            .build_responder()
            .map_err(|_| NoiseTransportError::Handshake)?;
        Ok(Self {
            state: Some(state),
            peer: None,
            stage: 0,
        })
    }

    pub fn read_first(
        &mut self,
        registry: &PeerRegistry,
        message: &[u8],
    ) -> Result<PeerRecord, NoiseTransportError> {
        if self.stage != 0 {
            return Err(NoiseTransportError::HandshakeOrder);
        }
        let state = self.state.as_mut().unwrap();
        read_handshake(state, message)?;
        let peer = registry.lookup(&copy_remote_static(state)?)?;
        self.peer = Some(peer);
        self.stage = 1;
        Ok(peer)
    }

    pub fn write_second_and_finish(
        mut self,
    ) -> Result<(Vec<u8>, AuthenticatedNoiseChannel), NoiseTransportError> {
        if self.stage != 1 {
            return Err(NoiseTransportError::HandshakeOrder);
        }
        let peer = self.peer.ok_or(NoiseTransportError::UnknownPeer)?;
        let mut state = self.state.take().unwrap();
        let message = write_handshake(&mut state)?;
        if !state.is_handshake_finished() {
            return Err(NoiseTransportError::HandshakeOrder);
        }
        let transport = state
            .into_transport_mode()
            .map_err(|_| NoiseTransportError::Noise)?;
        Ok((message, AuthenticatedNoiseChannel::new(transport, peer)))
    }
}

pub struct AuthenticatedNoiseChannel {
    transport: TransportState,
    peer: PeerRecord,
    inbound_sequence: Option<Kps1SequenceCursor>,
    outbound_sequence: Option<Kps1SequenceCursor>,
    next_send_message: u64,
    next_receive_message: u64,
    pending: Option<PendingAssembly>,
    poisoned: bool,
}

impl AuthenticatedNoiseChannel {
    fn new(transport: TransportState, peer: PeerRecord) -> Self {
        Self {
            transport,
            peer,
            inbound_sequence: None,
            outbound_sequence: None,
            next_send_message: 1,
            next_receive_message: 1,
            pending: None,
            poisoned: false,
        }
    }

    pub const fn peer(&self) -> PeerRecord {
        self.peer
    }

    pub fn bind_kps1_session(
        &mut self,
        session_nonce: u64,
        inbound_next_sequence: u64,
        outbound_next_sequence: u64,
    ) -> Result<(), NoiseTransportError> {
        if self.inbound_sequence.is_some() || self.outbound_sequence.is_some() {
            return Err(NoiseTransportError::HandshakeOrder);
        }
        self.inbound_sequence = Some(
            Kps1SequenceCursor::new(session_nonce, inbound_next_sequence)
                .map_err(NoiseTransportError::Kps1)?,
        );
        self.outbound_sequence = Some(
            Kps1SequenceCursor::new(session_nonce, outbound_next_sequence)
                .map_err(NoiseTransportError::Kps1)?,
        );
        Ok(())
    }

    pub fn seal_kps1(&mut self, frame: &[u8]) -> Result<Vec<Vec<u8>>, NoiseTransportError> {
        if self.poisoned {
            return Err(NoiseTransportError::ChannelPoisoned);
        }
        let result = self.seal_kps1_inner(frame);
        if result.is_err() {
            self.poisoned = true;
            self.pending = None;
        }
        result
    }

    fn seal_kps1_inner(&mut self, frame: &[u8]) -> Result<Vec<Vec<u8>>, NoiseTransportError> {
        if frame.len() > MAX_ENCRYPTED_KPS1_LENGTH {
            return Err(NoiseTransportError::FrameLength);
        }
        let decoded = parse_kps1_frame(frame).map_err(NoiseTransportError::Kps1)?;
        self.outbound_sequence
            .as_mut()
            .ok_or(NoiseTransportError::HandshakeOrder)?
            .accept(decoded.header)
            .map_err(NoiseTransportError::Kps1)?;
        let message_id = self.next_send_message;
        self.next_send_message = self
            .next_send_message
            .checked_add(1)
            .ok_or(NoiseTransportError::MessageOverflow)?;
        let chunk_count = frame.len().div_ceil(SECURE_FRAGMENT_DATA_LENGTH);
        let mut packets = Vec::with_capacity(chunk_count);
        for (index, chunk) in frame.chunks(SECURE_FRAGMENT_DATA_LENGTH).enumerate() {
            let offset = index
                .checked_mul(SECURE_FRAGMENT_DATA_LENGTH)
                .ok_or(NoiseTransportError::MessageOverflow)?;
            let final_fragment = index + 1 == chunk_count;
            let mut plaintext = vec![0_u8; SECURE_FRAGMENT_HEADER_LENGTH + chunk.len()];
            plaintext[..4].copy_from_slice(&SECURE_FRAGMENT_MAGIC);
            put_u64(&mut plaintext, 4, message_id);
            put_u32(&mut plaintext, 12, frame.len() as u32);
            put_u32(&mut plaintext, 16, offset as u32);
            put_u16(&mut plaintext, 20, chunk.len() as u16);
            put_u16(
                &mut plaintext,
                22,
                if final_fragment {
                    SECURE_FRAGMENT_FINAL
                } else {
                    0
                },
            );
            plaintext[SECURE_FRAGMENT_HEADER_LENGTH..].copy_from_slice(chunk);
            let mut ciphertext = vec![0_u8; plaintext.len() + 16];
            let length = self
                .transport
                .write_message(&plaintext, &mut ciphertext)
                .map_err(|_| NoiseTransportError::Noise)?;
            ciphertext.truncate(length);
            if ciphertext.len() > MAX_NOISE_CIPHERTEXT_LENGTH {
                return Err(NoiseTransportError::FrameLength);
            }
            let mut packet = Vec::with_capacity(4 + ciphertext.len());
            packet.extend_from_slice(&(ciphertext.len() as u32).to_be_bytes());
            packet.extend_from_slice(&ciphertext);
            packets.push(packet);
        }
        Ok(packets)
    }

    pub fn open_packet(&mut self, packet: &[u8]) -> Result<Option<Vec<u8>>, NoiseTransportError> {
        if self.poisoned {
            return Err(NoiseTransportError::ChannelPoisoned);
        }
        let result = self.open_packet_inner(packet);
        if result.is_err() {
            self.poisoned = true;
            self.pending = None;
        }
        result
    }

    fn open_packet_inner(&mut self, packet: &[u8]) -> Result<Option<Vec<u8>>, NoiseTransportError> {
        if packet.len() < 4 {
            return Err(NoiseTransportError::FrameLength);
        }
        let ciphertext_length = u32::from_be_bytes(packet[..4].try_into().unwrap()) as usize;
        if ciphertext_length == 0
            || ciphertext_length > MAX_NOISE_CIPHERTEXT_LENGTH
            || packet.len() != ciphertext_length + 4
        {
            return Err(NoiseTransportError::FrameLength);
        }
        let mut plaintext = vec![0_u8; ciphertext_length];
        let length = self
            .transport
            .read_message(&packet[4..], &mut plaintext)
            .map_err(|_| NoiseTransportError::Noise)?;
        plaintext.truncate(length);
        if plaintext.len() < SECURE_FRAGMENT_HEADER_LENGTH
            || plaintext[..4] != SECURE_FRAGMENT_MAGIC
        {
            return Err(NoiseTransportError::FragmentHeader);
        }
        let message_id = get_u64(&plaintext, 4);
        let total_length = get_u32(&plaintext, 12) as usize;
        let offset = get_u32(&plaintext, 16) as usize;
        let chunk_length = usize::from(get_u16(&plaintext, 20));
        let flags = get_u16(&plaintext, 22);
        if flags & !SECURE_FRAGMENT_FINAL != 0 {
            return Err(NoiseTransportError::FragmentFlags);
        }
        if !(KPS1_HEADER_LENGTH..=MAX_ENCRYPTED_KPS1_LENGTH).contains(&total_length)
            || chunk_length == 0
            || chunk_length > SECURE_FRAGMENT_DATA_LENGTH
            || plaintext.len() != SECURE_FRAGMENT_HEADER_LENGTH + chunk_length
        {
            return Err(NoiseTransportError::FragmentHeader);
        }
        let is_final = flags & SECURE_FRAGMENT_FINAL != 0;
        if message_id != self.next_receive_message {
            return Err(NoiseTransportError::FragmentOrder);
        }
        if self.pending.is_none() {
            if offset != 0 {
                return Err(NoiseTransportError::FragmentOrder);
            }
            self.pending = Some(PendingAssembly {
                message_id,
                total_length,
                data: Vec::with_capacity(total_length),
            });
        }
        let pending = self.pending.as_mut().unwrap();
        if pending.message_id != message_id
            || pending.total_length != total_length
            || pending.data.len() != offset
            || offset.checked_add(chunk_length) > Some(total_length)
        {
            return Err(NoiseTransportError::FragmentOrder);
        }
        pending
            .data
            .extend_from_slice(&plaintext[SECURE_FRAGMENT_HEADER_LENGTH..]);
        let complete = pending.data.len() == total_length;
        if is_final != complete {
            return Err(NoiseTransportError::FragmentFlags);
        }
        if !complete {
            return Ok(None);
        }
        let complete = self.pending.take().unwrap().data;
        let decoded = parse_kps1_frame(&complete).map_err(NoiseTransportError::Kps1)?;
        self.inbound_sequence
            .as_mut()
            .ok_or(NoiseTransportError::HandshakeOrder)?
            .accept(decoded.header)
            .map_err(NoiseTransportError::Kps1)?;
        self.next_receive_message = self
            .next_receive_message
            .checked_add(1)
            .ok_or(NoiseTransportError::MessageOverflow)?;
        Ok(Some(complete))
    }
}

struct PendingAssembly {
    message_id: u64,
    total_length: usize,
    data: Vec<u8>,
}

fn finish_xx(state: HandshakeState) -> Result<UnconfirmedNoiseChannel, NoiseTransportError> {
    if !state.is_handshake_finished() {
        return Err(NoiseTransportError::HandshakeOrder);
    }
    let remote_static = copy_remote_static(&state)?;
    let handshake_hash = copy_handshake_hash(&state)?;
    let comparison_code = comparison_code(&handshake_hash);
    let transport = state
        .into_transport_mode()
        .map_err(|_| NoiseTransportError::Noise)?;
    Ok(UnconfirmedNoiseChannel {
        transport,
        remote_static,
        handshake_hash,
        comparison_code,
    })
}

fn parse_params(pattern: &str) -> Result<NoiseParams, NoiseTransportError> {
    pattern.parse().map_err(|_| NoiseTransportError::Parameters)
}

fn write_handshake(state: &mut HandshakeState) -> Result<Vec<u8>, NoiseTransportError> {
    let mut output = vec![0_u8; MAX_HANDSHAKE_MESSAGE_LENGTH];
    let length = state
        .write_message(&[], &mut output)
        .map_err(|_| NoiseTransportError::Handshake)?;
    if length == 0 || length > MAX_HANDSHAKE_MESSAGE_LENGTH {
        return Err(NoiseTransportError::HandshakeLength);
    }
    output.truncate(length);
    Ok(output)
}

fn read_handshake(state: &mut HandshakeState, message: &[u8]) -> Result<(), NoiseTransportError> {
    if message.is_empty() || message.len() > MAX_HANDSHAKE_MESSAGE_LENGTH {
        return Err(NoiseTransportError::HandshakeLength);
    }
    let mut payload = [0_u8; 1];
    let length = state
        .read_message(message, &mut payload)
        .map_err(|_| NoiseTransportError::Handshake)?;
    if length != 0 {
        return Err(NoiseTransportError::HandshakeLength);
    }
    Ok(())
}

fn copy_remote_static(state: &HandshakeState) -> Result<[u8; 32], NoiseTransportError> {
    let value = state
        .get_remote_static()
        .ok_or(NoiseTransportError::RemoteStaticMissing)?;
    if value.len() != 32 {
        return Err(NoiseTransportError::RemoteStaticLength);
    }
    let mut output = [0_u8; 32];
    output.copy_from_slice(value);
    Ok(output)
}

fn copy_handshake_hash(state: &HandshakeState) -> Result<[u8; 32], NoiseTransportError> {
    let value = state.get_handshake_hash();
    if value.len() != 32 {
        return Err(NoiseTransportError::HandshakeLength);
    }
    let mut output = [0_u8; 32];
    output.copy_from_slice(value);
    Ok(output)
}

fn comparison_code(handshake_hash: &[u8; 32]) -> ComparisonCode {
    ComparisonCode(u32::from_le_bytes(handshake_hash[..4].try_into().unwrap()) % 1_000_000)
}

fn registry_crc32(input: &[u8]) -> u32 {
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

fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    output[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn get_u16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(input[offset..offset + 2].try_into().unwrap())
}

fn get_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap())
}

fn get_u64(input: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(input[offset..offset + 8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksa64_presentation::{
        write_kps1_frame, Kps1Header, PresentationMessageKind, KPS1_FLAG_COALESCED,
    };

    fn perform_xx() -> (
        StaticNoiseKeypair,
        StaticNoiseKeypair,
        UnconfirmedNoiseChannel,
        UnconfirmedNoiseChannel,
    ) {
        let keys_a = StaticNoiseKeypair::from_parts(
            [
                0x77, 0x07, 0x6d, 0x0a, 0x73, 0x18, 0xa5, 0x7d, 0x3c, 0x16, 0xc1, 0x72, 0x51, 0xb2,
                0x66, 0x45, 0xdf, 0x4c, 0x2f, 0x87, 0xeb, 0xc0, 0x99, 0x2a, 0xb1, 0x77, 0xfb, 0xa5,
                0x1d, 0xb9, 0x2c, 0x2a,
            ],
            [
                0x85, 0x20, 0xf0, 0x09, 0x89, 0x30, 0xa7, 0x54, 0x74, 0x8b, 0x7d, 0xdc, 0xb4, 0x3e,
                0xf7, 0x5a, 0x0d, 0xbf, 0x3a, 0x0d, 0x26, 0x38, 0x1a, 0xf4, 0xeb, 0xa4, 0xa9, 0x8e,
                0xaa, 0x9b, 0x4e, 0x6a,
            ],
        );
        let keys_b = StaticNoiseKeypair::from_parts(
            [
                0x5d, 0xab, 0x08, 0x7e, 0x62, 0x4a, 0x8a, 0x4b, 0x79, 0xe1, 0x7f, 0x8b, 0x83, 0x80,
                0x0e, 0xe6, 0x6f, 0x3b, 0xb1, 0x29, 0x26, 0x18, 0xb6, 0xfd, 0x1c, 0x2f, 0x8b, 0x27,
                0xff, 0x88, 0xe0, 0xeb,
            ],
            [
                0xde, 0x9e, 0xdb, 0x7d, 0x7b, 0x7d, 0xc1, 0xb4, 0xd3, 0x5b, 0x61, 0xc2, 0xec, 0xe4,
                0x35, 0x37, 0x3f, 0x83, 0x43, 0xc8, 0x5b, 0x78, 0x67, 0x4d, 0xad, 0xfc, 0x7e, 0x14,
                0x6f, 0x88, 0x2b, 0x4f,
            ],
        );
        let mut initiator =
            XxInitiator::with_entropy(&keys_a, HandshakeEntropy::from_bytes([0x11; 32])).unwrap();
        let mut responder =
            XxResponder::with_entropy(&keys_b, HandshakeEntropy::from_bytes([0x22; 32])).unwrap();
        let first = initiator.write_first().unwrap();
        responder.read_first(&first).unwrap();
        let second = responder.write_second().unwrap();
        initiator.read_second(&second).unwrap();
        let (third, initiator_channel) = initiator.write_third_and_finish().unwrap();
        let responder_channel = responder.read_third_and_finish(&third).unwrap();
        (keys_a, keys_b, initiator_channel, responder_channel)
    }

    fn frame(sequence: u64, payload_length: usize) -> Vec<u8> {
        let payload = vec![0x5a; payload_length];
        let header = Kps1Header {
            kind: PresentationMessageKind::Snapshot,
            flags: KPS1_FLAG_COALESCED,
            session_nonce: 42,
            sequence,
            correlation_id: 0,
            payload_length: payload_length as u32,
        };
        let mut output = vec![0_u8; KPS1_HEADER_LENGTH + payload.len()];
        write_kps1_frame(header, &payload, &mut output).unwrap();
        output
    }

    #[test]
    fn xx_pairing_requires_matching_local_comparison_and_binds_role() {
        let (keys_a, keys_b, initiator, responder) = perform_xx();
        assert_eq!(initiator.comparison_code(), responder.comparison_code());
        assert_eq!(initiator.remote_static(), keys_b.public_key());
        assert_eq!(responder.remote_static(), keys_a.public_key());
        assert_eq!(format!("{}", initiator.comparison_code()).len(), 6);

        let wrong = ComparisonCode((initiator.comparison_code().value() + 1) % 1_000_000);
        let mut rejected_registry = PeerRegistry::default();
        assert!(matches!(
            rejected_registry
                .confirm_pairing(initiator, wrong, PresentationRole::FlightController,),
            Err(NoiseTransportError::ComparisonMismatch)
        ));

        let (_, _, initiator, responder) = perform_xx();
        let code = initiator.comparison_code();
        let mut registry_a = PeerRegistry::default();
        let mut registry_b = PeerRegistry::default();
        let channel_a = registry_a
            .confirm_pairing(initiator, code, PresentationRole::FlightController)
            .unwrap();
        let channel_b = registry_b
            .confirm_pairing(responder, code, PresentationRole::GuidedOperator)
            .unwrap();
        assert_eq!(channel_a.peer().role, PresentationRole::FlightController);
        assert_eq!(channel_b.peer().role, PresentationRole::GuidedOperator);
    }

    #[test]
    fn ik_reconnect_authenticates_stored_peer_and_honors_revocation() {
        let (keys_a, keys_b, initiator, responder) = perform_xx();
        let code = initiator.comparison_code();
        let mut registry_a = PeerRegistry::default();
        let mut registry_b = PeerRegistry::default();
        let _ = registry_a
            .confirm_pairing(initiator, code, PresentationRole::FlightController)
            .unwrap();
        let _ = registry_b
            .confirm_pairing(responder, code, PresentationRole::GuidedOperator)
            .unwrap();

        let peer_b = registry_a.lookup(&keys_b.public_key()).unwrap();
        let mut ik_a =
            IkInitiator::with_entropy(&keys_a, peer_b, HandshakeEntropy::from_bytes([0x33; 32]))
                .unwrap();
        let mut ik_b =
            IkResponder::with_entropy(&keys_b, HandshakeEntropy::from_bytes([0x44; 32])).unwrap();
        let first = ik_a.write_first().unwrap();
        assert_eq!(
            ik_b.read_first(&registry_b, &first).unwrap().public_key,
            keys_a.public_key()
        );
        let (second, _) = ik_b.write_second_and_finish().unwrap();
        let _ = ik_a.read_second_and_finish(&second).unwrap();

        registry_b.revoke(&keys_a.public_key()).unwrap();
        let peer_b = registry_a.lookup(&keys_b.public_key()).unwrap();
        let mut ik_a =
            IkInitiator::with_entropy(&keys_a, peer_b, HandshakeEntropy::from_bytes([0x33; 32]))
                .unwrap();
        let mut ik_b =
            IkResponder::with_entropy(&keys_b, HandshakeEntropy::from_bytes([0x44; 32])).unwrap();
        let first = ik_a.write_first().unwrap();
        assert_eq!(
            ik_b.read_first(&registry_b, &first),
            Err(NoiseTransportError::RevokedPeer)
        );
    }

    #[test]
    fn encrypted_kps1_fragments_reassemble_strictly_and_enforce_sequences() {
        let (_, _, initiator, responder) = perform_xx();
        let code = initiator.comparison_code();
        let mut registry_a = PeerRegistry::default();
        let mut registry_b = PeerRegistry::default();
        let mut channel_a = registry_a
            .confirm_pairing(initiator, code, PresentationRole::FlightController)
            .unwrap();
        let mut channel_b = registry_b
            .confirm_pairing(responder, code, PresentationRole::GuidedOperator)
            .unwrap();
        channel_a.bind_kps1_session(42, 1, 1).unwrap();
        channel_b.bind_kps1_session(42, 1, 1).unwrap();

        let large = frame(1, 200_000);
        let packets = channel_a.seal_kps1(&large).unwrap();
        assert!(packets.len() > 1);
        let mut received = None;
        for packet in packets {
            received = channel_b.open_packet(&packet).unwrap().or(received);
        }
        assert_eq!(received.as_deref(), Some(large.as_slice()));

        let stale = frame(1, 0);
        assert_eq!(
            channel_a.seal_kps1(&stale),
            Err(NoiseTransportError::Kps1(Kps1Error::Sequence))
        );
    }

    #[test]
    fn encrypted_transport_rejects_tampering_truncation_and_fragment_reordering() {
        let (_, _, initiator, responder) = perform_xx();
        let code = initiator.comparison_code();
        let mut registry_a = PeerRegistry::default();
        let mut registry_b = PeerRegistry::default();
        let mut channel_a = registry_a
            .confirm_pairing(initiator, code, PresentationRole::FlightController)
            .unwrap();
        let mut channel_b = registry_b
            .confirm_pairing(responder, code, PresentationRole::GuidedOperator)
            .unwrap();
        channel_a.bind_kps1_session(42, 1, 1).unwrap();
        channel_b.bind_kps1_session(42, 1, 1).unwrap();
        let packets = channel_a.seal_kps1(&frame(1, 100_000)).unwrap();

        let mut truncated = packets[0].clone();
        truncated.pop();
        assert_eq!(
            channel_b.open_packet(&truncated),
            Err(NoiseTransportError::FrameLength)
        );
        assert_eq!(
            channel_b.open_packet(&packets[0]),
            Err(NoiseTransportError::ChannelPoisoned)
        );

        // A fresh pair avoids consuming a Noise transport nonce after the
        // intentionally malformed outer packet.
        let (_, _, initiator, responder) = perform_xx();
        let code = initiator.comparison_code();
        let mut registry_a = PeerRegistry::default();
        let mut registry_b = PeerRegistry::default();
        let mut channel_a = registry_a
            .confirm_pairing(initiator, code, PresentationRole::FlightController)
            .unwrap();
        let mut channel_b = registry_b
            .confirm_pairing(responder, code, PresentationRole::GuidedOperator)
            .unwrap();
        channel_a.bind_kps1_session(42, 1, 1).unwrap();
        channel_b.bind_kps1_session(42, 1, 1).unwrap();
        let mut packets = channel_a.seal_kps1(&frame(1, 100_000)).unwrap();
        packets[0][10] ^= 0x80;
        assert_eq!(
            channel_b.open_packet(&packets[0]),
            Err(NoiseTransportError::Noise)
        );
    }

    #[test]
    fn outbound_error_permanently_poisons_channel() {
        let (keys_a, keys_b, initiator, responder) = perform_xx();
        let code = initiator.comparison_code();
        let mut registry_a = PeerRegistry::default();
        let mut registry_b = PeerRegistry::default();
        let mut channel_a = registry_a
            .confirm_pairing(initiator, code, PresentationRole::FlightController)
            .unwrap();
        let _channel_b = registry_b
            .confirm_pairing(responder, code, PresentationRole::GuidedOperator)
            .unwrap();
        channel_a.bind_kps1_session(42, 1, 1).unwrap();
        let invalid = vec![0_u8; KPS1_HEADER_LENGTH];
        assert!(matches!(
            channel_a.seal_kps1(&invalid),
            Err(NoiseTransportError::Kps1(_))
        ));
        assert_eq!(
            channel_a.seal_kps1(&frame(1, 0)),
            Err(NoiseTransportError::ChannelPoisoned)
        );
        assert_ne!(keys_a.public_key(), keys_b.public_key());
    }

    #[test]
    fn pairing_window_expires_and_rate_limits() {
        let mut window = PairingWindow::new(1_000, 100, 2).unwrap();
        assert_eq!(window.register_attempt(1_000), Ok(()));
        assert_eq!(window.register_attempt(1_050), Ok(()));
        assert_eq!(
            window.register_attempt(1_060),
            Err(NoiseTransportError::PairingRateLimited)
        );
        assert_eq!(
            window.register_attempt(1_101),
            Err(NoiseTransportError::PairingExpired)
        );
    }
    #[test]
    fn peer_registry_round_trips_strictly_and_rejects_corruption() {
        let (_, _, initiator_pairing, responder_pairing) = perform_xx();
        let mut registry = PeerRegistry::default();
        registry
            .confirm_pairing(
                responder_pairing,
                initiator_pairing.comparison_code(),
                PresentationRole::FlightController,
            )
            .unwrap();
        let public_key = registry.records()[0].public_key;
        registry.revoke(&public_key).unwrap();

        let encoded = registry.export_bounded().unwrap();
        assert!(encoded.len() <= PEER_REGISTRY_MAX_LENGTH);
        let decoded = PeerRegistry::import_bounded(&encoded).unwrap();
        assert_eq!(decoded.records(), registry.records());
        assert_eq!(
            decoded.lookup(&public_key),
            Err(NoiseTransportError::RevokedPeer)
        );

        let mut corrupt = encoded.clone();
        corrupt[PEER_REGISTRY_HEADER_LENGTH + 3] ^= 0x80;
        assert!(matches!(
            PeerRegistry::import_bounded(&corrupt),
            Err(NoiseTransportError::RegistryCrc)
        ));

        let mut reserved = encoded.clone();
        reserved[12] = 1;
        let trailer = reserved.len() - PEER_REGISTRY_TRAILER_LENGTH;
        let crc = registry_crc32(&reserved[..trailer]);
        put_u32(&mut reserved, trailer, crc);
        assert!(matches!(
            PeerRegistry::import_bounded(&reserved),
            Err(NoiseTransportError::RegistryReserved)
        ));

        let duplicate = PeerRegistry {
            peers: vec![registry.records()[0], registry.records()[0]],
        };
        assert_eq!(
            duplicate.export_bounded(),
            Err(NoiseTransportError::RegistryDuplicate)
        );
    }
}
