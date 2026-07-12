use alloc::vec::Vec;
use crate::io;
use crate::memory;
use crate::process;

const MAX_LINE: usize = 255;
const MAX_ELF_SIZE: usize = 512 * 1024;
// COM2 flow control: the kernel ACKs the daemon every ELF_ACK_CHUNK bytes
// (and on size-parse / finalize) so the host can't blast the ELF faster
// than the UART's 16-byte FIFO can be drained. Without this, multi-KB
// ELFs overflow the FIFO and arrive corrupted.
const ELF_ACK_CHUNK: usize = 256;
const ELF_ACK: u8 = 0x06; // ASCII ACK

enum ElfState {
    Idle,
    AwaitingSize,
    AwaitingData { remaining: usize },
}

pub struct Shell {
    line: [u8; MAX_LINE + 1],
    len: usize,
    prompt: &'static [u8],
    elf_state: ElfState,
    elf_buf: [u8; MAX_ELF_SIZE],
    elf_len: usize,
    last_elf: [u8; MAX_ELF_SIZE],
    last_elf_len: usize,
    size_buf: [u8; 10],
    size_len: usize,
    ack_counter: usize,
}

impl Shell {
    pub fn new(prompt: &'static [u8]) -> Self {
        Shell {
            line: [0; MAX_LINE + 1], len: 0, prompt,
            elf_state: ElfState::Idle,
            elf_buf: [0; MAX_ELF_SIZE],
            elf_len: 0,
            last_elf: [0; MAX_ELF_SIZE],
            last_elf_len: 0,
            size_buf: [0; 10],
            size_len: 0,
            ack_counter: 0,
        }
    }

    pub fn awaiting_response(&self) -> bool {
        matches!(self.elf_state, ElfState::AwaitingSize | ElfState::AwaitingData { .. })
    }

    pub fn handle_daemon_byte(&mut self, b: u8) {
        match self.elf_state {
            ElfState::Idle => {}
            ElfState::AwaitingSize => {
                if b == b'\n' {
                    let size_str = core::str::from_utf8(&self.size_buf[..self.size_len]).unwrap_or("0");
                    let size: usize = size_str.parse().unwrap_or(0);
                    if size == 0 || size > MAX_ELF_SIZE {
                        io::console_write(b"\r\nInvalid ELF size\r\n");
                        // ACK anyway so the daemon unblocks and stops streaming.
                        io::serial_putc_port(io::COM2, ELF_ACK);
                        self.elf_state = ElfState::Idle;
                        self.elf_len = 0;
                        self.size_len = 0;
                        self.ack_counter = 0;
                        self.print_prompt();
                        return;
                    }
                    self.elf_state = ElfState::AwaitingData { remaining: size };
                    self.elf_len = 0;
                    self.size_len = 0;
                    self.ack_counter = 0;
                    // First ACK: tells the daemon it may start streaming the binary.
                    io::serial_putc_port(io::COM2, ELF_ACK);
                } else if b >= b'0' && b <= b'9' {
                    if self.size_len < 9 {
                        self.size_buf[self.size_len] = b;
                        self.size_len += 1;
                    }
                }
            }
            ElfState::AwaitingData { remaining } => {
                if self.elf_len < MAX_ELF_SIZE {
                    self.elf_buf[self.elf_len] = b;
                    self.elf_len += 1;
                }
                self.ack_counter += 1;
                let new_remaining = remaining - 1;
                if new_remaining == 0 {
                    io::console_write(b"\r\nELF received, loading...\r\n");
                    let elf_slice = &self.elf_buf[..self.elf_len];
                    self.last_elf_len = self.elf_len;
                    self.last_elf[..self.elf_len].copy_from_slice(&self.elf_buf[..self.elf_len]);
                    self.elf_state = ElfState::Idle;
                    self.elf_len = 0;
                    self.size_len = 0;
                    self.ack_counter = 0;
                    // Final ACK: keeps the daemon's COM2/TCP stream clean for
                    // the next `gen`, and unblocks its last read.
                    io::serial_putc_port(io::COM2, ELF_ACK);
                    match process::run_elf(elf_slice) {
                        Ok(()) => {}
                        Err(e) => {
                            io::console_write(b"ELF run failed: ");
                            io::console_write(e.as_bytes());
                            io::console_write(b"\r\n");
                            self.print_prompt();
                        }
                    }
                } else {
                    if self.ack_counter >= ELF_ACK_CHUNK {
                        io::serial_putc_port(io::COM2, ELF_ACK);
                        self.ack_counter = 0;
                    }
                    self.elf_state = ElfState::AwaitingData { remaining: new_remaining };
                }
            }
        }
    }

