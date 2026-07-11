//! Minimal runtime for Karnelos ring-3 apps.
//!
//! Provides the `_start` entry point (zeroes BSS, calls `main`, exits),
//! a panic handler, and a bump allocator so generated apps can use `alloc`.

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

/// Virtual address of the user heap region (mapped RW by the kernel loader).
pub const USER_HEAP_BASE: usize = 0x10_0000;
/// Size of the user heap region (1 MiB).
pub const USER_HEAP_SIZE: usize = 0x10_0000;

extern "C" {
    static mut __bss_start: u8;
    static mut __bss_end: u8;
}

// --- Compiler-builtin intrinsics (freestanding: no libc provides these) ---

#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0;
    while i < n {
        *dest.add(i) = *src.add(i);
        i += 1;
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    let c = c as u8;
    let mut i = 0;
    while i < n {
        *s.add(i) = c;
        i += 1;
    }
    s
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    let mut i = 0;
    while i < n {
        let x = *a.add(i);
        let y = *b.add(i);
        if x != y {
            return (x as i32) - (y as i32);
        }
        i += 1;
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if (dest as usize) < (src as usize) {
        let mut i = 0;
        while i < n {
            *dest.add(i) = *src.add(i);
            i += 1;
        }
    } else {
        let mut i = n;
        while i > 0 {
            i -= 1;
            *dest.add(i) = *src.add(i);
        }
    }
    dest
}

/// Bump-pointer allocator over the kernel-mapped heap region.
struct BumpAlloc {
    brk: AtomicUsize,
}

unsafe impl GlobalAlloc for BumpAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size().max(1);
        let align = layout.align().max(1);
        let base = self.brk.load(Ordering::Relaxed);
        let addr = (base + align - 1) & !(align - 1);
        let new_brk = addr + size;
        if new_brk > USER_HEAP_BASE + USER_HEAP_SIZE {
            return core::ptr::null_mut();
        }
        self.brk.store(new_brk, Ordering::Relaxed);
        addr as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator: no frees.
    }
}

#[global_allocator]
static ALLOC: BumpAlloc = BumpAlloc {
    brk: AtomicUsize::new(USER_HEAP_BASE),
};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe {
        let mut p = core::ptr::addr_of_mut!(__bss_start);
        let end = core::ptr::addr_of!(__bss_end) as usize;
        while (p as usize) < end {
            *p = 0;
            p = p.add(1);
        }
    }

    let code = crate::main();
    crate::syscall::exit(code as u64);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    crate::syscall::exit(1);
}

#[alloc_error_handler]
fn alloc_error(_layout: Layout) -> ! {
    crate::syscall::exit(2);
}
