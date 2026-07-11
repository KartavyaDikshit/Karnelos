//! ELF64 loader for ring-3 apps.
//!
//! Parses a static-PIE (`ET_DYN`) x86-64 ELF, maps its `PT_LOAD` segments into
//! the *currently active* page tables, and applies `R_X86_64_RELATIVE`
//! relocations. Because the image is mapped 1:1 at its link-time virtual
//! addresses (base 0), each `R_X86_64_RELATIVE` relocation simply stores its
//! addend. Any other relocation type is rejected (we have no dynamic linker).

use x86_64::registers::control::Cr3;
use crate::io;
use crate::memory;

// --- ELF constants ---
const ET_EXEC: u16 = 2;
const ET_DYN: u16 = 3;
const PT_LOAD: u32 = 1;
const SHT_RELA: u32 = 4;
const R_X86_64_RELATIVE: u32 = 8;

const PAGE_FLAGS: u64 = 0x7; // PRESENT | WRITABLE | USER_ACCESSIBLE

fn rd_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn rd_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn rd_u64(b: &[u8], o: usize) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[o..o + 8]);
    u64::from_le_bytes(a)
}

fn phys_offset() -> u64 {
    *memory::PHYS_MEM_OFFSET.lock()
}

fn alloc_frame() -> usize {
    memory::FRAME_ALLOCATOR
        .lock()
        .allocate()
        .expect("out of memory for user page")
}

unsafe fn phys_write64(phys: u64, val: u64) {
    let off = phys_offset();
    core::ptr::write_volatile((off + phys) as *mut u64, val);
}

unsafe fn phys_read64(phys: u64) -> u64 {
    let off = phys_offset();
    core::ptr::read_volatile((off + phys) as *const u64)
}

/// Map a single virtual page (allocating intermediate page tables as needed)
/// to a freshly allocated frame. Records every allocated frame index in
/// `frames` so the caller can free them on process exit. Returns the physical
/// address of the mapped leaf frame.
pub unsafe fn map_user_page(vaddr: u64, frames: &mut alloc::vec::Vec<usize>) -> u64 {
    let (p4_frame, _) = Cr3::read();
    let mut table_phys = p4_frame.start_address().as_u64();

    let idx = [
        (vaddr >> 39) & 0x1ff,
        (vaddr >> 30) & 0x1ff,
        (vaddr >> 21) & 0x1ff,
        (vaddr >> 12) & 0x1ff,
    ];

    // Walk P4 -> P3 -> P2, allocating missing intermediate tables.
    for lvl in 0..3 {
        let entry_phys = table_phys + idx[lvl] * 8;
        let cur = phys_read64(entry_phys);
        if cur & 1 == 0 {
            let fi = alloc_frame();
            let next = fi as u64 * 4096;
            frames.push(fi);
            phys_write64(entry_phys, next | PAGE_FLAGS);
            table_phys = next;
        } else {
            table_phys = cur & !0xfff;
        }
    }

    // Leaf (P1): map a fresh frame.
    let entry_phys = table_phys + idx[3] * 8;
    let fi = alloc_frame();
    let frame = fi as u64 * 4096;
    frames.push(fi);
    phys_write64(entry_phys, frame | PAGE_FLAGS);
    frame
}

#[derive(Debug)]
pub struct LoadedElf {
    pub entry: u64,
}

