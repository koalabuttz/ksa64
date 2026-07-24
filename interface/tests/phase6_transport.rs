use ksa64_interface::phase6::{LinkFrame, LinkHeader, LinkRecordType, KLF6_NONE};
use ksa64_interface::phase6_transport::*;

#[test]
fn bounded_queue_wraps_without_allocation() {
    let mut q = ByteQueue::<3>::new();
    q.push(1).unwrap();
    q.push(2).unwrap();
    q.push(3).unwrap();
    assert_eq!(q.push(4), Err(TransportError::Capacity));
    assert_eq!(q.pop(), Some(1));
    q.push(4).unwrap();
    assert_eq!(
        (q.pop(), q.pop(), q.pop(), q.pop()),
        (Some(2), Some(3), Some(4), None)
    );
}

#[test]
fn transmitter_and_incremental_receiver_survive_backpressure() {
    let frame = LinkFrame {
        header: LinkHeader {
            record_type: LinkRecordType::Heartbeat,
            flags: 0,
            session_id: 9,
            sequence: 12,
            acknowledgement: KLF6_NONE,
            measurement_epoch: 1,
            production_epoch: 2,
            effective_epoch: 3,
        },
        payload: b"transport",
    };
    let mut tx = LinkTransmitter::new();
    tx.stage(&frame).unwrap();
    let mut wire = MemoryTransport::<1, 8>::new();
    let mut receiver = LinkReceiver::new();
    loop {
        let _ = tx.poll(&mut wire).unwrap();
        if let Some(byte) = wire.take_transmitted() {
            wire.inject(byte).unwrap();
        }
        if let Some(got) = receiver.poll(&mut wire).unwrap() {
            assert_eq!(got.header, frame.header);
            assert_eq!(got.payload, b"transport");
            break;
        }
    }
    assert!(tx.is_idle());
}

#[derive(Default)]
struct MockAcia {
    status: u8,
    rx: u8,
    tx: Option<u8>,
}
impl AciaRegisters for MockAcia {
    fn status(&mut self) -> u8 {
        self.status
    }
    fn read_data(&mut self) -> u8 {
        self.rx
    }
    fn write_data(&mut self, byte: u8) {
        self.tx = Some(byte)
    }
}
#[test]
fn acia_driver_is_nonblocking_and_fails_closed() {
    let mut link = AciaTransport::new(MockAcia {
        status: 0,
        rx: 42,
        tx: None,
    });
    assert_eq!(link.try_read().unwrap(), None);
    assert!(!link.try_write(7).unwrap());
    link.registers_mut().status = ACIA_STATUS_RX_READY | ACIA_STATUS_TX_EMPTY;
    assert_eq!(link.try_read().unwrap(), Some(42));
    assert!(link.try_write(7).unwrap());
    assert_eq!(link.registers().tx, Some(7));
    link.registers_mut().status = 1;
    assert_eq!(link.try_read(), Err(TransportError::Fault));
    assert!(!link.is_connected());
}

struct MockUci {
    polls: u8,
    begin: u8,
    rx: Option<u8>,
    tx: ByteQueue<2>,
    fail: bool,
}
impl UciTcpDevice for MockUci {
    fn begin_connect(&mut self) -> Result<(), TransportError> {
        self.begin += 1;
        Ok(())
    }
    fn poll_connect(&mut self) -> UciPoll {
        self.polls += 1;
        if self.polls < 2 {
            UciPoll::Pending
        } else {
            UciPoll::Connected
        }
    }
    fn try_receive(&mut self) -> Result<Option<u8>, TransportError> {
        if self.fail {
            Err(TransportError::Fault)
        } else {
            Ok(self.rx.take())
        }
    }
    fn try_send(&mut self, byte: u8) -> Result<bool, TransportError> {
        if self.fail {
            Err(TransportError::Fault)
        } else if self.tx.remaining() == 0 {
            Ok(false)
        } else {
            self.tx.push(byte)?;
            Ok(true)
        }
    }
}
#[test]
fn ultimate_uci_mock_connects_backpressures_and_latches_fault() {
    let device = MockUci {
        polls: 0,
        begin: 0,
        rx: Some(33),
        tx: ByteQueue::new(),
        fail: false,
    };
    let mut link = UltimateUciTransport::new(device);
    assert_eq!(link.try_read(), Err(TransportError::Disconnected));
    link.connect().unwrap();
    link.poll();
    assert_eq!(link.state(), UciState::Connecting);
    link.poll();
    assert!(link.is_connected());
    assert_eq!(link.try_read().unwrap(), Some(33));
    assert!(link.try_write(1).unwrap());
    assert!(link.try_write(2).unwrap());
    assert!(!link.try_write(3).unwrap());
    link.device_mut().fail = true;
    assert_eq!(link.try_read(), Err(TransportError::Fault));
    assert_eq!(link.state(), UciState::Faulted);
}

#[test]
fn serial_budget_selects_paced_user_port_and_realtime_acia() {
    let user_world = serial_slot_budget(USER_PORT_BAUD, REALTIME_WORLD_WORST_BYTES);
    let user_flight = serial_slot_budget(USER_PORT_BAUD, REALTIME_FLIGHT_WORST_BYTES);
    assert_eq!(user_world.wire_microseconds, 108_334);
    assert_eq!(user_flight.wire_microseconds, 75_000);
    assert!(!user_world.fits_fast_slot && !user_flight.fits_fast_slot);
    let acia_world = serial_slot_budget(TURBO232_BAUD, REALTIME_WORLD_WORST_BYTES);
    let acia_flight = serial_slot_budget(TURBO232_BAUD, REALTIME_FLIGHT_WORST_BYTES);
    assert_eq!(acia_world.wire_microseconds, 18_056);
    assert_eq!(acia_flight.wire_microseconds, 12_500);
    assert!(acia_world.fits_fast_slot && acia_flight.fits_fast_slot);
}

#[test]
fn realtime_cell_receiver_resynchronizes_and_checks_crc() {
    use ksa64_interface::phase6::{
        write_realtime_command, RealtimeCommandCell, REALTIME_COMMAND_LENGTH,
    };
    let cell = RealtimeCommandCell {
        session: 1,
        source_epoch: 2,
        effective_epoch: 3,
        flags: 0,
        discrete: 0,
        gimbal: [4, 5],
        rcs: [6, 7, 8],
        status: 0,
    };
    let mut bytes = [0u8; REALTIME_COMMAND_LENGTH];
    write_realtime_command(&cell, &mut bytes).unwrap();
    let mut wire = MemoryTransport::<128, 1>::new();
    for b in [9, 8, 7] {
        wire.inject(b).unwrap()
    }
    for b in bytes {
        wire.inject(b).unwrap()
    }
    let mut receiver = RealtimeCellReceiver::new();
    let (kind, got) = receiver.poll(&mut wire).unwrap().unwrap();
    assert_eq!(kind, RealtimeCellKind::Command);
    assert_eq!(got, &bytes);
    let mut corrupt = bytes;
    corrupt[12] ^= 1;
    for b in corrupt {
        wire.inject(b).unwrap()
    }
    assert_eq!(
        receiver.poll(&mut wire),
        Err(RealtimeReceiveError::Checksum)
    );
}
