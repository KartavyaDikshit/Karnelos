//! Userspace syscall shim for Karnelos.
//!
//! Stable ABI (matches the kernel `int 0x80` handler):
//!   rax = syscall number
//!   rdi, rsi, rdx, r10, r8, r9 = up to 6 arguments
//!   return value in rax
//!
//! Syscall numbers:
//!   0  exit(code)
//!   1  write(buf_ptr, len) -> bytes written
//!   2  read(buf_ptr, len)  -> bytes read
//!   3  exit (alias, kept for compatibility)
//!   4  storage_read(name_ptr, buf_ptr, len) -> bytes read
//!   5  storage_write(name_ptr, buf_ptr, len) -> bytes written
//!   6  getchar() -> char or 0 if none

use core::arch::asm;

/// Raw syscall: pass all six argument registers explicitly.
#[inline(always)]
pub unsafe fn syscall_raw(
    num: u64,
    a: u64,
    b: u64,
    c: u64,
    d: u64,
    e: u64,
    f: u64,
) -> u64 {
    let ret: u64;
    asm!(
        "int 0x80",
        in("rax") num,
        in("rdi") a,
        in("rsi") b,
        in("rdx") c,
        in("r10") d,
        in("r8") e,
        in("r9") f,
        lateout("rax") ret,
        options(nomem, nostack),
    );
    ret
}

/// Ergonomic syscall macro: `syscall!(NUM, arg1, arg2, ...)`.
#[macro_export]
macro_rules! syscall {
    ($n:expr $(,)?) => {
        $crate::syscall::syscall_raw($n as u64, 0, 0, 0, 0, 0, 0)
    };
    ($n:expr, $a:expr $(,)?) => {
        $crate::syscall::syscall_raw($n as u64, $a as u64, 0, 0, 0, 0, 0)
    };
    ($n:expr, $a:expr, $b:expr $(,)?) => {
        $crate::syscall::syscall_raw($n as u64, $a as u64, $b as u64, 0, 0, 0, 0)
    };
    ($n:expr, $a:expr, $b:expr, $c:expr $(,)?) => {
        $crate::syscall::syscall_raw($n as u64, $a as u64, $b as u64, $c as u64, 0, 0, 0)
    };
    ($n:expr, $a:expr, $b:expr, $c:expr, $d:expr $(,)?) => {
        $crate::syscall::syscall_raw($n as u64, $a as u64, $b as u64, $c as u64, $d as u64, 0, 0)
    };
    ($n:expr, $a:expr, $b:expr, $c:expr, $d:expr, $e:expr $(,)?) => {
        $crate::syscall::syscall_raw($n as u64, $a as u64, $b as u64, $c as u64, $d as u64, $e as u64, 0)
    };
    ($n:expr, $a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr $(,)?) => {
        $crate::syscall::syscall_raw($n as u64, $a as u64, $b as u64, $c as u64, $d as u64, $e as u64, $f as u64)
    };
}

/// Write a byte string to the console (syscall 1).
#[inline(always)]
pub fn write_bytes(s: &[u8]) -> u64 {
    unsafe { syscall_raw(1, s.as_ptr() as u64, s.len() as u64, 0, 0, 0, 0) }
}

/// Write a null-terminated / fixed string slice.
#[inline(always)]
pub fn print(s: &str) -> u64 {
    write_bytes(s.as_bytes())
}

/// Exit the process with the given code (syscall 0).
#[inline(always)]
pub fn exit(code: u64) -> ! {
    unsafe {
        syscall_raw(0, code, 0, 0, 0, 0, 0);
    }
    loop {}
}
