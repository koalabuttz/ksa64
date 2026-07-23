use ksa64_sim::phase4::reu::{detect_reu_capacity, ByteReu, ReuError};
use ksa64_sim::phase4::storage::SUPPORTED_REU_KIB;

struct MockReu {
    bytes: Vec<u8>,
    present: bool,
}
impl MockReu {
    fn new(kib: u32) -> Self {
        let mut bytes = vec![0u8; kib as usize * 1_024];
        for (index, byte) in bytes.iter_mut().enumerate().step_by(65_537) {
            *byte = (index as u8).wrapping_mul(37).wrapping_add(11);
        }
        Self {
            bytes,
            present: true,
        }
    }
}
impl ByteReu for MockReu {
    fn probe_presence(&mut self) -> Result<bool, ReuError> {
        Ok(self.present)
    }
    fn read_byte(&mut self, address: u32) -> Result<u8, ReuError> {
        if !self.present {
            return Err(ReuError::Absent);
        }
        Ok(self.bytes[address as usize % self.bytes.len()])
    }
    fn write_byte(&mut self, address: u32, value: u8) -> Result<(), ReuError> {
        if !self.present {
            return Err(ReuError::Absent);
        }
        let length = self.bytes.len();
        self.bytes[address as usize % length] = value;
        Ok(())
    }
}

#[test]
fn preserving_detector_identifies_every_supported_capacity() {
    for kib in SUPPORTED_REU_KIB {
        let mut device = MockReu::new(kib);
        let before = device.bytes.clone();
        assert_eq!(detect_reu_capacity(&mut device).unwrap(), kib);
        assert_eq!(device.bytes, before, "detector changed {kib} KiB contents");
    }
}

#[test]
fn absent_reu_falls_back_without_access() {
    let mut device = MockReu {
        bytes: vec![0; 1],
        present: false,
    };
    assert_eq!(detect_reu_capacity(&mut device).unwrap(), 0);
}
