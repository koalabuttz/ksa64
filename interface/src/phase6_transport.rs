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
        self.device.try_receive().map_err(|e| {
            self.state = UciState::Faulted;
            e
        })
    }
    fn try_write(&mut self, byte: u8) -> Result<bool, TransportError> {
        if self.state != UciState::Connected {
            return Err(TransportError::Disconnected);
        }
        self.device.try_send(byte).map_err(|e| {
            self.state = UciState::Faulted;
            e
        })
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
