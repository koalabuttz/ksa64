#![no_std]
#![no_main]

use core::panic::PanicInfo;

const STATUS: *mut u8 = 0xcff0 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const DEVICE: u8 = 8;
const DATA_LFN: u8 = 2;
const DATA_SA: u8 = 2;
const COMMAND_LFN: u8 = 15;
const COMMAND_SA: u8 = 15;
const FILE_NAME: &[u8; 15] = b"KSA4REPORT,S,W\0";
const EMPTY_NAME: &[u8; 1] = b"\0";
const STOCK_VOLUME: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase4/examples/ksa4-stock-report.kxv4"
));

extern "C" {
    fn cbm_k_setlfs(logical_file: u8, device: u8, secondary: u8);
    fn cbm_k_setnam(name: *const u8);
    fn cbm_k_open() -> u8;
    fn cbm_k_close(logical_file: u8);
    fn cbm_k_chkout(logical_file: u8) -> u8;
    fn cbm_k_chkin(logical_file: u8) -> u8;
    fn cbm_k_bsout(byte: u8);
    fn cbm_k_basin() -> u8;
    fn cbm_k_clrch();
    fn cbm_k_readst() -> u8;
}

unsafe fn result(prefix: [u8; 4], code: u8) -> ! {
    let mut index = 0usize;
    while index < prefix.len() {
        core::ptr::write_volatile(STATUS.add(index), prefix[index]);
        index += 1;
    }
    core::ptr::write_volatile(STATUS.add(4), code);
    core::ptr::write_volatile(BORDER, if code == 0 { 5 } else { 2 });
    loop {}
}

unsafe fn drive_status() -> Result<u8, u8> {
    cbm_k_setnam(EMPTY_NAME.as_ptr());
    cbm_k_setlfs(COMMAND_LFN, DEVICE, COMMAND_SA);
    let open = cbm_k_open();
    if open != 0 {
        return Err(open);
    }
    let input = cbm_k_chkin(COMMAND_LFN);
    if input != 0 {
        cbm_k_close(COMMAND_LFN);
        return Err(input);
    }
    let tens = cbm_k_basin();
    let ones = cbm_k_basin();
    let mut count = 0u8;
    while count < 64 {
        let byte = cbm_k_basin();
        if byte == 13 {
            break;
        }
        count = count.wrapping_add(1);
    }
    cbm_k_clrch();
    cbm_k_close(COMMAND_LFN);
    if tens.is_ascii_digit() && ones.is_ascii_digit() {
        Ok((tens - b'0') * 10 + (ones - b'0'))
    } else {
        Err(0xfe)
    }
}

unsafe fn export_stock_report() -> Result<(), u8> {
    cbm_k_setnam(FILE_NAME.as_ptr());
    cbm_k_setlfs(DATA_LFN, DEVICE, DATA_SA);
    let open = cbm_k_open();
    if open != 0 {
        return Err(open);
    }
    let output = cbm_k_chkout(DATA_LFN);
    if output != 0 {
        cbm_k_close(DATA_LFN);
        return Err(output);
    }
    for &byte in STOCK_VOLUME {
        cbm_k_bsout(byte);
        let status = cbm_k_readst();
        if status != 0 {
            cbm_k_clrch();
            cbm_k_close(DATA_LFN);
            return Err(status);
        }
    }
    cbm_k_clrch();
    cbm_k_close(DATA_LFN);
    match drive_status() {
        Ok(0) => Ok(()),
        Ok(code) => Err(code),
        Err(code) => Err(code),
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe { result(*b"X4ER", 0xff) }
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe {
        match export_stock_report() {
            Ok(()) => result(*b"X4OK", 0),
            Err(code) => result(*b"X4ER", code),
        }
    }
}
