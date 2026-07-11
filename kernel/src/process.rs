//! Ring-3 process model for Karnelos.
//!
//! Each app runs as an isolated process with its own page tables: a fresh P4
//! whose upper half (kernel space) is cloned from the kernel's P4, and whose
//! lower half holds the app's code/stack/heap. The kernel switches `CR3`, maps
//! the ELF (via the loader), and `iretq`s into ring 3. On the `exit` syscall the
//! process's frames are freed, `CR3` is restored, and control returns to the
//! shell.

use alloc::boxed::Box;
use alloc::vec::Vec;
use x86_64::structures::gdt::{GlobalDescriptorTable, Descriptor, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::structures::paging::PhysFrame;
use x86_64::VirtAddr;
use x86_64::registers::segmentation::{CS, SS, DS, ES, Segment};
use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::instructions::tables::load_tss;
use x86_64::PhysAddr;
use spin::Mutex;
use crate::io;
use crate::memory;
use crate::loader;

pub const USER_STACK_TOP: u64 = 0x8000_0000;
const USER_STACK_PAGES: usize = 16;
const DEMO_BASE: u64 = 0x50_0000;

static mut TSS: TaskStateSegment = TaskStateSegment::new();

pub struct ExitContext {
    pub kernel_rsp: u64,
    pub kernel_ret_rip: u64,
    pub kernel_p4_phys: u64,
}

pub static EXIT_CTX: Mutex<Option<ExitContext>> = Mutex::new(None);

pub struct Process {
    pub p4_frame: usize,
    pub frames: Vec<usize>,
}

pub static CURRENT_PROCESS: Mutex<Option<Process>> = Mutex::new(None);

struct Selectors {
    user_cs: SegmentSelector,
    user_ds: SegmentSelector,
}

static GDT_SEL: Mutex<Option<Selectors>> = Mutex::new(None);

// --- Built-in ring-3 demo program (inline asm) ---
core::arch::global_asm!(
    ".section .user_prog, \"ax\"",
    ".globl _user_prog_start",
    ".globl _user_prog_end",
    "_user_prog_start:",
    // syscall 42: print "Hello from ring 3!"
    "  mov eax, 42",
    "  int 0x80",
    // syscall 1: console_write(rdi=buf, rsi=len)
    "  mov eax, 1",
    "  lea rdi, [rip + msg]",
    "  mov rsi, 16",
    "  int 0x80",
    // syscall 0: exit(rdi=code)
    "  mov eax, 0",
    "  xor edi, edi",
    "  int 0x80",
    "msg:",
    "  .ascii \"Syscall 1 works!\\r\\n\"",
    "_user_prog_end:",
);

extern "C" {
    static _user_prog_start: u8;
    static _user_prog_end: u8;
}

fn user_prog_size() -> usize {
    unsafe {
        (&_user_prog_end as *const u8 as usize) - (&_user_prog_start as *const u8 as usize)
    }
}

pub fn init() {
    // Allocate a proper kernel stack frame for ring 3 -> ring 0 interrupts.
    let stack_idx = memory::FRAME_ALLOCATOR
        .lock()
        .allocate()
        .expect("out of memory for kernel stack");
    let stack_phys = stack_idx as u64 * 4096;
    let phys_offset = *memory::PHYS_MEM_OFFSET.lock();
    let stack_top = phys_offset + stack_phys + 4096;

    unsafe {
        TSS.privilege_stack_table[0] = VirtAddr::new(stack_top);
    }

    let mut gdt = GlobalDescriptorTable::new();
    let kernel_cs = gdt.add_entry(Descriptor::kernel_code_segment());
    let kernel_ds = gdt.add_entry(Descriptor::kernel_data_segment());
    let user_cs = gdt.add_entry(Descriptor::user_code_segment());
    let user_ds = gdt.add_entry(Descriptor::user_data_segment());
    let tss_sel = gdt.add_entry(Descriptor::tss_segment(unsafe { &*core::ptr::addr_of!(TSS) }));

    let gdt_static: &'static GlobalDescriptorTable = Box::leak(Box::new(gdt));
    gdt_static.load();

    unsafe {
        CS::set_reg(kernel_cs);
        SS::set_reg(kernel_ds);
        DS::set_reg(kernel_ds);
        ES::set_reg(kernel_ds);
        load_tss(tss_sel);
        *GDT_SEL.lock() = Some(Selectors { user_cs, user_ds });
    }

    crate::interrupts::register_int0x80();
    io::console_write(b"Userspace: GDT+TSS ready\r\n");
}

fn phys_offset() -> u64 {
    *memory::PHYS_MEM_OFFSET.lock()
}

unsafe fn phys_write64(phys: u64, val: u64) {
    let off = phys_offset();
    core::ptr::write_volatile((off + phys) as *mut u64, val);
}

unsafe fn phys_read64(phys: u64) -> u64 {
    let off = phys_offset();
    core::ptr::read_volatile((off + phys) as *const u64)
}

/// Allocate a fresh P4, clone the kernel's upper-half mappings into it, and
/// switch `CR3` to it. Returns the new P4 frame index and the (saved) kernel
/// P4 physical address.
fn switch_to_new_p4() -> (usize, u64) {
    let (kernel_p4, _) = Cr3::read();
    let kernel_p4_phys = kernel_p4.start_address().as_u64();

    let p4_idx = memory::FRAME_ALLOCATOR
        .lock()
        .allocate()
        .expect("out of memory for process P4");
    let p4_phys = p4_idx as u64 * 4096;

    unsafe {
        let off = phys_offset();
        core::ptr::write_bytes((off + p4_phys) as *mut u8, 0, 4096);
        // Clone the kernel's upper-half (entries 256..512).
        for i in 256..512 {
            let e = phys_read64(kernel_p4_phys + (i as u64) * 8);
            phys_write64(p4_phys + (i as u64) * 8, e);
        }
    }

    let frame = PhysFrame::from_start_address(PhysAddr::new(p4_phys)).unwrap();
    unsafe { Cr3::write(frame, Cr3Flags::empty()); }

    (p4_idx, kernel_p4_phys)
}

/// Map the user stack, record all process frames, set up the exit context,
/// and enter ring 3 at `entry`. Does not return.
unsafe fn enter_ring3(entry: u64, sel: &Selectors, mut code_frames: Vec<usize>, p4_idx: usize, kernel_p4_phys: u64) -> ! {
    // Map the user stack pages.
    let stack_base = USER_STACK_TOP - (USER_STACK_PAGES as u64) * 4096;
    for p in (stack_base..USER_STACK_TOP).step_by(4096) {
        loader::map_user_page(p, &mut code_frames);
    }
    // Zero the stack.
    for p in (stack_base..USER_STACK_TOP).step_by(4096) {
        core::ptr::write_bytes(p as *mut u8, 0, 4096);
    }

    // Save exit context (once): return to the shell using the TSS ring-0 stack.
    if EXIT_CTX.lock().is_none() {
        extern "C" { fn shell_main_loop(); }
        let rsp0 = TSS.privilege_stack_table[0].as_u64();
        EXIT_CTX.lock().replace(ExitContext {
            kernel_rsp: rsp0,
            kernel_ret_rip: shell_main_loop as *const () as u64,
            kernel_p4_phys,
        });
    } else if let Some(ref mut ctx) = *EXIT_CTX.lock() {
        ctx.kernel_p4_phys = kernel_p4_phys;
    }

    CURRENT_PROCESS.lock().replace(Process {
        p4_frame: p4_idx,
        frames: code_frames,
    });

    io::console_write(b"Jumping to ring 3...\r\n");

    core::arch::asm!(
        "mov {tmp}, {user_ds}",
        "or {tmp:l}, 3",
        "push {tmp}",
        "push {rsp}",
        "pushfq",
        "pop {tmp}",
        "or {tmp:l}, 0x200",
        "push {tmp}",
        "mov {tmp}, {user_cs}",
        "or {tmp:l}, 3",
        "push {tmp}",
        "push {rip}",
        "iretq",
        user_ds = in(reg) sel.user_ds.0 as u64,
        user_cs = in(reg) sel.user_cs.0 as u64,
        rsp = in(reg) USER_STACK_TOP,
        rip = in(reg) entry,
        tmp = out(reg) _,
    );
    loop {}
}

/// Run a previously loaded ELF image as a ring-3 process.
pub fn run_elf(elf: &[u8]) -> Result<(), &'static str> {
    let sel = GDT_SEL.lock();
    let sel = sel.as_ref().expect("Userspace not initialized");

    let (p4_idx, kernel_p4_phys) = switch_to_new_p4();

    let mut frames = Vec::new();
    let loaded = loader::load_elf(elf, &mut frames)?;

    unsafe { enter_ring3(loaded.entry, sel, frames, p4_idx, kernel_p4_phys) }
}

/// Run the built-in inline-asm demo in ring 3 (tests the syscall ABI).
pub fn run_user_demo() {
    let sel = GDT_SEL.lock();
    let sel = sel.as_ref().expect("Userspace not initialized");

    let (p4_idx, kernel_p4_phys) = switch_to_new_p4();

    let mut frames = Vec::new();
    unsafe {
        // Map one page for the demo code at DEMO_BASE (lower half).
        let frame = loader::map_user_page(DEMO_BASE, &mut frames);
        let off = phys_offset();
        core::ptr::write_bytes((off + frame) as *mut u8, 0, 4096);
        let size = user_prog_size();
        core::ptr::copy_nonoverlapping(
            &_user_prog_start as *const u8,
            (off + frame) as *mut u8,
            size,
        );
    }

    unsafe { enter_ring3(DEMO_BASE, sel, frames, p4_idx, kernel_p4_phys) }
}
