//! Allocation-free Phase 6 byte transports and bounded link pumps.
use crate::phase6::{
    parse_link_frame, LinkCodecError, LinkFrame, KLF6_MAX_DECODED, KLF6_MAX_ENCODED,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportError {
    Disconnected,
    Fault,
    Capacity,
}

/// Nonblocking byte stream used by every Phase 6 physical and emulated link.
pub trait ByteTransport {
    fn try_read(&mut self) -> Result<Option<u8>, TransportError>;
    fn try_write(&mut self, byte: u8) -> Result<bool, TransportError>;
    fn is_connected(&self) -> bool;
}

/// Fixed storage with no allocator and deterministic full/empty behavior.
pub struct ByteQueue<const N: usize> {
    bytes: [u8; N],
    read: usize,
    write: usize,
    length: usize,
}
impl<const N: usize> ByteQueue<N> {
    pub const fn new() -> Self {
        Self {
            bytes: [0; N],
            read: 0,
            write: 0,
            length: 0,
        }
    }
    pub const fn len(&self) -> usize {
        self.length
    }
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }
    pub const fn remaining(&self) -> usize {
        N - self.length
    }
    pub fn push(&mut self, byte: u8) -> Result<(), TransportError> {
        if self.length == N {
            return Err(TransportError::Capacity);
        }
        self.bytes[self.write] = byte;
        self.write = if self.write + 1 == N {
            0
        } else {
            self.write + 1
        };
        self.length += 1;
        Ok(())
    }
    pub fn pop(&mut self) -> Option<u8> {
        if self.length == 0 {
            return None;
        }
        let byte = self.bytes[self.read];
        self.read = if self.read + 1 == N { 0 } else { self.read + 1 };
        self.length -= 1;
        Some(byte)
    }
}
impl<const N: usize> Default for ByteQueue<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Deterministic host/mock transport. Move bytes between peers explicitly.
pub struct MemoryTransport<const RX: usize, const TX: usize> {
    receive: ByteQueue<RX>,
    transmit: ByteQueue<TX>,
    connected: bool,
}
impl<const RX: usize, const TX: usize> MemoryTransport<RX, TX> {
    pub const fn new() -> Self {
        Self {
            receive: ByteQueue::new(),
            transmit: ByteQueue::new(),
            connected: true,
        }
    }
    pub fn inject(&mut self, byte: u8) -> Result<(), TransportError> {
        self.receive.push(byte)
    }
    pub fn take_transmitted(&mut self) -> Option<u8> {
        self.transmit.pop()
    }
    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected
    }
    pub const fn pending_transmit(&self) -> usize {
        self.transmit.len()
    }
}
impl<const RX: usize, const TX: usize> Default for MemoryTransport<RX, TX> {
    fn default() -> Self {
        Self::new()
    }
}
impl<const RX: usize, const TX: usize> ByteTransport for MemoryTransport<RX, TX> {
    fn try_read(&mut self) -> Result<Option<u8>, TransportError> {
        if !self.connected {
            return Err(TransportError::Disconnected);
        }
        Ok(self.receive.pop())
    }
    fn try_write(&mut self, byte: u8) -> Result<bool, TransportError> {
        if !self.connected {
            return Err(TransportError::Disconnected);
        }
        if self.transmit.remaining() == 0 {
            return Ok(false);
        }
        self.transmit.push(byte)?;
        Ok(true)
    }
    fn is_connected(&self) -> bool {
        self.connected
    }
}

pub trait AciaRegisters {
    fn status(&mut self) -> u8;
    fn read_data(&mut self) -> u8;
    fn write_data(&mut self, byte: u8);
}
pub const ACIA_STATUS_RX_READY: u8 = 0x08;
pub const ACIA_STATUS_TX_EMPTY: u8 = 0x10;
pub const ACIA_STATUS_FAULT: u8 = 0x07;

