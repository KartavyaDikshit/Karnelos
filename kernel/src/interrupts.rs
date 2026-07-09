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
