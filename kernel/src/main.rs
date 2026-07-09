#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
extern crate alloc;

use core::panic::PanicInfo;
use bootloader::bootinfo::BootInfo;
use alloc::vec::Vec;
use alloc::string::String;

mod generated;
mod io;
mod keyboard;
mod interrupts;
mod memory;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    io::serial_write(b"\r\nKERNEL PANIC\r\n");
    if let Some(loc) = info.location() {
        io::serial_write(loc.file().as_bytes());
        io::serial_write(b":");
        let line = loc.line();
        let mut b = [0u8; 10];
        let mut i = 10;
        let mut n = line;
        while n > 0 { i -= 1; b[i] = b'0' + (n % 10) as u8; n /= 10; }
        if i == 10 { io::serial_putc(b'0'); } else { io::serial_write(&b[i..]); }
    }
    io::serial_write(b"\r\n");
    io::debug_write(b"KERNEL PANIC\n");
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

const PROMPT_ROW: usize = 9;

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

fn heap_test() {
    io::serial_write(b"Testing heap allocation...\r\n");
    let mut v: Vec<u8> = Vec::new();
    for i in 0..100 {
        v.push(i as u8);
    }
    io::serial_write(b"Vec allocated: ");
    memory::dec(v.len());
    io::serial_write(b" elements\r\n");
    for (i, &val) in v.iter().enumerate() {
        if val != i as u8 {
            io::serial_write(b"MISMATCH at ");
            memory::dec(i);
            io::serial_write(b"\r\n");
            return;
        }
    }
    io::serial_write(b"Vec contents verified OK\r\n");

    let mut s = String::from("Hello from heap via String!");
    io::serial_write(s.as_bytes());
    io::serial_write(b"\r\n");
    s.push_str(" (appended)");
    io::serial_write(s.as_bytes());
    io::serial_write(b"\r\n");
    io::serial_write(b"Heap test PASSED\r\n");
}

#[no_mangle]
pub extern "C" fn _start(boot_info: &'static BootInfo) -> ! {
    io::debug_write(b"Karnelos booting...\n");

    io::serial_init();
    io::vga_clear(0x0F, 0x00);
    io::vga_write(b"Karnelos OS v0.1 - Booting...", 0, 0x0F, 0x00);

    print_banner();
    serial_print_banner();

    io::debug_write(b"Starting interrupts\n");
    interrupts::init();

    io::debug_write(b"Initializing memory\n");
    memory::init(boot_info);
    io::debug_write(b"Initializing heap\n");
    memory::init_heap();
    io::debug_write(b"Testing heap\n");
    heap_test();

    io::debug_write(b"Initializing keyboard\n");
    keyboard::init();

    io::vga_write_at(b"> ", PROMPT_ROW, 0, 0x0F, 0x00);
    io::serial_write(b"> ");

    io::debug_write(b"Ready\n");

    let mut cursor: usize = 2;

    loop {
        x86_64::instructions::interrupts::enable_and_hlt();
        while let Some(c) = keyboard::read_char() {
            match c {
                b'\r' | b'\n' => {
                    io::serial_write(b"\r\n");
                    io::serial_write(b"> ");
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
