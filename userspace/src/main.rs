#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
extern crate alloc;
mod rt;
mod syscall;
use syscall::*;
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

fn print(s: &str) { syscall::print(s); }

fn main() -> i32 {
    print("=== Karnelos App Launcher ===\r\n");
    print("Available apps:\r\n");
    print("  1 - Todo App (categories, persistent storage)\r\n");
    print("  2 - File Manager (list/read/write/delete files)\r\n");
    print("  3 - Text Editor (line-based file editor)\r\n");
    print("  4 - Calendar (events and reminders)\r\n");
    print("  5 - Math Compiler (expression evaluator/code gen)\r\n");
    print("\r\nSwitch app in: userspace/src/main.rs\r\n");
    print("Or build individual bins: make userspace-bins\r\n");
    loop {
        print("launcher> ");
        let line = read_line().trim().to_string();
        if line.is_empty() { continue; }
        if line == "exit" || line == "quit" { print("Bye\r\n"); exit(0); }
        print(&alloc::format!("App {} selected. Build with: cargo build --bin app{}\r\n", line, line));
    }
}