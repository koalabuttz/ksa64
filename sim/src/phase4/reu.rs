//! Preserving REU detection and explicit C64 DMA transport.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReuError {
    Absent,
    Transfer,
    Address,
}

pub trait ByteReu {
    fn probe_presence(&mut self) -> Result<bool, ReuError>;
    fn read_byte(&mut self, address: u32) -> Result<u8, ReuError>;
    fn write_byte(&mut self, address: u32, value: u8) -> Result<(), ReuError>;
}

pub fn detect_reu_capacity<D: ByteReu>(device: &mut D) -> Result<u32, ReuError> {
    if !device.probe_presence()? {
        return Ok(0);
    }
    // Preserve one byte from every logical 64 KiB bank. Descending signatures leave
    // the lowest bank number in each physically aliased bank, so the first mismatch
    // identifies 2..256 physical banks without assuming a particular mirror wiring.
    let mut originals = [0u8; 256];
    let mut bank = 0usize;
    while bank < 256 {
        originals[bank] = device.read_byte((bank as u32) << 16)?;
        bank += 1;
    }
    bank = 256;
    while bank != 0 {
        bank -= 1;
        if let Err(error) = device.write_byte((bank as u32) << 16, bank as u8) {
            restore_banks(device, &originals);
            return Err(error);
        }
    }
    let mut banks = 0usize;
    while banks < 256 {
        match device.read_byte((banks as u32) << 16) {
            Ok(value) if value == banks as u8 => banks += 1,
            Ok(_) => break,
            Err(error) => {
                restore_banks(device, &originals);
                return Err(error);
            }
        }
    }
    restore_banks(device, &originals);
    let capacity_kib = if banks == 256 {
        16_384
    } else {
        (banks as u32) * 64
    };
    if matches!(
        capacity_kib,
        128 | 256 | 512 | 1_024 | 2_048 | 4_096 | 8_192 | 16_384
    ) {
        Ok(capacity_kib)
    } else {
        Err(ReuError::Address)
    }
}

fn restore_banks<D: ByteReu>(device: &mut D, originals: &[u8; 256]) {
    let mut bank = 0usize;
    while bank < 256 {
        let _ = device.write_byte((bank as u32) << 16, originals[bank]);
        bank += 1;
    }
}

#[cfg(feature = "c64")]
mod c64 {
    use super::{ByteReu, ReuError};
    use crate::phase4::archive::{ArchiveStorage, ArchiveStorageError};
    use crate::phase5_archive::{
        ArchiveStorage as Phase5ArchiveStorage, ArchiveStorageError as Phase5ArchiveStorageError,
    };

    const STATUS: *mut u8 = 0xdf00 as *mut u8;
    const COMMAND: *mut u8 = 0xdf01 as *mut u8;
    const C64_ADDRESS_LO: *mut u8 = 0xdf02 as *mut u8;
    const C64_ADDRESS_HI: *mut u8 = 0xdf03 as *mut u8;
    const REU_ADDRESS_LO: *mut u8 = 0xdf04 as *mut u8;
    const REU_ADDRESS_MID: *mut u8 = 0xdf05 as *mut u8;
    const REU_ADDRESS_HI: *mut u8 = 0xdf06 as *mut u8;
    const LENGTH_LO: *mut u8 = 0xdf07 as *mut u8;
    const LENGTH_HI: *mut u8 = 0xdf08 as *mut u8;
    const IRQ_MASK: *mut u8 = 0xdf09 as *mut u8;
    const ADDRESS_CONTROL: *mut u8 = 0xdf0a as *mut u8;
    const STASH: u8 = 0x90;
    const FETCH: u8 = 0x91;
    static mut BYTE: u8 = 0;

    unsafe fn dma(command: u8, c64_address: u16, reu_address: u32, length: u16) {
        core::ptr::write_volatile(C64_ADDRESS_LO, c64_address as u8);
        core::ptr::write_volatile(C64_ADDRESS_HI, (c64_address >> 8) as u8);
        core::ptr::write_volatile(REU_ADDRESS_LO, reu_address as u8);
        core::ptr::write_volatile(REU_ADDRESS_MID, (reu_address >> 8) as u8);
        core::ptr::write_volatile(REU_ADDRESS_HI, (reu_address >> 16) as u8);
        core::ptr::write_volatile(LENGTH_LO, length as u8);
        core::ptr::write_volatile(LENGTH_HI, (length >> 8) as u8);
        core::ptr::write_volatile(IRQ_MASK, 0);
        core::ptr::write_volatile(ADDRESS_CONTROL, 0);
        core::ptr::write_volatile(COMMAND, command);
        let _ = core::ptr::read_volatile(STATUS);
    }

