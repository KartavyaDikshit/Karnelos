#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
extern crate alloc;
mod rt;
mod syscall;
use syscall::*;
use alloc::vec::Vec;
use alloc::string::ToString;

fn getchar() -> u8 {
    loop {
        let c = unsafe { syscall_raw(6, 0, 0, 0, 0, 0, 0) } as u8;
        if c != 0 { return c; }
    }
}

fn read_line() -> alloc::string::String {
    let mut s = alloc::string::String::new();
    loop {
        let c = getchar();
        if c == b'\r' || c == b'\n' { break; }
        if c == 0x08 || c == 0x7F {
            if !s.is_empty() { s.pop(); }
        } else {
            s.push(c as char);
        }
    }
    s
}

fn storage_list() -> alloc::vec::Vec<u8> {
    let mut buf = [0u8; 4096];
    let n = unsafe { syscall_raw(7, buf.as_mut_ptr() as u64, 4096, 0, 0, 0, 0) };
    buf[..n as usize].to_vec()
}

fn storage_read(name: &[u8]) -> Vec<u8> {
    let mut buf = [0u8; 4096];
    let n = unsafe { syscall_raw(4, name.as_ptr() as u64, buf.as_mut_ptr() as u64, 4096, 0, 0, 0) };
    buf[..n as usize].to_vec()
}

fn storage_write(name: &[u8], data: &[u8]) {
    unsafe { syscall_raw(5, name.as_ptr() as u64, data.as_ptr() as u64, data.len() as u64, 0, 0, 0); }
}

fn storage_delete(name: &[u8]) -> bool {
    unsafe { syscall_raw(8, name.as_ptr() as u64, 0, 0, 0, 0, 0) == 0 }
}

fn cmd_help() {
    print("File Manager - Commands:\r\n");
    print("  ls                    - List files\r\n");
    print("  read <filename>       - Read file contents\r\n");
    print("  write <name> <text>   - Write text to a file\r\n");
    print("  delete <filename>     - Delete a file\r\n");
    print("  help                  - Show this help\r\n");
    print("  exit                  - Exit the app\r\n");
}

fn cmd_read(args: &[u8]) {
    let name = args.trim_ascii();
    if name.is_empty() {
        print("Usage: read <filename>\r\n");
        return;
    }
    let data = storage_read(name);
    if data.is_empty() {
        print(&alloc::format!("File '{}' not found\r\n", core::str::from_utf8(name).unwrap_or("?")));
    } else {
        print("--- Content ---\r\n");
        print(core::str::from_utf8(&data).unwrap_or("(binary data)"));
        print("\r\n--- End ---\r\n");
    }
}

fn cmd_write(args: &[u8]) {
    let s = core::str::from_utf8(args).unwrap_or("").trim();
    if let Some(pos) = s.find(' ') {
        let name = s[..pos].trim().as_bytes();
        let content = s[pos+1..].trim().as_bytes();
        storage_write(name, content);
        print(&alloc::format!("Wrote {} bytes to '{}'\r\n", content.len(), core::str::from_utf8(name).unwrap_or("?")));
    } else {
        print("Usage: write <filename> <content>\r\n");
    }
}

fn cmd_delete(args: &[u8]) {
    let name = args.trim_ascii();
    if name.is_empty() {
        print("Usage: delete <filename>\r\n");
        return;
    }
    if storage_delete(name) {
        print(&alloc::format!("Deleted '{}'\r\n", core::str::from_utf8(name).unwrap_or("?")));
    } else {
        print(&alloc::format!("File '{}' not found\r\n", core::str::from_utf8(name).unwrap_or("?")));
    }
}

fn main_loop() -> ! {
    print("=== File Manager ===\r\n");
    print("Type 'help' for commands.\r\n");
    loop {
        print("files> ");
        let line = read_line();
        let line = line.trim().to_string();
        if line.is_empty() { continue; }
        let (cmd, args) = if let Some(pos) = line.find(' ') {
            (line[..pos].to_string(), line[pos+1..].trim().to_string())
        } else {
            (line.clone(), alloc::string::String::new())
        };
        match cmd.as_str() {
            "ls" | "list" => {
                let data = storage_list();
                print(core::str::from_utf8(&data).unwrap_or(""));
            }
            "read" | "cat" => cmd_read(args.as_bytes()),
            "write" => cmd_write(args.as_bytes()),
            "delete" | "del" | "rm" => cmd_delete(args.as_bytes()),
            "help" => cmd_help(),
            "exit" | "quit" => { print("Goodbye!\r\n"); let _ = exit(0); }
            _ => { print(&alloc::format!("Unknown: '{}'. Type 'help'.\r\n", cmd)); }
        }
    }
}

fn main() -> i32 {
    main_loop();
}