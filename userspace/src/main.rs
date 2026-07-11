#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
extern crate alloc;
mod rt;
mod syscall;
use syscall::*;

#[no_mangle]
pub extern "C" fn main() -> i32 {
    // >>> KARNELOS_BODY_START
    // Generated app code goes here.
    print("Hello from a generated ring-3 app!\r\n");

    let mut v: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
    for i in 1..=5u8 {
        v.push(b'0' + i);
    }
    print("counted: ");
    print(unsafe { core::str::from_utf8_unchecked(&v) });
    print("\r\n");

    exit(0);
    // >>> KARNELOS_BODY_END
    0
}
