//! Target-visible PAL C64 cycle timing through cascaded CIA1 timers.

const CIA1_TIMER_A_LOW: *mut u8 = 0xdc04 as *mut u8;
const CIA1_TIMER_A_HIGH: *mut u8 = 0xdc05 as *mut u8;
const CIA1_TIMER_B_LOW: *mut u8 = 0xdc06 as *mut u8;
const CIA1_TIMER_B_HIGH: *mut u8 = 0xdc07 as *mut u8;
const CIA1_INTERRUPT_CONTROL: *mut u8 = 0xdc0d as *mut u8;
const CIA1_CONTROL_A: *mut u8 = 0xdc0e as *mut u8;
const CIA1_CONTROL_B: *mut u8 = 0xdc0f as *mut u8;
const CIA2_INTERRUPT_CONTROL: *mut u8 = 0xdd0d as *mut u8;
const VIC_CONTROL_1: *mut u8 = 0xd011 as *mut u8;
const VIC_RASTER: *mut u8 = 0xd012 as *mut u8;
const VIC_SPRITE_ENABLE: *mut u8 = 0xd015 as *mut u8;

const CONTROL_FORCE_LOAD: u8 = 0x10;
const CONTROL_START: u8 = 0x01;
const TIMER_B_COUNTS_TIMER_A: u8 = 0x40;

unsafe fn write_register(address: *mut u8, value: u8) {
    core::ptr::write_volatile(address, value);
}

unsafe fn read_register(address: *mut u8) -> u8 {
    core::ptr::read_volatile(address)
}

unsafe fn wait_for_frame_start() {
    while read_register(VIC_RASTER) == 0 && read_register(VIC_CONTROL_1) & 0x80 == 0 {}
    while read_register(VIC_RASTER) != 0 || read_register(VIC_CONTROL_1) & 0x80 != 0 {}
}

/// Disables display DMA, sprites, and CIA interrupts before a measurement.
///
/// # Safety
/// The caller must run on a C64-compatible target with the standard VIC-II and
/// CIA register map, and must accept that display DMA and interrupts are changed.
pub unsafe fn prepare_cia_timing() {
    write_register(CIA1_INTERRUPT_CONTROL, 0x7f);
    let _ = read_register(CIA1_INTERRUPT_CONTROL);
    write_register(CIA2_INTERRUPT_CONTROL, 0x7f);
    let _ = read_register(CIA2_INTERRUPT_CONTROL);
    write_register(VIC_CONTROL_1, read_register(VIC_CONTROL_1) & 0xef);
    write_register(VIC_SPRITE_ENABLE, 0x00);
}

/// Starts the cascaded 32-bit CIA1 clock at a PAL frame boundary.
///
/// # Safety
/// `prepare_cia_timing` must have been called on a C64-compatible target. No
/// other code may concurrently reprogram CIA1 or the VIC-II raster state.
pub unsafe fn start_cia_timer() {
    wait_for_frame_start();
    write_register(CIA1_CONTROL_A, 0x00);
    write_register(CIA1_CONTROL_B, TIMER_B_COUNTS_TIMER_A);
    write_register(CIA1_TIMER_A_LOW, 0xff);
    write_register(CIA1_TIMER_A_HIGH, 0xff);
    write_register(CIA1_TIMER_B_LOW, 0xff);
    write_register(CIA1_TIMER_B_HIGH, 0xff);
    write_register(CIA1_CONTROL_A, CONTROL_FORCE_LOAD);
    write_register(CIA1_CONTROL_B, TIMER_B_COUNTS_TIMER_A | CONTROL_FORCE_LOAD);
    write_register(CIA1_CONTROL_B, TIMER_B_COUNTS_TIMER_A | CONTROL_START);
    write_register(CIA1_CONTROL_A, CONTROL_START);
}

/// Stops the timer and returns elapsed processor clocks.
///
/// # Safety
/// The cascaded timer must have been started by `start_cia_timer`, and no
/// interrupt or concurrent code may modify CIA1 timer registers.
pub unsafe fn stop_cia_timer() -> u32 {
    write_register(CIA1_CONTROL_A, 0x00);
    write_register(CIA1_CONTROL_B, TIMER_B_COUNTS_TIMER_A);
    let remaining = read_register(CIA1_TIMER_A_LOW) as u32
        | ((read_register(CIA1_TIMER_A_HIGH) as u32) << 8)
        | ((read_register(CIA1_TIMER_B_LOW) as u32) << 16)
        | ((read_register(CIA1_TIMER_B_HIGH) as u32) << 24);
    u32::MAX.wrapping_sub(remaining)
}

/// Measures the timer start/stop boundary cost for subtraction from a run.
///
/// # Safety
/// The caller must satisfy the requirements of `prepare_cia_timing` and must
/// allow this function to start and stop CIA1's timers.
pub unsafe fn measure_cia_boundary_overhead() -> u32 {
    start_cia_timer();
    stop_cia_timer()
}
