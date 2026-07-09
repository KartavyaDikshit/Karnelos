use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use pic8259::ChainedPics;
use spin::Mutex;
use crate::keyboard;
use crate::io;

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
        idt.double_fault.set_handler_fn(double_fault_handler);
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

extern "x86-interrupt" fn double_fault_handler(stack: InterruptStackFrame, _code: u64) -> ! {
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
// r8 = pointer to CPU-pushed frame: [RIP, CS, RFLAGS, RSP, SS]
core::arch::global_asm!(
    ".globl int_80_stub",
    "int_80_stub:",
    "  sub rsp, 48",
    "  mov [rsp], rax",
    "  mov [rsp+8], rbx",
    "  mov [rsp+16], rcx",
    "  mov [rsp+24], rdx",
    "  mov [rsp+32], rsi",
    "  mov [rsp+40], rdi",
    "  mov rdi, [rsp]",     // arg1: syscall number (was rax)
    "  mov rsi, [rsp+8]",   // arg2: arg1 (was rbx)
    "  mov rdx, [rsp+16]",  // arg3: arg2 (was rcx)
    "  mov rcx, [rsp+24]",  // arg4: arg3 (was rdx)
    "  lea r8, [rsp+48]",   // arg5: pointer to CPU-pushed frame
    "  call syscall_handler",
    "  mov [rsp], rax",     // save return value over saved rax
    "  mov rbx, [rsp+8]",
    "  mov rcx, [rsp+16]",
    "  mov rdx, [rsp+24]",
    "  mov rsi, [rsp+32]",
    "  mov rdi, [rsp+40]",
    "  add rsp, 48",
    "  iretq",
);

#[no_mangle]
extern "C" fn syscall_handler(num: u64, arg1: u64, arg2: u64, _arg3: u64, _frame: *mut [u64; 5]) -> u64 {
    match num {
        0 => {
            io::console_write(b"User program exited\r\n");
            // Switch to the saved kernel stack and return to the shell.
            if let Some(ctx) = crate::userspace::EXIT_CTX.lock().take() {
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
            // fallback: shouldn't reach here
            loop { x86_64::instructions::hlt(); }
        }
        1 => {
            // console_write(buf_addr, len)
            let buf = arg1 as *const u8;
            let len = arg2 as usize;
            if len > 0 && len <= 4096 {
                let slice = unsafe { core::slice::from_raw_parts(buf, len) };
                io::console_write(slice);
            }
            0
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
