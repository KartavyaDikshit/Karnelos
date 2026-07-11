use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use x86_64::PhysAddr;
use x86_64::structures::paging::PhysFrame;
use x86_64::registers::control::{Cr3, Cr3Flags};
use pic8259::ChainedPics;
use spin::Mutex;
use crate::keyboard;
use crate::io;
use crate::memory;
use crate::filesystem;
use crate::process;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = 40;

pub static PICS: Mutex<ChainedPics> = Mutex::new(
    unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) }
);

#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard = PIC_1_OFFSET + 1,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 { self as u8 }
    fn as_usize(self) -> usize { usize::from(self.as_u8()) }
}

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

pub fn init() {
    unsafe {
        let idt = &mut *core::ptr::addr_of_mut!(IDT);
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        // Register via set_handler_addr to avoid the diverging-handler
        // (`-> !`) type requirement; the function itself returns `()`.
        let df_addr = x86_64::VirtAddr::new(double_fault_handler as *const () as u64);
        idt.double_fault.set_handler_addr(df_addr);
        idt.general_protection_fault.set_handler_fn(gpf_handler);
        idt.invalid_opcode.set_handler_fn(invalid_opcode_handler);
        idt.stack_segment_fault.set_handler_fn(ssf_handler);
        idt.divide_error.set_handler_fn(divide_error_handler);

        idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer_handler);
        idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(keyboard_handler);

        (&*core::ptr::addr_of!(IDT)).load();

        PICS.lock().initialize();
        x86_64::instructions::interrupts::enable();
    }
}

fn print_exception(name: &[u8], stack: &InterruptStackFrame, err: Option<u64>) {
    io::serial_write(b"\r\n*** EXCEPTION: ");
    io::serial_write(name);
    io::serial_write(b" ***\r\n");
    if let Some(e) = err {
        io::serial_write(b"Error code: ");
        io::serial_write(b"0x");
        let mut v = e;
        for _ in 0..16 {
            let nibble = (v >> 60) as u8;
            io::serial_putc(if nibble < 10 { b'0' + nibble } else { b'A' + nibble - 10 });
            v <<= 4;
        }
        io::serial_write(b"\r\n");
    }
    io::serial_write(b"RIP: 0x");
    let rip = stack.instruction_pointer.as_u64();
    for i in (0..16).rev() {
        let nibble = ((rip >> (i * 4)) & 0xF) as u8;
        io::serial_putc(if nibble < 10 { b'0' + nibble } else { b'A' + nibble - 10 });
    }
    io::serial_write(b"\r\n");
}

fn halt_loop() -> ! {
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

extern "x86-interrupt" fn breakpoint_handler(stack: InterruptStackFrame) {
    print_exception(b"BREAKPOINT", &stack, None);
}

extern "x86-interrupt" fn page_fault_handler(stack: InterruptStackFrame, code: PageFaultErrorCode) {
    print_exception(b"PAGE FAULT", &stack, Some(code.bits() as u64));
    halt_loop();
}

extern "x86-interrupt" fn double_fault_handler(stack: InterruptStackFrame, _code: u64) {
    print_exception(b"DOUBLE FAULT", &stack, None);
    halt_loop();
}

extern "x86-interrupt" fn gpf_handler(stack: InterruptStackFrame, code: u64) {
    print_exception(b"GENERAL PROTECTION FAULT", &stack, Some(code));
    halt_loop();
}

extern "x86-interrupt" fn invalid_opcode_handler(stack: InterruptStackFrame) {
    print_exception(b"INVALID OPCODE", &stack, None);
    halt_loop();
}

extern "x86-interrupt" fn divide_error_handler(stack: InterruptStackFrame) {
    print_exception(b"DIVIDE BY ZERO", &stack, None);
    halt_loop();
}

extern "x86-interrupt" fn ssf_handler(stack: InterruptStackFrame, code: u64) {
    print_exception(b"STACK SEGMENT FAULT", &stack, Some(code));
    halt_loop();
}

extern "x86-interrupt" fn timer_handler(_stack: InterruptStackFrame) {
    unsafe { PICS.lock().notify_end_of_interrupt(InterruptIndex::Timer.as_u8()); }
}

extern "x86-interrupt" fn keyboard_handler(_stack: InterruptStackFrame) {
    let scancode = unsafe { x86_64::instructions::port::Port::new(0x60).read() };
    keyboard::handle_scancode(scancode);
    unsafe { PICS.lock().notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8()); }
}

// --- Syscall (int 0x80) ---
// Stable ABI (matches the userspace `syscall!` macro):
//   rax = syscall number
//   rdi, rsi, rdx, r10, r8, r9 = up to 6 arguments
//   return value in rax
//
// The stub saves all GPRs, shuffles them into the C calling convention
// (rdi,rsi,rdx,rcx,r8,r9), calls `syscall_handler`, restores the return value
// into the saved rax slot, restores the GPRs, and `iretq`s back to ring 3.
core::arch::global_asm!(
    ".globl int_80_stub",
    "int_80_stub:",
    "  push rax",
    "  push rbx",
    "  push rcx",
    "  push rdx",
    "  push rsi",
    "  push rdi",
    "  push rbp",
    "  push r8",
    "  push r9",
    "  push r10",
    "  push r11",
    "  push r12",
    "  push r13",
    "  push r14",
    "  push r15",
    "  mov rdi, rax",          // num
    "  mov rsi, [rsp+5*8]",     // saved rdi  -> arg1
    "  mov rdx, [rsp+4*8]",     // saved rsi  -> arg2
    "  mov rcx, [rsp+3*8]",     // saved rdx  -> arg3
    "  mov r8,  [rsp+9*8]",     // saved r10  -> arg4
    "  mov r9,  [rsp+7*8]",     // saved r8   -> arg5
    "  call syscall_handler",
    "  mov [rsp], rax",         // store return over saved rax
    "  pop r15",
    "  pop r14",
    "  pop r13",
    "  pop r12",
    "  pop r11",
    "  pop r10",
    "  pop r9",
    "  pop r8",
    "  pop rbp",
    "  pop rdi",
    "  pop rsi",
    "  pop rdx",
    "  pop rcx",
    "  pop rbx",
    "  pop rax",
    "  iretq",
);

