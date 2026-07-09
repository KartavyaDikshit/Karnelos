use alloc::boxed::Box;
use x86_64::structures::gdt::{GlobalDescriptorTable, Descriptor, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::structures::paging::PageTableFlags;
use x86_64::VirtAddr;
use x86_64::registers::segmentation::{CS, SS, DS, ES, Segment};
use x86_64::registers::control::Cr3;
use x86_64::instructions::tables::load_tss;
use spin::Mutex;
use crate::io;

pub const USER_CODE_BASE: u64 = 0x80_0040_0000;
pub const USER_STACK_BASE: u64 = 0x80_7FFF_F000;
pub const USER_STACK_SIZE: u64 = 4096;

static mut TSS: TaskStateSegment = TaskStateSegment::new();

pub struct ExitContext {
    pub kernel_rsp: u64,
    pub kernel_ret_rip: u64,
    pub kernel_cs: u64,
    pub kernel_ds: u64,
}

pub static EXIT_CTX: Mutex<Option<ExitContext>> = Mutex::new(None);

struct Selectors {
    kernel_cs: SegmentSelector,
    kernel_ds: SegmentSelector,
    user_cs: SegmentSelector,
    user_ds: SegmentSelector,
}

static GDT_SEL: Mutex<Option<Selectors>> = Mutex::new(None);

core::arch::global_asm!(
    ".section .user_prog, \"ax\"",
    ".globl _user_prog_start",
    ".globl _user_prog_end",
    "_user_prog_start:",
    // syscall 42: print "Hello from ring 3!"
    "  mov eax, 42",
    "  int 0x80",
    // syscall 1: console_write(msg, 16)
    "  mov eax, 1",
    "  lea rbx, [rip + msg]",
    "  mov ecx, 16",
    "  int 0x80",
    // syscall 0: exit
    "  mov eax, 0",
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
    // Allocate a proper kernel stack frame for ring 3→0 interrupts
    let stack_idx = crate::memory::FRAME_ALLOCATOR.lock().allocate()
        .expect("out of memory for kernel stack");
    let stack_phys = stack_idx as u64 * 4096;
    let phys_offset = *crate::memory::PHYS_MEM_OFFSET.lock();
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
        *GDT_SEL.lock() = Some(Selectors { kernel_cs, kernel_ds, user_cs, user_ds });
    }

    crate::interrupts::register_int0x80();

    io::console_write(b"Userspace: GDT+TSS ready\r\n");
}

fn alloc_frame_phys() -> u64 {
    let idx = crate::memory::FRAME_ALLOCATOR.lock().allocate()
        .expect("out of memory");
    let frame = idx as u64 * 4096;
    let phys_offset = *crate::memory::PHYS_MEM_OFFSET.lock();
    unsafe {
        core::ptr::write_bytes((phys_offset + frame) as *mut u8, 0, 4096);
    }
    frame
}

unsafe fn phys_write64(phys: u64, val: u64) {
    let phys_offset = *crate::memory::PHYS_MEM_OFFSET.lock();
    core::ptr::write_volatile((phys_offset + phys) as *mut u64, val);
}

fn map_user_pages() {
    let (p4_frame, _) = unsafe { Cr3::read() };
    let p4_phys = p4_frame.start_address().as_u64();

    unsafe {
        let p3_phys = alloc_frame_phys();
        phys_write64(p4_phys + (1 * 8), p3_phys | 0x7);

        let p2_phys = alloc_frame_phys();
        phys_write64(p3_phys + (0 * 8), p2_phys | 0x7);

        let code_idx = crate::memory::FRAME_ALLOCATOR.lock().allocate().expect("code frame");
        let code_phys = code_idx as u64 * 4096;

        let p1_code_phys = alloc_frame_phys();
        phys_write64(p2_phys + (2 * 8), p1_code_phys | 0x7);
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;
        phys_write64(p1_code_phys + (0 * 8), code_phys | flags.bits());

        let prog_size = user_prog_size();
        if prog_size > 4096 {
            io::console_write(b"User program too large!\r\n");
            return;
        }
        let phys_offset = *crate::memory::PHYS_MEM_OFFSET.lock();
        core::ptr::copy_nonoverlapping(
            &_user_prog_start as *const u8,
            (phys_offset + code_phys) as *mut u8,
            prog_size,
        );

        let stack_idx = crate::memory::FRAME_ALLOCATOR.lock().allocate().expect("stack frame");
        let stack_phys = stack_idx as u64 * 4096;

        let p1_stack_phys = alloc_frame_phys();
        phys_write64(p2_phys + (511 * 8), p1_stack_phys | 0x7);
        phys_write64(p1_stack_phys + (511 * 8), stack_phys | flags.bits());

        core::ptr::write_bytes((phys_offset + stack_phys) as *mut u8, 0, USER_STACK_SIZE as usize);

        core::arch::asm!("mfence", options(nostack));
        let (p4_frame, flags) = Cr3::read();
        Cr3::write(p4_frame, flags);
    }
}

pub fn run_user_prog() {
    let sel = GDT_SEL.lock();
    let sel = sel.as_ref().expect("Userspace not initialized");

    map_user_pages();

    let user_rsp = USER_STACK_BASE + USER_STACK_SIZE;

    // Save exit context once: jump to shell_main_loop with the TSS interrupt stack.
    // This abandons the current kernel call stack but preserves all global state.
    if EXIT_CTX.lock().is_none() {
        extern "C" { fn shell_main_loop(); }
        let rsp0 = unsafe { TSS.privilege_stack_table[0].as_u64() };
        *EXIT_CTX.lock() = Some(ExitContext {
            kernel_rsp: rsp0,
            kernel_ret_rip: shell_main_loop as *const () as u64,
            kernel_cs: sel.kernel_cs.0 as u64,
            kernel_ds: sel.kernel_ds.0 as u64,
        });
    }

    io::console_write(b"Jumping to ring 3...\r\n");

    unsafe {
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
            rsp = in(reg) user_rsp,
            rip = in(reg) USER_CODE_BASE,
            tmp = out(reg) _,
        );
    }
}
