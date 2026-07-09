pub const COM1: u16 = 0x3F8;
pub const COM2: u16 = 0x2F8;
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

pub fn debug_putc(c: u8) { outb(DEBUG_PORT, c); }

pub fn debug_write(s: &[u8]) {
    for &b in s { debug_putc(b); }
}

pub fn serial_init() {
    serial_init_port(COM1);
}

pub fn serial_init_port(port: u16) {
    outb(port + 1, 0x00); outb(port + 3, 0x80);
    outb(port + 0, 0x01); outb(port + 1, 0x00);
    outb(port + 3, 0x03); outb(port + 2, 0xC7);
    outb(port + 4, 0x0B);
}

pub fn serial_putc(c: u8) { serial_putc_port(COM1, c); }

pub fn serial_putc_port(port: u16, c: u8) {
    for _ in 0..10000 {
        if inb(port + 5) & 0x20 != 0 { outb(port, c); return; }
    }
}

pub fn serial_write(s: &[u8]) { serial_write_port(COM1, s); }

pub fn serial_write_port(port: u16, s: &[u8]) {
    for &b in s { serial_putc_port(port, b); }
}

pub fn serial_read_port(port: u16) -> Option<u8> {
    if inb(port + 5) & 1 != 0 { Some(inb(port)) } else { None }
}

pub fn vga_putc(c: u8, row: usize, col: usize, fg: u8, bg: u8) {
    let pos = (row * 80 + col) * 2;
    unsafe {
        VGA_ADDR.add(pos).write(c);
        VGA_ADDR.add(pos + 1).write(bg << 4 | fg);
    }
}

pub fn vga_write(s: &[u8], row: usize, fg: u8, bg: u8) {
    for (i, &b) in s.iter().enumerate().take(80) {
        vga_putc(b, row, i, fg, bg);
    }
}

pub fn vga_clear(fg: u8, bg: u8) {
    for row in 0..25 {
        for col in 0..80 { vga_putc(b' ', row, col, fg, bg); }
    }
}

#[allow(dead_code)]
pub struct VgaWriter {
    row: usize,
    col: usize,
    fg: u8,
    bg: u8,
    scroll_top: usize,
}

#[allow(dead_code)]
impl VgaWriter {
    pub fn new(scroll_top: usize, fg: u8, bg: u8) -> Self {
        VgaWriter { row: scroll_top, col: 0, fg, bg, scroll_top }
    }

    pub fn write_byte(&mut self, c: u8) {
        match c {
            b'\r' => self.col = 0,
            b'\n' => self.newline(),
            0x08 => {
                if self.col > 0 { self.col -= 1; self.put(b' '); }
            }
            b if b >= 0x20 => {
                self.put(c);
                self.col += 1;
                if self.col >= 80 { self.newline(); }
            }
            _ => {}
        }
    }

    pub fn write_str(&mut self, s: &[u8]) {
        for &b in s { self.write_byte(b); }
    }

    fn put(&self, c: u8) {
        vga_putc(c, self.row, self.col, self.fg, self.bg);
    }

    fn newline(&mut self) {
        self.col = 0;
        if self.row < 24 {
            self.row += 1;
        } else {
            self.scroll();
        }
    }

    fn scroll(&mut self) {
        for r in self.scroll_top..24 {
            for c in 0..80 {
                let src = ((r + 1) * 80 + c) * 2;
                let dst = (r * 80 + c) * 2;
                unsafe {
                    VGA_ADDR.add(dst).write(VGA_ADDR.add(src).read());
                    VGA_ADDR.add(dst + 1).write(VGA_ADDR.add(src + 1).read());
                }
            }
        }
        for c in 0..80 { vga_putc(b' ', 24, c, self.fg, self.bg); }
    }
}
