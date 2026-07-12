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

fn sr(name: &[u8]) -> alloc::vec::Vec<u8> {
    let mut buf = [0u8; 4096];
    let n = unsafe { syscall_raw(4, name.as_ptr() as u64, buf.as_mut_ptr() as u64, 4096, 0, 0, 0) };
    buf[..n as usize].to_vec()
}

fn sw(name: &[u8], data: &[u8]) {
    unsafe { syscall_raw(5, name.as_ptr() as u64, data.as_ptr() as u64, data.len() as u64, 0, 0, 0); }
}

fn sd(name: &[u8]) -> bool {
    unsafe { syscall_raw(8, name.as_ptr() as u64, 0, 0, 0, 0, 0) == 0 }
}

fn slist() -> alloc::vec::Vec<u8> {
    let mut buf = [0u8; 4096];
    let n = unsafe { syscall_raw(7, buf.as_mut_ptr() as u64, 4096, 0, 0, 0, 0) };
    buf[..n as usize].to_vec()
}

fn print(s: &str) { syscall::print(s); }

fn main() -> i32 {
    print("=== File Manager ===\r\n");
    print("Commands: ls, read <f>, write <f> <t>, del <f>, help, exit\r\n");
    loop {
        print("files> ");
        let line = read_line().trim().to_string();
        if line.is_empty() { continue; }
        let (cmd, args) = if let Some(pos) = line.find(' ') {
            (line[..pos].to_string(), line[pos+1..].trim().to_string())
        } else { (line.clone(), String::new()) };
        match cmd.as_str() {
            "ls" | "list" => { print(core::str::from_utf8(&slist()).unwrap_or("")); }
            "read" | "cat" => {
                let name = args.trim().as_bytes().to_vec();
                if name.is_empty() { print("Usage: read <f>\r\n"); continue; }
                let data = sr(&name);
                if data.is_empty() { print("Not found\r\n"); }
                else { print("---\r\n"); print(core::str::from_utf8(&data).unwrap_or("")); print("\r\n---\r\n"); }
            }
            "write" => {
                if let Some(pos) = args.find(' ') {
                    let name = args[..pos].trim().as_bytes().to_vec();
                    let text = args[pos+1..].trim().as_bytes().to_vec();
                    sw(&name, &text);
                    print(&alloc::format!("Wrote {} bytes\r\n", text.len()));
                } else { print("Usage: write <f> <t>\r\n"); }
            }
            "del" | "delete" | "rm" => {
                let name = args.trim().as_bytes().to_vec();
                if name.is_empty() { print("Usage: del <f>\r\n"); continue; }
                if sd(&name) { print("Deleted\r\n"); } else { print("Not found\r\n"); }
            }
            "help" => print("ls, read <f>, write <f> <t>, del <f>, exit\r\n"),
            "exit" | "quit" => { print("Bye\r\n"); exit(0); }
            _ => print(&alloc::format!("Unknown: {}\r\n", cmd)),
        }
    }
}