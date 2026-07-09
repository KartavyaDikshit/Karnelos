#![no_std]
#![no_main]

use core::panic::PanicInfo;

mod generated;

const COM1: u16 = 0x3F8;
const DEBUG_PORT: u16 = 0xE9;

fn outb(port: u16, val: u8) {
    unsafe { core::arch::asm!("out dx, al", in("dx") port, in("al") val); }
}

fn inb(port: u16) -> u8 {
    let val: u8;
    unsafe { core::arch::asm!("in al, dx", out("al") val, in("dx") port); }
    val
}

fn debug_write(s: &[u8]) {
    for &b in s {
        outb(DEBUG_PORT, b);
    }
}

fn serial_init() {
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x80);
    outb(COM1 + 0, 0x01);
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x03);
    outb(COM1 + 2, 0xC7);
    outb(COM1 + 4, 0x0B);
}

fn serial_putc(c: u8) {
    for _ in 0..10000 {
        if inb(COM1 + 5) & 0x20 != 0 {
            outb(COM1, c);
            return;
        }
    }
}

fn serial_write(s: &[u8]) {
    for &b in s {
        serial_putc(b);
    }
}

fn serial_getc() -> Option<u8> {
    if inb(COM1 + 5) & 0x01 != 0 {
        Some(inb(COM1))
    } else {
        None
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_write(b"\r\nKERNEL PANIC\r\n");
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

fn print_prompt() {
    serial_write(b"\r\n> ");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    serial_init();
    debug_write(b"Karnelos booting...\n");

    serial_write(b"\r\n");
    serial_write(b"  _  __                         _           \r\n");
    serial_write(b" | |/ /__ _ _ _ ___ ___ ___ ___| |___ ___   \r\n");
    serial_write(b" | ' </ _` | '_/ -_|_-</ -_|_-< | / _/ _ \\  \r\n");
    serial_write(b" |_|\\_\\__,_|_| \\___/__/\\___/__/_|_\\__\\___/  \r\n");
    serial_write(b"\r\n");
    serial_write(b" Karnelos OS v0.1\r\n");
    serial_write(b" x86-64 | Local AI | Generated Everything\r\n");
    serial_write(b"\r\n");

    print_prompt();

    loop {
        if let Some(c) = serial_getc() {
            serial_putc(c);
        } else {
            unsafe { core::arch::asm!("hlt"); }
        }
    }
}