    pub struct C64ReuStorage {
        capacity: u32,
    }
    impl C64ReuStorage {
        pub const fn new(capacity_kib: u32) -> Self {
            Self {
                capacity: capacity_kib * 1_024,
            }
        }

        fn transfer(
            &mut self,
            command: u8,
            reu_offset: u32,
            bytes: *mut u8,
            length: usize,
        ) -> Result<(), ReuError> {
            if reu_offset
                .checked_add(length as u32)
                .filter(|end| *end <= self.capacity)
                .is_none()
                || length > u16::MAX as usize
            {
                return Err(ReuError::Address);
            }
            let address = bytes as usize;
            if address > u16::MAX as usize || address as u32 + length as u32 > 65_536 {
                return Err(ReuError::Address);
            }
            if length != 0 {
                unsafe { dma(command, address as u16, reu_offset, length as u16) };
            }
            Ok(())
        }

        pub fn write_slice(&mut self, offset: u32, bytes: &[u8]) -> Result<(), ReuError> {
            self.transfer(STASH, offset, bytes.as_ptr() as *mut u8, bytes.len())
        }

        pub fn read_slice(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), ReuError> {
            self.transfer(FETCH, offset, bytes.as_mut_ptr(), bytes.len())
        }
    }

    impl ByteReu for C64ReuStorage {
        fn probe_presence(&mut self) -> Result<bool, ReuError> {
            unsafe {
                let pointer = core::ptr::addr_of_mut!(BYTE);
                core::ptr::write_volatile(pointer, 0x39);
                dma(FETCH, pointer as u16, 0, 1);
                let original = core::ptr::read_volatile(pointer);
                core::ptr::write_volatile(pointer, 0x5a);
                dma(STASH, pointer as u16, 0, 1);
                core::ptr::write_volatile(pointer, 0xa5);
                dma(FETCH, pointer as u16, 0, 1);
                let present = core::ptr::read_volatile(pointer) == 0x5a;
                if present {
                    core::ptr::write_volatile(pointer, original);
                    dma(STASH, pointer as u16, 0, 1);
                }
                Ok(present)
            }
        }

        fn read_byte(&mut self, address: u32) -> Result<u8, ReuError> {
            if address >= self.capacity {
                // Detection intentionally probes aliased addresses, so mask by configured maximum.
                // The physical REU performs the actual capacity wrap.
            }
            unsafe {
                let pointer = core::ptr::addr_of_mut!(BYTE);
                core::ptr::write_volatile(pointer, 0x6d);
                dma(FETCH, pointer as u16, address & 0x00ff_ffff, 1);
                Ok(core::ptr::read_volatile(pointer))
            }
        }

        fn write_byte(&mut self, address: u32, value: u8) -> Result<(), ReuError> {
            unsafe {
                let pointer = core::ptr::addr_of_mut!(BYTE);
                core::ptr::write_volatile(pointer, value);
                dma(STASH, pointer as u16, address & 0x00ff_ffff, 1);
            }
            Ok(())
        }
    }

    impl ArchiveStorage for C64ReuStorage {
        fn capacity(&self) -> u32 {
            self.capacity
        }
        fn read(&mut self, offset: u32, out: &mut [u8]) -> Result<(), ArchiveStorageError> {
            self.read_slice(offset, out)
                .map_err(|_| ArchiveStorageError)
        }
        fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), ArchiveStorageError> {
            self.write_slice(offset, bytes)
                .map_err(|_| ArchiveStorageError)
        }
    }

    impl Phase5ArchiveStorage for C64ReuStorage {
        fn capacity(&self) -> u32 {
            self.capacity
        }
        fn read(&mut self, offset: u32, out: &mut [u8]) -> Result<(), Phase5ArchiveStorageError> {
            self.read_slice(offset, out)
                .map_err(|_| Phase5ArchiveStorageError)
        }
        fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Phase5ArchiveStorageError> {
            self.write_slice(offset, bytes)
                .map_err(|_| Phase5ArchiveStorageError)
        }
    }
    pub fn detect_c64_reu_capacity() -> Result<u32, ReuError> {
        let mut device = C64ReuStorage::new(16_384);
        super::detect_reu_capacity(&mut device)
    }
}

#[cfg(feature = "c64")]
pub use c64::{detect_c64_reu_capacity, C64ReuStorage};
