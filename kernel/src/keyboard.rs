use core::sync::atomic::{AtomicUsize, Ordering};

fn outb(port: u16, val: u8) {
    unsafe { core::arch::asm!("out dx, al", in("dx") port, in("al") val); }
}

fn inb(port: u16) -> u8 {
    let val: u8;
    unsafe { core::arch::asm!("in al, dx", out("al") val, in("dx") port); }
    val
}

const BUF_SIZE: usize = 256;

pub struct KeyBuffer {
    buf: [u8; BUF_SIZE],
    head: AtomicUsize,
    tail: AtomicUsize,
}

impl KeyBuffer {
    const fn new() -> Self {
        KeyBuffer {
            buf: [0; BUF_SIZE],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    fn push(&mut self, c: u8) {
        let h = self.head.load(Ordering::Relaxed);
        let next_h = (h + 1) % BUF_SIZE;
        if next_h != self.tail.load(Ordering::Relaxed) {
            self.buf[h] = c;
            self.head.store(next_h, Ordering::Release);
        }
    }

    pub fn pop(&mut self) -> Option<u8> {
        let t = self.tail.load(Ordering::Relaxed);
        if t == self.head.load(Ordering::Acquire) {
            return None;
        }
        let c = self.buf[t];
        self.tail.store((t + 1) % BUF_SIZE, Ordering::Release);
        Some(c)
    }
}

pub static KEY_BUFFER: spin::Mutex<KeyBuffer> = spin::Mutex::new(KeyBuffer::new());

#[repr(u8)]
#[derive(Clone, Copy, PartialEq)]
enum Modifier {
    Shift = 1 << 0,
    Ctrl = 1 << 1,
    Alt = 1 << 2,
    Caps = 1 << 3,
}

struct KeyboardState {
    modifiers: u8,
    extended: bool,
}

static STATE: spin::Mutex<KeyboardState> = spin::Mutex::new(KeyboardState {
    modifiers: 0,
    extended: false,
});

const NORMAL: [u8; 128] = [
    0,     0,     b'1',  b'2',  b'3',  b'4',  b'5',  b'6',  // 00-07
    b'7',  b'8',  b'9',  b'0',  b'-',  b'=',  0x08,  b'\t', // 08-0F
    b'q',  b'w',  b'e',  b'r',  b't',  b'y',  b'u',  b'i',  // 10-17
    b'o',  b'p',  b'[',  b']',  b'\n', 0,     b'a',  b's',  // 18-1F
    b'd',  b'f',  b'g',  b'h',  b'j',  b'k',  b'l',  b';',  // 20-27
    b'\'', b'`',  0,     b'\\', b'z',  b'x',  b'c',  b'v',  // 28-2F
    b'b',  b'n',  b'm',  b',',  b'.',  b'/',  0,     b'*',  // 30-37
    0,     b' ',  0,     0,     0,     0,     0,     0,      // 38-3F
    0,     0,     0,     0,     0,     0,     0,     0,      // 40-47
    b'7',  b'8',  b'9',  b'-',  b'4',  b'5',  b'6',  b'+',  // 48-4F
    b'1',  b'2',  b'3',  b'0',  b'.',  0,     0,     0,      // 50-57
    0,     0,     0,     0,     0,     0,     0,     0,      // 58-5F
    0,     0,     0,     0,     0,     0,     0,     0,      // 60-67
    0,     0,     0,     0,     0,     0,     0,     0,      // 68-6F
    0,     0,     0,     0,     0,     0,     0,     0,      // 70-77
    0,     0,     0,     0,     0,     0,     0,     0,      // 78-7F
];

const SHIFTED: [u8; 128] = [
    0,     0,     b'!',  b'@',  b'#',  b'$',  b'%',  b'^',  // 00-07
    b'&',  b'*',  b'(',  b')',  b'_',  b'+',  0x08,  b'\t', // 08-0F
    b'Q',  b'W',  b'E',  b'R',  b'T',  b'Y',  b'U',  b'I',  // 10-17
    b'O',  b'P',  b'{',  b'}',  b'\n', 0,     b'A',  b'S',  // 18-1F
    b'D',  b'F',  b'G',  b'H',  b'J',  b'K',  b'L',  b':',  // 20-27
    b'"',  b'~',  0,     b'|',  b'Z',  b'X',  b'C',  b'V',  // 28-2F
    b'B',  b'N',  b'M',  b'<',  b'>',  b'?',  0,     b'*',  // 30-37
    0,     b' ',  0,     0,     0,     0,     0,     0,      // 38-3F
    0,     0,     0,     0,     0,     0,     0,     0,      // 40-47
    b'7',  b'8',  b'9',  b'-',  b'4',  b'5',  b'6',  b'+',  // 48-4F
    b'1',  b'2',  b'3',  b'0',  b'.',  0,     0,     0,      // 50-57
    0,     0,     0,     0,     0,     0,     0,     0,      // 58-5F
    0,     0,     0,     0,     0,     0,     0,     0,      // 60-67
    0,     0,     0,     0,     0,     0,     0,     0,      // 68-6F
    0,     0,     0,     0,     0,     0,     0,     0,      // 70-77
    0,     0,     0,     0,     0,     0,     0,     0,      // 78-7F
];

fn ps2_reset() {
    let timeout = 10000;
    for _ in 0..timeout {
        if inb(0x64) & 0x02 == 0 {
            break;
        }
    }

    outb(0x64, 0xAE);
    outb(0x64, 0x20);
    for _ in 0..timeout {
        if inb(0x64) & 0x01 != 0 {
            break;
        }
    }
    let config = inb(0x60);
    let config = config | 0x01;
    outb(0x64, 0x60);
    for _ in 0..timeout {
        if inb(0x64) & 0x02 == 0 {
            break;
        }
    }
    outb(0x60, config);
}

pub fn init() {
    ps2_reset();
}

pub fn handle_scancode(scancode: u8) {
    let mut state = STATE.lock();

    if state.extended {
        state.extended = false;
        let break_code = scancode & 0x80 != 0;
        let code = scancode & 0x7F;

        match code {
            0x1D => if !break_code { state.modifiers |= Modifier::Ctrl as u8; }
                    else { state.modifiers &= !(Modifier::Ctrl as u8); },
            0x38 => if !break_code { state.modifiers |= Modifier::Alt as u8; }
                    else { state.modifiers &= !(Modifier::Alt as u8); },
            _ => {}
        }
        return;
    }

    if scancode == 0xE0 {
        state.extended = true;
        return;
    }

    let break_code = scancode & 0x80 != 0;
    let code = scancode & 0x7F;

    if code as usize >= 128 {
        return;
    }

    match code {
        0x2A | 0x36 => {
            if !break_code { state.modifiers |= Modifier::Shift as u8; }
            else { state.modifiers &= !(Modifier::Shift as u8); }
        }
        0x1D => {
            if !break_code { state.modifiers |= Modifier::Ctrl as u8; }
            else { state.modifiers &= !(Modifier::Ctrl as u8); }
        }
        0x38 => {
            if !break_code { state.modifiers |= Modifier::Alt as u8; }
            else { state.modifiers &= !(Modifier::Alt as u8); }
        }
        0x3A => {
            if !break_code { state.modifiers ^= Modifier::Caps as u8; }
        }
        _ => {
            if !break_code && code < 128 {
                let shifted = (state.modifiers & (Modifier::Shift as u8)) != 0;
                let caps = (state.modifiers & (Modifier::Caps as u8)) != 0;
                let mut c = if shifted { SHIFTED[code as usize] } else { NORMAL[code as usize] };

                if c >= b'a' && c <= b'z' && caps {
                    c = c.to_ascii_uppercase();
                } else if c >= b'A' && c <= b'Z' && caps {
                    c = c.to_ascii_lowercase();
                }

                if c != 0 {
                    drop(state);
                    KEY_BUFFER.lock().push(c);
                }
            }
        }
    }
}

pub fn read_char() -> Option<u8> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        KEY_BUFFER.lock().pop()
    })
}
