use alloc::vec::Vec;
use crate::io;
use crate::memory;

const MAX_LINE: usize = 255;

pub struct Shell {
    line: [u8; MAX_LINE + 1],
    len: usize,
    prompt: &'static [u8],
}

impl Shell {
    pub fn new(prompt: &'static [u8]) -> Self {
        Shell { line: [0; MAX_LINE + 1], len: 0, prompt }
    }

    pub fn handle_char(&mut self, c: u8) {
        match c {
            b'\r' | b'\n' => {
                io::serial_write(b"\r\n");
                self.execute();
                self.len = 0;
                self.print_prompt();
            }
            0x08 | 0x7F => {
                if self.len > 0 {
                    self.len -= 1;
                    io::serial_write(b"\x08 \x08");
                }
            }
            b if b >= 0x20 && b < 0x7F => {
                if self.len < MAX_LINE {
                    self.line[self.len] = b;
                    self.len += 1;
                    io::serial_putc(b);
                }
            }
            _ => {}
        }
    }

    pub fn print_prompt(&self) {
        io::serial_write(self.prompt);
    }

    fn execute(&self) {
        let line = &self.line[..self.len];
        let line = line.trim_ascii();
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
            b"gen" | b"generate" => cmd_gen(args),
            b"test-heap" => cmd_test_heap(),
            _ => {
                io::serial_write(b"Unknown: '");
                io::serial_write(cmd);
                io::serial_write(b"'. Type 'help'.\r\n");
            }
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
    io::serial_write(s);
    io::serial_write(b"\r\n");
}

fn cmd_help() {
    writeln(b"Available commands:");
    writeln(b"  help        - Show this help");
    writeln(b"  memory|mem  - Show memory info");
    writeln(b"  clear|cls   - Clear screen");
    writeln(b"  echo <text> - Echo text");
    writeln(b"  info        - System information");
    writeln(b"  gen|generate <prompt> - Generate code via LLM");
    writeln(b"  test-heap   - Run heap allocation test");
}

fn cmd_memory() {
    memory::FRAME_ALLOCATOR.lock().print_info(*memory::PHYS_MEM_OFFSET.lock());
}

fn cmd_clear() {
    io::vga_clear(0x0F, 0x00);
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

fn cmd_gen(_args: &[u8]) {
    if _args.is_empty() {
        writeln(b"Usage: gen <prompt>");
        writeln(b"Example: gen print hello world");
        return;
    }
    writeln(b"Sending to daemon (port COM2)...");
    io::serial_write_port(io::COM2, b"KARNELOS_GEN:");
    io::serial_write_port(io::COM2, _args);
    io::serial_write_port(io::COM2, b"\n");
    writeln(b"Check daemon terminal for build result.");
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
