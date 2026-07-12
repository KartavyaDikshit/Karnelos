#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
extern crate alloc;
#[path = "../rt.rs"] mod rt;
#[path = "../syscall.rs"] mod syscall;
use syscall::*;
use alloc::vec::Vec;
use alloc::string::String;
use alloc::string::ToString;

fn getchar() -> u8 {
    loop { let c = unsafe { syscall_raw(6, 0, 0, 0, 0, 0, 0) } as u8; if c != 0 { return c; } }
}

fn read_line() -> String {
    let mut s = String::new();
    loop {
        let c = getchar();
        if c == b'\r' || c == b'\n' { break; }
        if c == 0x08 || c == 0x7F { if !s.is_empty() { s.pop(); } }
        else { s.push(c as char); }
    }
    s
}

fn storage_read(name: &[u8]) -> Vec<u8> {
    let mut buf = [0u8; 4096];
    let n = unsafe { syscall_raw(4, name.as_ptr() as u64, buf.as_mut_ptr() as u64, 4096, 0, 0, 0) };
    buf[..n as usize].to_vec()
}

fn storage_write(name: &[u8], data: &[u8]) {
    unsafe { syscall_raw(5, name.as_ptr() as u64, data.as_ptr() as u64, data.len() as u64, 0, 0, 0); }
}

fn print(s: &str) { syscall::print(s); }

fn load_lines(data: &[u8]) -> Vec<String> {
    let mut lines = Vec::new();
    let s = core::str::from_utf8(data).unwrap_or("");
    for line in s.lines() { lines.push(String::from(line)); }
    lines
}

fn save_lines(filename: &str, lines: &[String]) {
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        out.push_str(line);
        if i < lines.len() - 1 { out.push('\n'); }
    }
    storage_write(filename.as_bytes(), out.as_bytes());
}

fn list_lines(lines: &[String], around: usize) {
    if lines.is_empty() { print("(empty)\r\n"); return; }
    let start = if around > 3 { around - 3 } else { 0 };
    let end = (around + 3).min(lines.len());
    if end == 0 {
        let n = lines.len().min(20);
        for i in 0..n { print(&alloc::format!("{:>4}: {}\r\n", i + 1, lines[i])); }
    } else {
        for i in start..end { print(&alloc::format!("{:>4}: {}\r\n", i + 1, lines[i])); }
    }
}

fn editor_main(filename: &str) -> ! {
    let mut lines = Vec::new();
    let data = storage_read(filename.as_bytes());
    if !data.is_empty() {
        lines = load_lines(&data);
        print(&alloc::format!("Loaded {} lines\r\n", lines.len()));
    } else {
        print(&alloc::format!("New file\r\n"));
    }
    list_lines(&lines, 0);

    loop {
        print(":> ");
        let cmd = read_line();
        let cmd = cmd.trim().to_string();
        if cmd.is_empty() { continue; }

        if cmd == ":q" || cmd == ":quit" { print("Goodbye!\r\n"); exit(0); }
        else if cmd == ":w" || cmd == ":save" {
            save_lines(filename, &lines);
            print(&alloc::format!("Saved {} lines\r\n", lines.len()));
        }
        else if cmd == ":h" || cmd == ":help" {
            print(":l [n]  - List lines around n\r\n:i n text - Insert at line n\r\n");
            print(":d n    - Delete line n\r\n:r n text - Replace line n\r\n");
            print(":w      - Save  :q - Quit\r\n");
        }
        else if cmd.starts_with(":l") {
            let n: usize = cmd[2..].trim().parse().unwrap_or(0);
            list_lines(&lines, if n > 0 { n - 1 } else { 0 });
        }
        else if cmd.starts_with(":i ") {
            let rest = cmd[3..].trim();
            if let Some(pos) = rest.find(' ') {
                let n: usize = rest[..pos].parse().unwrap_or(0);
                let text = &rest[pos+1..];
                if n > 0 && n <= lines.len() + 1 {
                    lines.insert(n - 1, String::from(text));
                    print("Inserted\r\n");
                } else { print("Invalid line\r\n"); }
            } else { print(":i n text\r\n"); }
        }
        else if cmd.starts_with(":d ") {
            let n: usize = cmd[3..].trim().parse().unwrap_or(0);
            if n > 0 && n <= lines.len() { lines.remove(n - 1); print("Deleted\r\n"); }
            else { print("Invalid line\r\n"); }
        }
        else if cmd.starts_with(":r ") {
            let rest = cmd[3..].trim();
            if let Some(pos) = rest.find(' ') {
                let n: usize = rest[..pos].parse().unwrap_or(0);
                let text = &rest[pos+1..];
                if n > 0 && n <= lines.len() { lines[n - 1] = String::from(text); print("Replaced\r\n"); }
                else { print("Invalid line\r\n"); }
            } else { print(":r n text\r\n"); }
        }
        else { print("Unknown. :h for help\r\n"); }
    }
}

fn main() -> i32 {
    print("=== Text Editor ===\r\n");
    print("Filename: ");
    let filename = read_line();
    if filename.is_empty() { print("No filename\r\n"); exit(0); }
    editor_main(&filename);
}
