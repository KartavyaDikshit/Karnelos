#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;

mod generated;
mod io;
mod keyboard;
mod interrupts;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    io::serial_write(b"\r\nKERNEL PANIC\r\n");
    io::debug_write(b"KERNEL PANIC\n");
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

const PROMPT_ROW: usize = 9;
const PROMPT_PREFIX: &[u8] = b"> ";

fn print_banner() {
    io::vga_write(b"  _  __                         _           ", 1, 0x0F, 0x00);
    io::vga_write(b" | |/ /__ _ _ _ ___ ___ ___ ___| |___ ___   ", 2, 0x0F, 0x00);
    io::vga_write(b" | ' </ _` | '_/ -_|_-</ -_|_-< | / _/ _ \\  ", 3, 0x0F, 0x00);
    io::vga_write(b" |_|\\_\\__,_|_| \\___/__/\\___/__/_|_\\__\\___/  ", 4, 0x0F, 0x00);
    io::vga_write(b"                                              ", 5, 0x0F, 0x00);
    io::vga_write(b" Karnelos OS v0.1                            ", 6, 0x0A, 0x00);
    io::vga_write(b" x86-64 | Local AI | Generated Everything     ", 7, 0x0A, 0x00);
}

fn serial_print_banner() {
    io::serial_write(b"\r\n");
    io::serial_write(b"  _  __                         _           \r\n");
    io::serial_write(b" | |/ /__ _ _ _ ___ ___ ___ ___| |___ ___   \r\n");
    io::serial_write(b" | ' </ _` | '_/ -_|_-</ -_|_-< | / _/ _ \\  \r\n");
    io::serial_write(b" |_|\\_\\__,_|_| \\___/__/\\___/__/_|_\\__\\___/  \r\n");
    io::serial_write(b"\r\n");
    io::serial_write(b" Karnelos OS v0.1\r\n");
    io::serial_write(b" x86-64 | Local AI | Generated Everything\r\n");
    io::serial_write(b"\r\n");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    io::debug_write(b"Karnelos booting...\n");

    io::serial_init();
    keyboard::init();
    io::vga_clear(0x0F, 0x00);
    io::vga_write(b"Karnelos OS v0.1 - Booting...", 0, 0x0F, 0x00);

    print_banner();
    serial_print_banner();

    io::debug_write(b"Starting interrupts\n");
    interrupts::init();

    io::vga_write_at(PROMPT_PREFIX, PROMPT_ROW, 0, 0x0F, 0x00);
    io::serial_write(PROMPT_PREFIX);

    io::debug_write(b"Ready\n");

    let mut cursor: usize = 2;

    loop {
        x86_64::instructions::interrupts::enable_and_hlt();
        while let Some(c) = keyboard::read_char() {
            match c {
                b'\r' | b'\n' => {
                    io::serial_write(b"\r\n");
                    io::serial_write(PROMPT_PREFIX);
                    cursor = 2;
                    io::vga_write_at(b"  ", PROMPT_ROW, 0, 0x0F, 0x00);
                }
                0x08 | 0x7F => {
                    if cursor > 2 {
                        cursor -= 1;
                        io::serial_write(b"\x08 \x08");
                        io::vga_putc(b' ', PROMPT_ROW, cursor, 0x0F, 0x00);
                    }
                }
                _b => {
                    if cursor < 79 {
                        io::serial_putc(c);
                        io::vga_putc(c, PROMPT_ROW, cursor, 0x0F, 0x00);
                        cursor += 1;
                    }
                }
            }
        }
    }
}
