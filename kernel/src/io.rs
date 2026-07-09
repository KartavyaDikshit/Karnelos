pub const COM1: u16 = 0x3F8;
pub const DEBUG_PORT: u16 = 0xE9;
pub const VGA_ADDR: *mut u8 = 0xB8000 as *mut u8;

pub fn outb(port: u16, val: u8) {
    unsafe { core::arch::asm!("out dx, al", in("dx") port, in("al") val); }
}

pub fn inb(port: u16) -> u8 {
    let val: u8;
    unsafe { core::arch::asm!("in al, dx", out("al") val, in("dx") port); }
    val
}

pub fn debug_putc(c: u8) {
    outb(DEBUG_PORT, c);
}

pub fn debug_write(s: &[u8]) {
    for &b in s {
        debug_putc(b);
    }
}

pub fn serial_init() {
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x80);
    outb(COM1 + 0, 0x01);
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x03);
    outb(COM1 + 2, 0xC7);
    outb(COM1 + 4, 0x0B);
}

pub fn serial_putc(c: u8) {
    for _ in 0..10000 {
        if inb(COM1 + 5) & 0x20 != 0 {
            outb(COM1, c);
            return;
        }
    }
}

pub fn serial_write(s: &[u8]) {
    for &b in s {
        serial_putc(b);
    }
}

pub fn vga_putc(c: u8, row: usize, col: usize, fg: u8, bg: u8) {
    let pos = (row * 80 + col) * 2;
    unsafe {
        VGA_ADDR.add(pos).write(c);
        VGA_ADDR.add(pos + 1).write(bg << 4 | fg);
    }
}

pub fn vga_write(s: &[u8], row: usize, fg: u8, bg: u8) {
    let len = if s.len() > 80 { 80 } else { s.len() };
    for (i, &b) in s[..len].iter().enumerate() {
        vga_putc(b, row, i, fg, bg);
    }
}

pub fn vga_write_at(s: &[u8], row: usize, col: usize, fg: u8, bg: u8) {
    for (i, &b) in s.iter().enumerate() {
        if col + i < 80 {
            vga_putc(b, row, col + i, fg, bg);
        }
    }
}

pub fn vga_clear(fg: u8, bg: u8) {
    for row in 0..25 {
        for col in 0..80 {
            vga_putc(b' ', row, col, fg, bg);
        }
    }
}
