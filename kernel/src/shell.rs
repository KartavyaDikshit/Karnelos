use alloc::vec::Vec;
use crate::io;
use crate::memory;

const MAX_LINE: usize = 255;
const RESP_BUF: usize = 128;

pub struct Shell {
    line: [u8; MAX_LINE + 1],
    len: usize,
    prompt: &'static [u8],
    resp_buf: [u8; RESP_BUF],
    resp_len: usize,
    awaiting_response: bool,
}

impl Shell {
    pub fn new(prompt: &'static [u8]) -> Self {
        Shell { line: [0; MAX_LINE + 1], len: 0, prompt,
                resp_buf: [0; RESP_BUF], resp_len: 0, awaiting_response: false }
    }

    pub fn awaiting_response(&self) -> bool { self.awaiting_response }

    pub fn handle_daemon_byte(&mut self, b: u8) {
        if b == b'\n' || self.resp_len >= RESP_BUF - 1 {
            self.resp_buf[self.resp_len] = 0;
            let len = self.resp_len + 1;
            let trimmed = self.resp_buf[..len].trim_ascii();
            io::console_write(trimmed);
            io::console_write(b"\r\n");
            if trimmed == b"BUILD_OK" {
                io::console_write(b"Type 'reboot' to load the new kernel with generated code.\r\n");
            }
            self.resp_len = 0;
            self.awaiting_response = false;
            self.print_prompt();
        } else if b == b'\r' {
            // skip
        } else {
            self.resp_buf[self.resp_len] = b;
            self.resp_len += 1;
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
            b"run" => cmd_run(),
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
        self.awaiting_response = true;
        self.resp_len = 0;
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
    writeln(b"  gen|generate <prompt> - Generate code via LLM");
    writeln(b"  run         - Run the last generated code");
    writeln(b"  user        - Test ring 3 user execution");
    writeln(b"  reboot      - Reboot the system (loads new kernel after gen)");
    writeln(b"  test-heap   - Run heap allocation test");
    writeln(b"  storage <cmd> - Persistent storage (format|ls|write|read|info)");
}

fn cmd_memory() {
    memory::FRAME_ALLOCATOR.lock().print_info(*memory::PHYS_MEM_OFFSET.lock());
}

fn cmd_clear() {
    io::vga_clear(0x0F, 0x00);
    io::VGA_WRITER.lock().row = 8;
    io::VGA_WRITER.lock().col = 0;
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

fn cmd_run() {
    crate::generated::generated_main();
}

fn cmd_user() {
    crate::process::run_user_demo();
}

fn cmd_reboot() {
    writeln(b"Rebooting...");
    io::reboot();
}

fn cmd_storage(args: &[u8]) {
    // Parse subcommand + argument
    let mut it = args.splitn(2, |&b| b == b' ' || b == b'\t');
    let sub = it.next().unwrap_or(&[][..]).trim_ascii();
    let rest = it.next().unwrap_or(&[][..]).trim_ascii();

    match sub {
        b"format" => crate::filesystem::format(),
        b"ls" | b"list" => crate::filesystem::list(),
        b"info" => crate::filesystem::info(),
        b"write" => {
            // rest = "<name> <text...>"
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
