#![no_std]
#![no_main]

use core::panic::PanicInfo;

mod generated;

const COM1: u16 = 0x3F8;
const DEBUG_PORT: u16 = 0xE9;
const VGA_ADDR: *mut u8 = 0xB8000 as *mut u8;

fn outb(port: u16, val: u8) {
    unsafe { core::arch::asm!("out dx, al", in("dx") port, in("al") val); }
}

fn inb(port: u16) -> u8 {
    let val: u8;
    unsafe { core::arch::asm!("in al, dx", out("al") val, in("dx") port); }
    val
}

fn debug_putc(c: u8) {
    outb(DEBUG_PORT, c);
}

fn debug_write(s: &[u8]) {
    for &b in s {
        debug_putc(b);
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

fn vga_putc(c: u8, row: usize, col: usize, fg: u8, bg: u8) {
    let pos = (row * 80 + col) * 2;
    unsafe {
        VGA_ADDR.add(pos).write(c);
        VGA_ADDR.add(pos + 1).write(bg << 4 | fg);
    }
}

fn vga_write(s: &[u8], row: usize, fg: u8, bg: u8) {
    let len = if s.len() > 80 { 80 } else { s.len() };
    for (i, &b) in s[..len].iter().enumerate() {
        vga_putc(b, row, i, fg, bg);
    }
}

fn vga_write_at(s: &[u8], row: usize, col: usize, fg: u8, bg: u8) {
    for (i, &b) in s.iter().enumerate() {
        if col + i < 80 {
            vga_putc(b, row, col + i, fg, bg);
        }
    }
}

const PROMPT_ROW: usize = 9;

fn vga_clear(fg: u8, bg: u8) {
    for row in 0..25 {
        for col in 0..80 {
            vga_putc(b' ', row, col, fg, bg);
        }
    }
}

fn print_banner() {
    vga_write(b"  _  __                         _           ", 1, 0x0F, 0x00);
    vga_write(b" | |/ /__ _ _ _ ___ ___ ___ ___| |___ ___   ", 2, 0x0F, 0x00);
    vga_write(b" | ' </ _` | '_/ -_|_-</ -_|_-< | / _/ _ \\  ", 3, 0x0F, 0x00);
    vga_write(b" |_|\\_\\__,_|_| \\___/__/\\___/__/_|_\\__\\___/  ", 4, 0x0F, 0x00);
    vga_write(b"                                              ", 5, 0x0F, 0x00);
    vga_write(b" Karnelos OS v0.1                            ", 6, 0x0A, 0x00);
    vga_write(b" x86-64 | Local AI | Generated Everything     ", 7, 0x0A, 0x00);
    vga_write(b">                                              ", PROMPT_ROW, 0x0F, 0x00);
}

fn serial_print_banner() {
    serial_write(b"\r\n");
    serial_write(b"  _  __                         _           \r\n");
    serial_write(b" | |/ /__ _ _ _ ___ ___ ___ ___| |___ ___   \r\n");
    serial_write(b" | ' </ _` | '_/ -_|_-</ -_|_-< | / _/ _ \\  \r\n");
    serial_write(b" |_|\\_\\__,_|_| \\___/__/\\___/__/_|_\\__\\___/  \r\n");
    serial_write(b"\r\n");
    serial_write(b" Karnelos OS v0.1\r\n");
    serial_write(b" x86-64 | Local AI | Generated Everything\r\n");
    serial_write(b"\r\n");
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_write(b"\r\nKERNEL PANIC\r\n");
    debug_write(b"KERNEL PANIC\n");
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    debug_write(b"Karnelos booting...\n");

    serial_init();
    vga_clear(0x0F, 0x00);
    vga_write(b"Karnelos OS v0.1 - Booting...", 0, 0x0F, 0x00);

    print_banner();
    serial_print_banner();

    debug_write(b"Ready\n");

    vga_write_at(b"> ", PROMPT_ROW, 0, 0x0F, 0x00);
    serial_write(b"> ");

    loop {
        if let Some(c) = serial_getc() {
            serial_putc(c);
            debug_putc(c);
            vga_write_at(b"> ", PROMPT_ROW, 0, 0x0F, 0x00);
            serial_write(b"\r\n> ");
        } else {
            unsafe { core::arch::asm!("hlt"); }
        }
    }
}
