#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
extern crate alloc;
#[path = "../rt.rs"] mod rt;
#[path = "../syscall.rs"] mod syscall;
use syscall::*;
use alloc::string::String;
use alloc::string::ToString;

const STORAGE_NAME: &[u8] = b"todos";

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

fn print(s: &str) { syscall::print(s); }

fn parse_todos(data: &[u8]) -> alloc::vec::Vec<(bool, String, String)> {
    let mut todos = alloc::vec::Vec::new();
    let s = core::str::from_utf8(data).unwrap_or("");
    for line in s.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let done = line.starts_with("[x]");
        let rest = if done { &line[3..] } else if line.starts_with("[ ]") { &line[3..] } else { line };
        let rest = rest.trim();
        if let Some(pos) = rest.find(':') {
            todos.push((done, String::from(rest[..pos].trim()), String::from(rest[pos+1..].trim())));
        } else {
            todos.push((done, String::from("general"), String::from(rest)));
        }
    }
    todos
}

fn fmt_todos(todos: &[(bool, String, String)]) -> String {
    let mut s = String::new();
    for (done, cat, desc) in todos {
        s.push_str(&alloc::format!("[{}] {}: {}\n", if *done { "x" } else { " " }, cat, desc));
    }
    s
}

fn main() -> i32 {
    print("=== Todo App ===\r\n");
    print("Commands: add <cat> <desc>, list [cat], done <n>, del <n>, help, exit\r\n");
    loop {
        print("todo> ");
        let line = read_line().trim().to_string();
        if line.is_empty() { continue; }
        let (cmd, args) = if let Some(pos) = line.find(' ') {
            (line[..pos].to_string(), line[pos+1..].trim().to_string())
        } else { (line.clone(), String::new()) };
        match cmd.as_str() {
            "add" => {
                if args.is_empty() { print("Usage: add <cat> <desc>\r\n"); continue; }
                let (cat, desc) = if let Some(p) = args.find(' ') {
                    (args[..p].trim().to_string(), args[p+1..].trim().to_string())
                } else { (String::from("general"), args.trim().to_string()) };
                let mut todos = parse_todos(&sr(STORAGE_NAME));
                todos.push((false, cat, desc));
                sw(STORAGE_NAME, fmt_todos(&todos).as_bytes());
                print("Added\r\n");
            }
            "list" => {
                let todos = parse_todos(&sr(STORAGE_NAME));
                if todos.is_empty() { print("No todos\r\n"); continue; }
                let filter = if args.is_empty() { None } else { Some(args.as_str()) };
                for (i, (done, cat, desc)) in todos.iter().enumerate() {
                    if let Some(f) = filter { if cat != f { continue; } }
                    print(&alloc::format!("{}. [{}] [{}] {}\r\n", i + 1, if *done { "x" } else { " " }, cat, desc));
                }
            }
            "done" => {
                let n: usize = args.parse().unwrap_or(0);
                if n == 0 { print("Usage: done <n>\r\n"); continue; }
                let mut todos = parse_todos(&sr(STORAGE_NAME));
                if n > 0 && n <= todos.len() { todos[n-1].0 = true; sw(STORAGE_NAME, fmt_todos(&todos).as_bytes()); print("Done\r\n"); }
                else { print("Invalid number\r\n"); }
            }
            "del" | "delete" | "rm" => {
                let n: usize = args.parse().unwrap_or(0);
                if n == 0 { print("Usage: del <n>\r\n"); continue; }
                let mut todos = parse_todos(&sr(STORAGE_NAME));
                if n > 0 && n <= todos.len() { todos.remove(n-1); sw(STORAGE_NAME, fmt_todos(&todos).as_bytes()); print("Deleted\r\n"); }
                else { print("Invalid number\r\n"); }
            }
            "help" => print("add <cat> <desc>, list [cat], done <n>, del <n>, exit\r\n"),
            "exit" | "quit" => { print("Bye\r\n"); exit(0); }
            _ => print(&alloc::format!("Unknown: {}\r\n", cmd)),
        }
    }
}