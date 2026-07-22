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

pub unsafe fn prepare_cia_timing() {
    write_register(CIA1_INTERRUPT_CONTROL, 0x7f);
    let _ = read_register(CIA1_INTERRUPT_CONTROL);
    write_register(CIA2_INTERRUPT_CONTROL, 0x7f);
    let _ = read_register(CIA2_INTERRUPT_CONTROL);
    write_register(VIC_CONTROL_1, read_register(VIC_CONTROL_1) & 0xef);
    write_register(VIC_SPRITE_ENABLE, 0x00);
}

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

pub unsafe fn stop_cia_timer() -> u32 {
    write_register(CIA1_CONTROL_A, 0x00);
    write_register(CIA1_CONTROL_B, TIMER_B_COUNTS_TIMER_A);

    let timer_a_low = read_register(CIA1_TIMER_A_LOW) as u32;
    let timer_a_high = read_register(CIA1_TIMER_A_HIGH) as u32;
    let timer_b_low = read_register(CIA1_TIMER_B_LOW) as u32;
    let timer_b_high = read_register(CIA1_TIMER_B_HIGH) as u32;
    let remaining = timer_a_low | (timer_a_high << 8) | (timer_b_low << 16) | (timer_b_high << 24);
    u32::MAX.wrapping_sub(remaining)
}

pub unsafe fn measure_cia_boundary_overhead() -> u32 {
    start_cia_timer();
    stop_cia_timer()
}