    pub fn handle_char(&mut self, c: u8) {
        match c {
            b'\r' | b'\n' => {
                io::console_write(b"\r\n");
                self.execute();
                self.len = 0;
                self.print_prompt();
            }
            0x08 | 0x7F => {
                if self.len > 0 {
                    self.len -= 1;
                    io::console_write(b"\x08 \x08");
                }
            }
            b if b >= 0x20 && b < 0x7F => {
                if self.len < MAX_LINE {
                    self.line[self.len] = b;
                    self.len += 1;
                    io::console_putc(b);
                }
            }
            _ => {}
        }
    }

    pub fn print_prompt(&self) {
        io::console_write(self.prompt);
    }

    fn execute(&mut self) {
        let len = self.len;
        let mut line_copy = [0u8; MAX_LINE + 1];
        line_copy[..len].copy_from_slice(&self.line[..len]);
        let line = (&line_copy[..len]).trim_ascii();
        if line.is_empty() { return; }

        let (cmd, args) = match line.iter().position(|&b| b == b' ' || b == b'\t') {
            Some(i) => (line[..i].trim_ascii(), line[i..].trim_ascii()),
            None => (line.trim_ascii(), &[][..]),
        };

        match cmd {
            b"help" => cmd_help(),
            b"memory" | b"mem" => cmd_memory(),
            b"clear" | b"cls" => cmd_clear(),
            b"echo" => cmd_echo(args),
            b"info" => cmd_info(),
            b"gen" | b"generate" => self.cmd_gen(args),
            b"run" => self.cmd_run(),
            b"app" => self.cmd_app(args),
            b"user" => cmd_user(),
            b"reboot" => cmd_reboot(),
            b"test-heap" => cmd_test_heap(),
            b"storage" => cmd_storage(args),
            _ => {
                io::console_write(b"Unknown: '");
                io::console_write(cmd);
                io::console_write(b"'. Type 'help'.\r\n");
            }
        }
    }

    fn cmd_gen(&mut self, _args: &[u8]) {
        if _args.is_empty() {
            io::console_write(b"Usage: gen <prompt>\r\n");
            return;
        }
        io::console_write(b"Sending to daemon (COM2)...\r\n");
        io::serial_write_port(io::COM2, b"KARNELOS_GEN:");
        io::serial_write_port(io::COM2, _args);
        io::serial_write_port(io::COM2, b"\n");
        self.elf_state = ElfState::AwaitingSize;
        self.elf_len = 0;
        self.size_len = 0;
    }

    fn cmd_run(&mut self) {
        if self.last_elf_len == 0 {
            io::console_write(b"No ELF loaded. Use 'gen' first.\r\n");
            return;
        }
        io::console_write(b"Running last ELF...\r\n");
        let elf_slice = &self.last_elf[..self.last_elf_len];
        match process::run_elf(elf_slice) {
            Ok(()) => {}
            Err(e) => {
                io::console_write(b"ELF run failed: ");
                io::console_write(e.as_bytes());
                io::console_write(b"\r\n");
            }
        }
    }

    fn cmd_app(&mut self, args: &[u8]) {
        let mut it = args.splitn(2, |&b| b == b' ' || b == b'\t');
        let sub = it.next().unwrap_or(&[][..]).trim_ascii();
        let rest = it.next().unwrap_or(&[][..]).trim_ascii();

        match sub {
            b"save" => {
                let sp = rest.iter().position(|&b| b == b' ' || b == b'\t');
                match sp {
                    Some(i) => {
                        let name = rest[..i].trim_ascii();
                        if name.is_empty() {
                            io::console_write(b"Usage: app save <name>\r\n");
                        } else if self.last_elf_len == 0 {
                            io::console_write(b"app: no ELF in memory (use 'gen' first)\r\n");
                        } else {
                            crate::filesystem::write_file(name, &self.last_elf[..self.last_elf_len]);
                            io::console_write(b"app: saved to storage\r\n");
                        }
                    }
                    None => io::console_write(b"Usage: app save <name>\r\n"),
                }
            }
            b"run" => {
                if rest.is_empty() {
                    io::console_write(b"Usage: app run <name>\r\n");
                    return;
                }
                let mut buf = [0u8; MAX_ELF_SIZE];
                let n = crate::filesystem::read_file(rest, &mut buf);
                if n == 0 {
                    io::console_write(b"app: not found (run 'storage format' first?)\r\n");
                } else {
                    io::console_write(b"Loading app from storage...\r\n");
                    match process::run_elf(&buf[..n]) {
                        Ok(()) => {}
                        Err(e) => {
                            io::console_write(b"ELF run failed: ");
                            io::console_write(e.as_bytes());
                            io::console_write(b"\r\n");
                        }
                    }
                }
            }
            _ => io::console_write(b"Usage: app save <name> | app run <name>\r\n"),
        }
    }
}

