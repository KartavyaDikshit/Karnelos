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

const STORE: &[u8] = b"events";

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

fn sr(name: &[u8]) -> Vec<u8> {
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

fn print(s: &str) { syscall::print(s); }

fn parse_events(data: &[u8]) -> Vec<(String, String)> {
    let mut events = Vec::new();
    let s = core::str::from_utf8(data).unwrap_or("");
    for line in s.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Some(pos) = line.find(": ") {
            events.push((String::from(line[..pos].trim()), String::from(line[pos+2..].trim())));
        }
    }
    events
}

fn fmt_events(events: &[(String, String)]) -> String {
    let mut s = String::new();
    for (date, desc) in events {
        s.push_str(&alloc::format!("{}: {}\n", date, desc));
    }
    s
}

fn main() -> i32 {
    print("=== Calendar ===\r\n");
    print("Commands: add <date> <desc>, list, today <desc>, del <n>, remind, help, exit\r\n");
    print("Date format: YYYY-MM-DD or 'today'/'tomorrow'\r\n");
    loop {
        print("cal> ");
        let line = read_line().trim().to_string();
        if line.is_empty() { continue; }
        let (cmd, args) = if let Some(pos) = line.find(' ') {
            (line[..pos].to_string(), line[pos+1..].trim().to_string())
        } else { (line.clone(), String::new()) };
        match cmd.as_str() {
            "add" => {
                if let Some(pos) = args.find(' ') {
                    let date = args[..pos].trim().to_string();
                    let desc = args[pos+1..].trim().to_string();
                    let mut ev = parse_events(&sr(STORE));
                    ev.push((date, desc));
                    sw(STORE, fmt_events(&ev).as_bytes());
                    print("Event added\r\n");
                } else { print("Usage: add <date> <desc>\r\n"); }
            }
            "list" => {
                let ev = parse_events(&sr(STORE));
                if ev.is_empty() { print("No events\r\n"); }
                else { for (i, (d, desc)) in ev.iter().enumerate() { print(&alloc::format!("{}. {}: {}\r\n", i + 1, d, desc)); } }
            }
            "today" => {
                let desc = if args.is_empty() { "Reminder" } else { &args };
                let mut ev = parse_events(&sr(STORE));
                ev.push((String::from("today"), String::from(desc)));
                sw(STORE, fmt_events(&ev).as_bytes());
                print("Added for today\r\n");
            }
            "del" | "delete" => {
                let n: usize = args.parse().unwrap_or(0);
                if n == 0 { print("Usage: del <n>\r\n"); continue; }
                let mut ev = parse_events(&sr(STORE));
                if n > 0 && n <= ev.len() { ev.remove(n-1); sw(STORE, fmt_events(&ev).as_bytes()); print("Deleted\r\n"); }
                else { print("Invalid\r\n"); }
            }
            "remind" => {
                let ev = parse_events(&sr(STORE));
                let mut found = false;
                for (d, desc) in &ev {
                    if d == "today" {
                        print(&alloc::format!("!!! TODAY: {}\r\n", desc));
                        found = true;
                    }
                }
                if !found { print("No reminders for today\r\n"); }
            }
            "help" => print("add <date> <desc>, list, today <desc>, del <n>, remind, exit\r\n"),
            "exit" | "quit" => { print("Bye\r\n"); exit(0); }
            _ => print(&alloc::format!("Unknown: {}\r\n", cmd)),
        }
    }
}