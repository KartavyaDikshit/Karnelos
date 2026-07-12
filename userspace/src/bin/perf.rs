#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
extern crate alloc;
#[path = "../rt.rs"] mod rt;
#[path = "../syscall.rs"] mod syscall;
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

fn get_metrics(clear: bool) -> alloc::vec::Vec<u8> {
    let mut buf = [0u8; 4096];
    let n = unsafe { syscall_raw(9, buf.as_mut_ptr() as u64, 4096, if clear { 1 } else { 0 }, 0, 0, 0) };
    buf[..n as usize].to_vec()
}

fn main() -> i32 {
    print("=== Performance Dashboard ===\r\n");
    print("Commands: show, clear, refresh, help, exit\r\n");
    loop {
        print("perf> ");
        let line = read_line().trim().to_string();
        if line.is_empty() { continue; }
        match line.as_str() {
            "show" | "s" | "display" => {
                let data = get_metrics(false);
                print(core::str::from_utf8(&data).unwrap_or(""));
            }
            "clear" | "reset" => {
                let data = get_metrics(true);
                print("Metrics cleared.\r\n");
            }
            "refresh" | "r" => {
                let data = get_metrics(false);
                print("Current metrics:\r\n");
                print(core::str::from_utf8(&data).unwrap_or(""));
            }
            "help" | "h" => {
                print("Commands:\r\n");
                print("  show    - Display all performance metrics\r\n");
                print("  clear   - Reset all metrics to zero\r\n");
                print("  refresh - Show current metrics\r\n");
                print("  exit    - Exit\r\n");
            }
            "exit" | "quit" | "q" => { print("Bye!\r\n"); syscall::exit(0); }
            _ => print(&alloc::format!("Unknown: {}. Type help\r\n", line)),
        }
    }
}