#[allow(dead_code)]
trait AsciiTrim {
    fn trim_ascii(&self) -> &[u8];
}

#[allow(dead_code)]
impl AsciiTrim for [u8] {
    fn trim_ascii(&self) -> &[u8] {
        let s = self.iter().position(|&b| b != b' ' && b != b'\t').unwrap_or(self.len());
        let e = self.iter().rposition(|&b| b != b' ' && b != b'\t').map(|i| i + 1).unwrap_or(0);
        &self[s..e]
    }
}

fn writeln(s: &[u8]) {
    io::console_write(s);
    io::console_write(b"\r\n");
}

fn cmd_help() {
    writeln(b"Available commands:");
    writeln(b"  help        - Show this help");
    writeln(b"  memory|mem  - Show memory info");
    writeln(b"  clear|cls   - Clear screen");
    writeln(b"  echo <text> - Echo text");
    writeln(b"  info        - System information");
    writeln(b"  gen|generate <prompt> - Generate a ring-3 app via LLM (no reboot)");
    writeln(b"  run         - Run the last generated app");
    writeln(b"  app save <name> - Save last app to persistent storage");
    writeln(b"  app run <name>  - Run a saved app from storage");
    writeln(b"  user        - Test ring 3 user execution");
    writeln(b"  reboot      - Reboot the system");
    writeln(b"  test-heap   - Run heap allocation test");
    writeln(b"  storage <cmd> - Persistent storage (format|ls|write|read|info)");
}

fn cmd_memory() {
    memory::FRAME_ALLOCATOR.lock().print_info(*memory::PHYS_MEM_OFFSET.lock());
}

fn cmd_clear() {
    io::vga_clear(0x0F, 0x00);
    let mut fb = io::FRAMEBUFFER.lock();
    fb.row = 8;
    fb.col = 0;
}

fn cmd_info() {
    writeln(b"Karnelos OS v0.1");
    writeln(b"Arch: x86-64");
    writeln(b"Mode: Long mode (64-bit)");
    writeln(b"Kernel: Rust no_std");
    cmd_memory();
}

fn cmd_echo(args: &[u8]) {
    if args.is_empty() {
        writeln(b"");
    } else {
        writeln(args);
    }
}

fn cmd_test_heap() {
    let mut v: Vec<u8> = Vec::new();
    for i in 0..200 { v.push(i as u8); }
    let mut ok = true;
    for (i, &val) in v.iter().enumerate() {
        if val != i as u8 { ok = false; break; }
    }
    if ok {
        writeln(b"Heap test: Vec<u8>[200] OK");
    } else {
        writeln(b"Heap test: FAILED");
    }

    let s = alloc::format!("String from heap: {} items", v.len());
    writeln(s.as_bytes());
}

fn cmd_user() {
    crate::process::run_user_demo();
}

fn cmd_reboot() {
    writeln(b"Rebooting...");
    io::reboot();
}

fn cmd_storage(args: &[u8]) {
    let mut it = args.splitn(2, |&b| b == b' ' || b == b'\t');
    let sub = it.next().unwrap_or(&[][..]).trim_ascii();
    let rest = it.next().unwrap_or(&[][..]).trim_ascii();

    match sub {
        b"format" => crate::filesystem::format(),
        b"ls" | b"list" => crate::filesystem::list(),
        b"info" => crate::filesystem::info(),
        b"write" => {
            let sp = rest.iter().position(|&b| b == b' ' || b == b'\t');
            match sp {
                Some(i) => {
                    let name = rest[..i].trim_ascii();
                    let text = rest[i..].trim_ascii();
                    if name.is_empty() {
                        writeln(b"Usage: storage write <name> <text>");
                    } else {
                        crate::filesystem::write_file(name, text);
                    }
                }
                None => writeln(b"Usage: storage write <name> <text>"),
            }
        }
        b"read" => {
            if rest.is_empty() {
                writeln(b"Usage: storage read <name>");
            } else {
                let mut buf = [0u8; 4096];
                let n = crate::filesystem::read_file(rest, &mut buf);
                if n == 0 {
                    writeln(b"storage: file not found");
                } else {
                    io::console_write(b"\r\n");
                    io::console_write(&buf[..n]);
                    io::console_write(b"\r\n");
                }
            }
        }
        _ => writeln(b"Usage: storage <format|ls|write|read|info>"),
    }
}
