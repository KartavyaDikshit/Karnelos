#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
extern crate alloc;

use core::panic::PanicInfo;
use bootloader_api::BootInfo;

mod io;
mod keyboard;
mod interrupts;
mod memory;
mod shell;
mod loader;
mod process;
mod ata;
mod filesystem;

pub static mut SHELL: Option<shell::Shell> = None;

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
    loop { unsafe { core::arch::asm!("hlt"); } }
}

fn banner_vga() {
    io::vga_write(b"  _  __                         _           ", 1, 0x0F, 0x00);
    io::vga_write(b" | |/ /__ _ _ _ ___ ___ ___ ___| |___ ___   ", 2, 0x0F, 0x00);
    io::vga_write(b" | ' </ _` | '_/ -_|_-</ -_|_-< | / _/ _ \\  ", 3, 0x0F, 0x00);
    io::vga_write(b" |_|\\_\\__,_|_| \\___/__/\\___/__/_|_\\__\\___/  ", 4, 0x0F, 0x00);
    io::vga_write(b"                                              ", 5, 0x0F, 0x00);
    io::vga_write(b" Karnelos OS v0.1                            ", 6, 0x0A, 0x00);
    io::vga_write(b" x86-64 | Local AI | Generated Everything     ", 7, 0x0A, 0x00);
}

fn banner_serial() {
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

pub static CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    config.kernel_stack_size = 4 * 1024 * 1024;
    config.mappings.physical_memory = Some(bootloader_api::config::Mapping::Dynamic);
    config.mappings.page_table_recursive = Some(bootloader_api::config::Mapping::Dynamic);
    config
};

bootloader_api::entry_point!(kernel_main, config = &CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    io::debug_write(b"Karnelos booting...\n");

    io::serial_init();
    io::serial_init_port(io::COM2);

    if let Some(fb) = boot_info.framebuffer.as_ref() {
        let info = fb.info();
        io::init_framebuffer(
            fb.buffer().as_ptr() as u64,
            info.width,
            info.height,
            info.bytes_per_pixel,
            info.stride,
            info.pixel_format,
        );
        io::debug_write(b"Framebuffer initialized\n");
    } else {
        io::debug_write(b"No framebuffer available\n");
    }

    io::vga_clear(0x0F, 0x00);
    banner_vga();
    banner_serial();

    io::debug_write(b"Initializing interrupts\n");
    interrupts::init();

    io::debug_write(b"Initializing memory\n");
    memory::init(boot_info);
    io::debug_write(b"Initializing heap\n");
    memory::init_heap();
    io::debug_write(b"Initializing storage (ATA/IDE)\n");
    ata::init();
    io::debug_write(b"Initializing userspace\n");
    process::init();

    io::debug_write(b"Initializing keyboard\n");
    keyboard::init();

    io::debug_write(b"Starting shell\n");

    let s = shell::Shell::new(b"karnelos> ");
    unsafe { SHELL = Some(s); }

    io::debug_write(b"Ready\n");

    shell_main_loop();
}

#[no_mangle]
pub extern "C" fn shell_main_loop() -> ! {
    {
        unsafe {
            if let Some(ref mut shell) = SHELL {
                shell.print_prompt();
            }
        }
    }
    loop {
        x86_64::instructions::interrupts::enable_and_hlt();
        unsafe {
            if let Some(ref mut shell) = SHELL {
                while let Some(c) = keyboard::read_char() {
                    shell.handle_char(c);
                }
                if io::inb(io::COM1 + 5) & 1 != 0 {
                    let c = io::inb(io::COM1);
                    if c == b'\r' || c == b'\n' || c == 0x08 || c == 0x7F || (c >= 0x20 && c < 0x7F) {
                        shell.handle_char(c);
                    }
                }
                if shell.awaiting_response() {
                    if io::inb(io::COM2 + 5) & 1 != 0 {
                        let c = io::inb(io::COM2);
                        shell.handle_daemon_byte(c);
                    }
                }
            }
        }
    }
}