/// Register-neutral SwiftLink/Turbo232 nonblocking driver.
pub struct AciaTransport<R> {
    registers: R,
    connected: bool,
}
impl<R> AciaTransport<R> {
    pub const fn new(registers: R) -> Self {
        Self {
            registers,
            connected: true,
        }
    }
    pub fn registers(&self) -> &R {
        &self.registers
    }
    pub fn registers_mut(&mut self) -> &mut R {
        &mut self.registers
    }
}
impl<R: AciaRegisters> ByteTransport for AciaTransport<R> {
    fn try_read(&mut self) -> Result<Option<u8>, TransportError> {
        if !self.connected {
            return Err(TransportError::Disconnected);
        }
        let status = self.registers.status();
        if status & ACIA_STATUS_FAULT != 0 {
            self.connected = false;
            return Err(TransportError::Fault);
        }
        Ok(if status & ACIA_STATUS_RX_READY != 0 {
            Some(self.registers.read_data())
        } else {
            None
        })
    }
    fn try_write(&mut self, byte: u8) -> Result<bool, TransportError> {
        if !self.connected {
            return Err(TransportError::Disconnected);
        }
        let status = self.registers.status();
        if status & ACIA_STATUS_FAULT != 0 {
            self.connected = false;
            return Err(TransportError::Fault);
        }
        if status & ACIA_STATUS_TX_EMPTY == 0 {
            return Ok(false);
        }
        self.registers.write_data(byte);
        Ok(true)
    }
    fn is_connected(&self) -> bool {
        self.connected
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UciPoll {
    Pending,
    Connected,
    Fault,
}
pub trait UciTcpDevice {
    fn begin_connect(&mut self) -> Result<(), TransportError>;
    fn poll_connect(&mut self) -> UciPoll;
    fn try_receive(&mut self) -> Result<Option<u8>, TransportError>;
    fn try_send(&mut self, byte: u8) -> Result<bool, TransportError>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UciState {
    Idle,
    Connecting,
    Connected,
    Faulted,
}
/// State machine shared by the Ultimate UCI adapter and its mandatory native mock.
pub struct UltimateUciTransport<D> {
    device: D,
    state: UciState,
}
impl<D> UltimateUciTransport<D> {
    pub const fn new(device: D) -> Self {
        Self {
            device,
            state: UciState::Idle,
        }
    }
    pub const fn state(&self) -> UciState {
        self.state
    }
    pub fn device(&self) -> &D {
        &self.device
    }
    pub fn device_mut(&mut self) -> &mut D {
        &mut self.device
    }
}
impl<D: UciTcpDevice> UltimateUciTransport<D> {
    pub fn connect(&mut self) -> Result<(), TransportError> {
        if self.state != UciState::Idle && self.state != UciState::Faulted {
            return Ok(());
        }
        self.device.begin_connect()?;
        self.state = UciState::Connecting;
        Ok(())
    }
    pub fn poll(&mut self) {
        if self.state == UciState::Connecting {
            self.state = match self.device.poll_connect() {
                UciPoll::Pending => UciState::Connecting,
                UciPoll::Connected => UciState::Connected,
                UciPoll::Fault => UciState::Faulted,
            }
        }
    }
}
impl<D: UciTcpDevice> ByteTransport for UltimateUciTransport<D> {
    fn try_read(&mut self) -> Result<Option<u8>, TransportError> {
        if self.state != UciState::Connected {
            return Err(TransportError::Disconnected);
        }
        match self.device.try_receive() {
            Ok(value) => Ok(value),
            Err(error) => {
                self.state = UciState::Faulted;
                Err(error)
            }
        }
    }
    fn try_write(&mut self, byte: u8) -> Result<bool, TransportError> {
        if self.state != UciState::Connected {
            return Err(TransportError::Disconnected);
        }
        match self.device.try_send(byte) {
            Ok(value) => Ok(value),
            Err(error) => {
                self.state = UciState::Faulted;
                Err(error)
            }
        }
    }
    fn is_connected(&self) -> bool {
        self.state == UciState::Connected
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiveError {
    Transport(TransportError),
    Codec(LinkCodecError),
    Overflow,
}
/// Incremental delimiter receiver. A bad or oversized frame is discarded at its boundary.
pub struct LinkReceiver {
    encoded: [u8; KLF6_MAX_ENCODED],
    decoded: [u8; KLF6_MAX_DECODED],
    length: usize,
    overflow: bool,
}
impl LinkReceiver {
    pub const fn new() -> Self {
        Self {
            encoded: [0; KLF6_MAX_ENCODED],
            decoded: [0; KLF6_MAX_DECODED],
            length: 0,
            overflow: false,
        }
    }
    pub fn poll<'a, T: ByteTransport>(
        &'a mut self,
        transport: &mut T,
    ) -> Result<Option<LinkFrame<'a>>, ReceiveError> {
        loop {
            let Some(byte) = transport.try_read().map_err(ReceiveError::Transport)? else {
                return Ok(None);
            };
            if byte != 0 {
                if self.length + 1 < self.encoded.len() {
                    self.encoded[self.length] = byte;
                    self.length += 1;
                } else {
                    self.overflow = true;
                }
                continue;
            }
            if self.overflow {
                self.length = 0;
                self.overflow = false;
                return Err(ReceiveError::Overflow);
            }
            if self.length == 0 {
                continue;
            }
            self.encoded[self.length] = 0;
            let n = self.length + 1;
            self.length = 0;
            return parse_link_frame(&self.encoded[..n], &mut self.decoded)
                .map(Some)
                .map_err(ReceiveError::Codec);
        }
    }
}
impl Default for LinkReceiver {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LinkTransmitter {
    encoded: [u8; KLF6_MAX_ENCODED],
    length: usize,
    offset: usize,
}
impl LinkTransmitter {
    pub const fn new() -> Self {
        Self {
            encoded: [0; KLF6_MAX_ENCODED],
            length: 0,
            offset: 0,
        }
    }
    pub const fn is_idle(&self) -> bool {
        self.offset == self.length
    }
    pub fn stage(&mut self, frame: &LinkFrame<'_>) -> Result<(), LinkCodecError> {
        if !self.is_idle() {
            return Err(LinkCodecError::Capacity);
        }
        let mut decoded = [0u8; KLF6_MAX_DECODED];
        self.length = crate::phase6::write_link_frame(frame, &mut decoded, &mut self.encoded)?;
        self.offset = 0;
        Ok(())
    }
    pub fn poll<T: ByteTransport>(&mut self, transport: &mut T) -> Result<bool, TransportError> {
        while self.offset < self.length {
            if !transport.try_write(self.encoded[self.offset])? {
                return Ok(false);
            }
            self.offset += 1;
        }
        Ok(true)
    }
}
impl Default for LinkTransmitter {
    fn default() -> Self {
        Self::new()
    }
}

pub const SERIAL_BITS_PER_BYTE_8N1: u32 = 10;
pub const USER_PORT_BAUD: u32 = 9_600;
pub const TURBO232_BAUD: u32 = 57_600;
pub const PAL_FAST_SLOT_MICROSECONDS: u32 = 31_250;
pub const REALTIME_WORLD_FAST_BYTES: u16 = crate::phase6::REALTIME_INERTIAL_LENGTH as u16;
pub const REALTIME_WORLD_WORST_BYTES: u16 =
    (crate::phase6::REALTIME_INERTIAL_LENGTH + crate::phase6::REALTIME_AID_LENGTH) as u16;
pub const REALTIME_FLIGHT_FAST_BYTES: u16 = crate::phase6::REALTIME_COMMAND_LENGTH as u16;
pub const REALTIME_FLIGHT_WORST_BYTES: u16 =
    (crate::phase6::REALTIME_COMMAND_LENGTH + crate::phase6::REALTIME_STATUS_LENGTH) as u16;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SerialSlotBudget {
    pub baud: u32,
    pub bytes: u16,
    pub wire_microseconds: u32,
    pub fits_fast_slot: bool,
}
pub const fn serial_slot_budget(baud: u32, bytes: u16) -> SerialSlotBudget {
    let micros = if baud == 0 {
        u32::MAX
    } else {
        let bit_microseconds = bytes as u32 * SERIAL_BITS_PER_BYTE_8N1 * 1_000_000;
        let quotient = bit_microseconds / baud;
        let remainder = bit_microseconds - quotient * baud;
        quotient + if remainder == 0 { 0 } else { 1 }
    };
    SerialSlotBudget {
        baud,
        bytes,
        wire_microseconds: micros,
        fits_fast_slot: micros <= PAL_FAST_SLOT_MICROSECONDS,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RealtimeCellKind {
    Inertial,
    Command,
    Aid,
    Status,
}
impl RealtimeCellKind {
    pub const fn length(self) -> usize {
        match self {
            Self::Inertial => crate::phase6::REALTIME_INERTIAL_LENGTH,
            Self::Command => crate::phase6::REALTIME_COMMAND_LENGTH,
            Self::Aid => crate::phase6::REALTIME_AID_LENGTH,
            Self::Status => crate::phase6::REALTIME_STATUS_LENGTH,
        }
    }
    fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Inertial),
            2 => Some(Self::Command),
            3 => Some(Self::Aid),
            4 => Some(Self::Status),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RealtimeReceiveError {
    Transport(TransportError),
    Checksum,
}
pub struct RealtimeCellReceiver {
    bytes: [u8; crate::phase6::REALTIME_AID_LENGTH],
    length: u8,
    needed: u8,
}
impl RealtimeCellReceiver {
    pub const fn new() -> Self {
        Self {
            bytes: [0; crate::phase6::REALTIME_AID_LENGTH],
            length: 0,
            needed: 0,
        }
    }
    pub fn poll<'a, T: ByteTransport>(
        &'a mut self,
        transport: &mut T,
    ) -> Result<Option<(RealtimeCellKind, &'a [u8])>, RealtimeReceiveError> {
        loop {
            let Some(byte) = transport
                .try_read()
                .map_err(RealtimeReceiveError::Transport)?
            else {
                return Ok(None);
            };
            if self.length == 0 {
                if byte != crate::phase6::KLR6_SYNC[0] {
                    continue;
                }
                self.bytes[0] = byte;
                self.length = 1;
                continue;
            }
            if self.length == 1 {
                if byte != crate::phase6::KLR6_SYNC[1] {
                    self.length = if byte == crate::phase6::KLR6_SYNC[0] {
                        1
                    } else {
                        0
                    };
                    continue;
                }
                self.bytes[1] = byte;
                self.length = 2;
                continue;
            }
            if self.length == 2 {
                if byte != 6 {
                    self.length = 0;
                    continue;
                }
                self.bytes[2] = byte;
                self.length = 3;
                continue;
            }
            if self.length == 3 {
                let Some(kind) = RealtimeCellKind::from_tag(byte) else {
                    self.length = 0;
                    continue;
                };
                self.bytes[3] = byte;
                self.length = 4;
                self.needed = kind.length() as u8;
                continue;
            }
            self.bytes[self.length as usize] = byte;
            self.length = self.length.wrapping_add(1);
            if self.length == self.needed {
                let kind = RealtimeCellKind::from_tag(self.bytes[3]).unwrap();
                let n = self.length as usize;
                self.length = 0;
                let expected = u16::from_le_bytes([self.bytes[n - 2], self.bytes[n - 1]]);
                if crate::phase6::crc16_ccitt(&self.bytes[..n - 2]) != expected {
                    return Err(RealtimeReceiveError::Checksum);
                }
                return Ok(Some((kind, &self.bytes[..n])));
            }
        }
    }
}
impl Default for RealtimeCellReceiver {
    fn default() -> Self {
        Self::new()
    }
}
pub struct ByteTransmitter<const N: usize> {
    bytes: [u8; N],
    length: usize,
    offset: usize,
}
impl<const N: usize> ByteTransmitter<N> {
    pub const fn new() -> Self {
        Self {
            bytes: [0; N],
            length: 0,
            offset: 0,
        }
    }
    pub const fn is_idle(&self) -> bool {
        self.offset == self.length
    }
    pub fn stage(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        if !self.is_idle() || bytes.len() > N {
            return Err(TransportError::Capacity);
        }
        self.bytes[..bytes.len()].copy_from_slice(bytes);
        self.length = bytes.len();
        self.offset = 0;
        Ok(())
    }
    pub fn poll<T: ByteTransport>(&mut self, transport: &mut T) -> Result<bool, TransportError> {
        while self.offset < self.length {
            if !transport.try_write(self.bytes[self.offset])? {
                return Ok(false);
            }
            self.offset += 1
        }
        Ok(true)
    }
}
impl<const N: usize> Default for ByteTransmitter<N> {
    fn default() -> Self {
        Self::new()
    }
}
