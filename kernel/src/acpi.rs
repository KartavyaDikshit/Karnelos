use x86_64::PhysAddr;
use crate::memory;

pub fn find_rsdp(boot_rsdp: Option<u64>) -> Option<u64> {
    if let Some(addr) = boot_rsdp {
        return Some(addr);
    }
    unsafe {
        let ebda_seg = *(memory::phys_to_virt(PhysAddr::new(0x40E)).as_ptr::<u16>()) as u32;
        let ebda = (ebda_seg as u64) << 4;
        if ebda > 0 {
            for off in (0..1024u64).step_by(16) {
                let ptr = memory::phys_to_virt(PhysAddr::new(ebda + off));
                if *(ptr.as_ptr::<[u8; 8]>()) == *b"RSD PTR " {
                    return Some(ebda + off);
                }
            }
        }
    }
    for phys in (0xE0000u64..0xFFFFF).step_by(16) {
        let ptr = memory::phys_to_virt(PhysAddr::new(phys));
        unsafe {
            if *(ptr.as_ptr::<[u8; 8]>()) == *b"RSD PTR " {
                return Some(phys);
            }
        }
    }
    None
}

pub unsafe fn find_madt(boot_rsdp: Option<u64>) -> Option<u64> {
    let rsdp_phys = find_rsdp(boot_rsdp)?;
    let rsdp = memory::phys_to_virt(PhysAddr::new(rsdp_phys));
    let rev = *(rsdp.as_ptr::<u8>().add(15));
    let sdt_phys = if rev >= 2 {
        let xsdt = *(rsdp.as_ptr::<u64>().add(3));
        if xsdt == 0 { return None; }
        xsdt
    } else {
        let rsdt = *(rsdp.as_ptr::<u32>().add(4)) as u64;
        if rsdt == 0 { return None; }
        rsdt
    };
    let sdt_len = read_u32(sdt_phys + 4);
    if sdt_len < 36 { return None; }
    let count = ((sdt_len - 36) / 4) as usize;
    for i in 0..count {
        let entry = sdt_phys + 36 + (i * 4) as u64;
        let tbl = read_u32(entry) as u64;
        if tbl == 0 { continue; }
        let sig = read_sig(tbl);
        if &sig == b"APIC" {
            return Some(tbl);
        }
    }
    None
}

unsafe fn read_u32(phys: u64) -> u32 {
    let ptr = memory::phys_to_virt(PhysAddr::new(phys));
    ptr.as_ptr::<u32>().read_volatile()
}

unsafe fn read_sig(phys: u64) -> [u8; 4] {
    let ptr = memory::phys_to_virt(PhysAddr::new(phys));
    ptr.as_ptr::<[u8; 4]>().read_volatile()
}

pub unsafe fn parse_madt(madt_phys: u64) -> (u64, u64, alloc::vec::Vec<u8>) {
    let madt = memory::phys_to_virt(PhysAddr::new(madt_phys));
    let lapic_addr = *(madt.as_ptr::<u32>().add(4)) as u64;
    let len = *(madt.as_ptr::<u32>().add(1));
    let mut cpus = alloc::vec::Vec::new();
    let mut ioapic_addr = 0u64;
    let mut off = 44u64;
    while (off as u32) < len {
        let entry = madt.as_ptr::<u8>().add(off as usize);
        let typ = *entry;
        let elen = *entry.add(1);
        if typ == 0 {
            let apic_id = *entry.add(3);
            let flags = *(entry.add(4) as *const u32);
            if flags & 1 != 0 {
                cpus.push(apic_id);
            }
        } else if typ == 1 {
            ioapic_addr = *(entry.add(4) as *const u32) as u64;
        }
        off += elen as u64;
    }
    (lapic_addr, ioapic_addr, cpus)
}
