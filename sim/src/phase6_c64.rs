//! Memory-mapped SwiftLink/Turbo232 adapter for C64 endpoint builds.
use ksa64_interface::phase6_transport::AciaRegisters;
pub const ACIA_BASE_IO1: u16 = 0xde00;
pub const ACIA_BASE_IO2: u16 = 0xdf00;
pub const TURBO232_COMMAND_POLLING_8N1: u8 = 0x0b;
pub const TURBO232_CONTROL_ENHANCED_8N1: u8 = 0x10;
pub const TURBO232_ENHANCED_57K6: u8 = 0x02;
pub const TURBO232_ENHANCED_115K2: u8 = 0x01;
#[derive(Clone, Copy)]
pub struct C64AciaRegisters {
    base: u16,
}
impl C64AciaRegisters {
    pub const fn new(base: u16) -> Option<Self> {
        if base == ACIA_BASE_IO1 || base == ACIA_BASE_IO2 {
            Some(Self { base })
        } else {
            None
        }
    }
    pub const fn base(&self) -> u16 {
        self.base
    }
    /// Resets and selects polled 8-N-1 SwiftLink 38,400 baud mode.
    ///
    /// # Safety
    /// A SwiftLink-compatible ACIA must be present at the configured base
    /// address, and no other code may access it concurrently.
    pub unsafe fn configure_swiftlink_38400(&mut self) {
        let p = self.base as usize as *mut u8;
        core::ptr::write_volatile(p.add(1), 0);
        core::ptr::write_volatile(p.add(3), 0x1f);
        core::ptr::write_volatile(p.add(2), TURBO232_COMMAND_POLLING_8N1)
    }
    /// Resets and selects polled 8-N-1 Turbo232 enhanced 57,600 baud mode.
    /// Caller must ensure a compatible cartridge or VICE ACIA is present.
    ///
    /// # Safety
    /// A Turbo232-compatible ACIA must be present at the configured base
    /// address, and no other code may access it concurrently.
    pub unsafe fn configure_turbo232_57k6(&mut self) {
        let p = self.base as usize as *mut u8;
        core::ptr::write_volatile(p.add(1), 0);
        core::ptr::write_volatile(p.add(3), TURBO232_CONTROL_ENHANCED_8N1);
        core::ptr::write_volatile(p.add(7), TURBO232_ENHANCED_57K6);
        core::ptr::write_volatile(p.add(2), TURBO232_COMMAND_POLLING_8N1)
    }
}
impl AciaRegisters for C64AciaRegisters {
    fn status(&mut self) -> u8 {
        unsafe { core::ptr::read_volatile((self.base as usize + 1) as *const u8) }
    }
    fn read_data(&mut self) -> u8 {
        unsafe { core::ptr::read_volatile(self.base as usize as *const u8) }
    }
    fn write_data(&mut self, byte: u8) {
        unsafe { core::ptr::write_volatile(self.base as usize as *mut u8, byte) }
    }
}