#[no_mangle]
extern "C" fn syscall_handler(
    num: u64,
    a: u64,
    b: u64,
    c: u64,
    _d: u64,
    _e: u64,
) -> u64 {
    match num {
        // exit(code)
        0 => {
            let ctx = process::EXIT_CTX.lock().take();
            if let Some(ctx) = ctx {
                // Restore the kernel page tables.
                let kp = PhysFrame::from_start_address(PhysAddr::new(ctx.kernel_p4_phys)).unwrap();
                unsafe { Cr3::write(kp, Cr3Flags::empty()); }
                // Free the process's frames.
                if let Some(proc) = process::CURRENT_PROCESS.lock().take() {
                    for f in proc.frames {
                        memory::FRAME_ALLOCATOR.lock().deallocate(f);
                    }
                    memory::FRAME_ALLOCATOR.lock().deallocate(proc.p4_frame);
                }
                // Switch to the kernel stack and return to the shell.
                unsafe {
                    core::arch::asm!(
                        "mov rsp, {rsp}",
                        "push {rip}",
                        "ret",
                        rsp = in(reg) ctx.kernel_rsp,
                        rip = in(reg) ctx.kernel_ret_rip,
                    );
                }
            }
            loop { x86_64::instructions::hlt(); }
        }
        // write(buf_ptr, len) -> bytes written
        1 => {
            let buf = a as *const u8;
            let len = b as usize;
            if len > 0 && len <= 8192 {
                let slice = unsafe { core::slice::from_raw_parts(buf, len) };
                io::console_write(slice);
            }
            0
        }
        // read(buf_ptr, len) -> bytes read (from keyboard)
        2 => {
            let buf = a as *mut u8;
            let max = b as usize;
            let mut n = 0usize;
            while n < max {
                match crate::keyboard::read_char() {
                    Some(ch) => {
                        unsafe { *buf.add(n) = ch; }
                        n += 1;
                        if ch == b'\r' || ch == b'\n' { break; }
                    }
                    None => break,
                }
            }
            n as u64
        }
        // storage_read(name_ptr, buf_ptr, len) -> bytes read
        4 => {
            let (name, nl) = read_name(a as *const u8);
            let buf = b as *mut u8;
            let buflen = c as usize;
            let mut kbuf = [0u8; 4096];
            let n = filesystem::read_file(&name[..nl], &mut kbuf);
            let n = n.min(buflen).min(4096);
            unsafe { core::ptr::copy_nonoverlapping(kbuf.as_ptr(), buf, n); }
            n as u64
        }
        // storage_write(name_ptr, data_ptr, len) -> bytes written
        5 => {
            let (name, nl) = read_name(a as *const u8);
            let data = b as *const u8;
            let len = c as usize;
            let n = len.min(4096);
            let mut kbuf = [0u8; 4096];
            unsafe { core::ptr::copy_nonoverlapping(data, kbuf.as_mut_ptr(), n); }
            filesystem::write_file(&name[..nl], &kbuf[..n]);
            n as u64
        }
        // getchar() -> char or 0
        6 => {
            match crate::keyboard::read_char() {
                Some(ch) => ch as u64,
                None => 0,
            }
        }
        42 => {
            io::console_write(b"Hello from ring 3!\r\n");
            0
        }
        _ => {
            io::console_write(b"Unknown syscall: ");
            let mut buf = [0u8; 20];
            let mut i = 20;
            let mut n = num;
            while n > 0 { i -= 1; buf[i] = b'0' + (n % 10) as u8; n /= 10; }
            if i == 20 { io::console_putc(b'0'); } else { io::console_write(&buf[i..]); }
            io::console_write(b"\r\n");
            0
        }
    }
}

/// Read a NUL-terminated name from a user pointer (max 56 bytes).
fn read_name(ptr: *const u8) -> ([u8; 56], usize) {
    let mut name = [0u8; 56];
    let mut nl = 0;
    while nl < 55 {
        let ch = unsafe { *ptr.add(nl) };
        if ch == 0 { break; }
        name[nl] = ch;
        nl += 1;
    }
    (name, nl)
}

pub fn register_int0x80() {
    unsafe {
        let idt = &mut *core::ptr::addr_of_mut!(IDT);
        extern "C" { fn int_80_stub(); }
        let addr = x86_64::VirtAddr::new(int_80_stub as usize as u64);
        idt[0x80].set_handler_addr(addr)
            .set_privilege_level(x86_64::PrivilegeLevel::Ring3);
        io::console_write(b"int 0x80 registered with DPL=3\r\n");
    }
}