/// Load an ELF image into the active page tables. Records every allocated
/// frame in `frames` for later cleanup. Returns the entry point.
pub fn load_elf(elf: &[u8], frames: &mut alloc::vec::Vec<usize>) -> Result<LoadedElf, &'static str> {
    if elf.len() < 64 {
        return Err("ELF too small");
    }
    if &elf[0..4] != b"\x7fELF" {
        return Err("not an ELF file");
    }
    if elf[4] != 2 {
        return Err("not 64-bit ELF");
    }
    if elf[5] != 1 {
        return Err("not little-endian ELF");
    }
    let e_type = rd_u16(elf, 16);
    if e_type != ET_EXEC && e_type != ET_DYN {
        return Err("not an executable/dynamic ELF");
    }
    if rd_u16(elf, 18) != 0x3E {
        return Err("not x86-64 ELF");
    }

    let e_entry = rd_u64(elf, 24);
    let e_phoff = rd_u64(elf, 32);
    let e_phentsize = rd_u16(elf, 54) as usize;
    let e_phnum = rd_u16(elf, 56) as usize;
    let e_shoff = rd_u64(elf, 40);
    let e_shentsize = rd_u16(elf, 58) as usize;
    let e_shnum = rd_u16(elf, 60) as usize;

    if e_phentsize < 56 || e_phnum == 0 {
        return Err("bad program header table");
    }

    // Map each PT_LOAD segment.
    for i in 0..e_phnum {
        let ph = e_phoff as usize + i * e_phentsize;
        let p_type = rd_u32(elf, ph);
        if p_type != PT_LOAD {
            continue;
        }
        let p_offset = rd_u64(elf, ph + 8);
        let p_vaddr = rd_u64(elf, ph + 16);
        let p_filesz = rd_u64(elf, ph + 32);
        let p_memsz = rd_u64(elf, ph + 40);

        if p_vaddr == 0 {
            return Err("PT_LOAD with zero vaddr");
        }

        let start = p_vaddr;
        let end = p_vaddr + p_memsz;
        let mut v = start;
        while v < end {
            unsafe {
                // Map the page (allocates a freshly allocated frame).
                let frame = map_user_page(v, frames);
                // Zero the whole frame, then copy in the file bytes that belong here.
                let off = phys_offset();
                core::ptr::write_bytes((off + frame) as *mut u8, 0, 4096);
                let page_off = v - start; // offset within the segment
                let file_remaining = p_filesz.saturating_sub(page_off);
                let chunk = (4096 - (page_off % 4096) as usize).min(file_remaining as usize);
                if chunk > 0 {
                    let file_pos = (p_offset + page_off) as usize;
                    let src = &elf[file_pos..file_pos + chunk];
                    core::ptr::copy_nonoverlapping(
                        src.as_ptr(),
                        (off + frame) as *mut u8,
                        chunk,
                    );
                }
            }
            v += 4096;
        }
    }

    // Apply relocations from SHT_RELA sections.
    if e_shoff != 0 && e_shnum > 0 {
        for i in 0..e_shnum {
            let sh = e_shoff as usize + i * e_shentsize;
            let sh_type = rd_u32(elf, sh + 4);
            if sh_type != SHT_RELA {
                continue;
            }
            let sh_offset = rd_u64(elf, sh + 24);
            let sh_size = rd_u64(elf, sh + 32);
            let sh_entsize = rd_u64(elf, sh + 56);
            if sh_entsize < 24 {
                return Err("bad RELA entsize");
            }
            let count = sh_size / sh_entsize;
            for j in 0..count {
                let re = sh_offset as usize + j as usize * sh_entsize as usize;
                let r_offset = rd_u64(elf, re);
                let r_info = rd_u64(elf, re + 8);
                let r_addend = rd_u64(elf, re + 16);
                let r_type = (r_info & 0xffff_ffff) as u32;
                match r_type {
                    R_X86_64_RELATIVE => {
                        // *(r_offset) = base(0) + r_addend
                        unsafe {
                            let off = phys_offset();
                            let ptr = (off + r_offset) as *mut u64;
                            core::ptr::write_volatile(ptr, r_addend);
                        }
                    }
                    other => {
                        io::console_write(b"loader: unsupported relocation type ");
                        let mut b = [0u8; 12];
                        let mut n = other;
                        let mut k = 12;
                        while n > 0 {
                            k -= 1;
                            b[k] = b'0' + (n % 10) as u8;
                            n /= 10;
                        }
                        io::console_write(&b[k..]);
                        io::console_write(b"\r\n");
                        return Err("unsupported relocation");
                    }
                }
            }
        }
    }

    if e_entry == 0 {
        return Err("zero entry point");
    }

    Ok(LoadedElf { entry: e_entry })
}
