// ATA PIO block driver over the IDE controller (secondary channel, master).
// QEMU exposes a raw disk via `-drive if=ide,index=2` as the secondary
// master (ports 0x170-0x177, control 0x376). PIO mode is fully
// polled (no interrupts needed) and is reliable across QEMU versions.
//
// This replaces the planned virtio-blk path because QEMU 11 dropped the
// legacy virtio queue interface; ATA PIO gives us the same block-level
// abstraction the filesystem needs.

use crate::io;
use crate::memory;
use spin::Mutex;

const BASE: u16 = 0x170; // secondary IDE channel
const DEV_SEL: u8 = 0xE0; // LBA | master

// Status register bits
const BSY: u8 = 0x80;
const DRQ: u8 = 0x08;
const ERR: u8 = 0x01;

const CMD_READ: u8 = 0x20;
const CMD_WRITE: u8 = 0x30;
const CMD_IDENTIFY: u8 = 0xEC;
const CMD_FLUSH: u8 = 0xE7;

struct Ata {
    present: bool,
    capacity_sectors: u64,
}

static DISK: Mutex<Ata> = Mutex::new(Ata { present: false, capacity_sectors: 0 });

fn status() -> u8 {
    io::inb(BASE + 7)
}

fn wait_not_bsy() -> bool {
    for _ in 0..10_000_000 {
        if status() & BSY == 0 {
            return true;
        }
    }
    false
}

fn wait_drq() -> bool {
    for _ in 0..10_000_000 {
        let s = status();
        if s & BSY == 0 && s & DRQ != 0 {
            return true;
        }
        if s & ERR != 0 {
            return false;
        }
    }
    false
}

fn select(lba: u64) {
    // dev/head: 0xE0 | (lba bits 24..27)
    io::outb(BASE + 6, DEV_SEL | ((lba >> 24) & 0x0F) as u8);
}

pub fn init() {
    // Probe: select master, issue IDENTIFY, see if it responds.
    if !wait_not_bsy() {
        io::debug_write(b"ata: BSY stuck\n");
        return;
    }
    select(0);
    io::outb(BASE + 2, 0); // sector count
    io::outb(BASE + 3, 0);
    io::outb(BASE + 4, 0);
    io::outb(BASE + 5, 0);
    io::outb(BASE + 7, CMD_IDENTIFY);

    // If no drive, status stays 0 or ERR is set quickly.
    let mut found = false;
    for _ in 0..1_000_000 {
        let s = status();
        if s & BSY == 0 {
            if s & DRQ != 0 {
                found = true;
            }
            break;
        }
    }
    if !found {
        io::console_write(b"ata: no secondary-master drive detected\r\n");
        return;
    }

    // Read 256 words of IDENTIFY data
    let mut buf = [0u16; 256];
    for w in buf.iter_mut() {
        *w = io::inw(BASE + 0);
    }
    // Words 60-61 (0-indexed) hold 28-bit total LBA sectors
    let sectors = (buf[61] as u32) << 16 | (buf[60] as u32);
    let capacity = sectors as u64;

    io::console_write(b"ata: drive ready, capacity=");
    memory::dec(capacity as usize);
    io::console_write(b" sectors\r\n");

    let mut d = DISK.lock();
    d.present = true;
    d.capacity_sectors = capacity;
}

fn read_sector(lba: u64, out: &mut [u8]) {
    if !wait_not_bsy() {
        return;
    }
    select(lba);
    io::outb(BASE + 2, 1); // 1 sector
    io::outb(BASE + 3, (lba & 0xFF) as u8);
    io::outb(BASE + 4, ((lba >> 8) & 0xFF) as u8);
    io::outb(BASE + 5, ((lba >> 16) & 0xFF) as u8);
    io::outb(BASE + 7, CMD_READ);

    if !wait_drq() {
        io::debug_write(b"ata: read error\n");
        return;
    }
    unsafe {
        let ptr = out.as_mut_ptr() as *mut u8;
        for i in 0..256 {
            let word = io::inw(BASE + 0);
            *ptr.add(i * 2) = (word & 0xFF) as u8;
            *ptr.add(i * 2 + 1) = ((word >> 8) & 0xFF) as u8;
        }
    }
    // Wait for command completion
    for _ in 0..10_000_000 {
        if status() & BSY == 0 {
            break;
        }
    }
}

fn write_sector(lba: u64, data: &[u8]) {
    if !wait_not_bsy() {
        return;
    }
    select(lba);
    io::outb(BASE + 2, 1);
    io::outb(BASE + 3, (lba & 0xFF) as u8);
    io::outb(BASE + 4, ((lba >> 8) & 0xFF) as u8);
    io::outb(BASE + 5, ((lba >> 16) & 0xFF) as u8);
    io::outb(BASE + 7, CMD_WRITE);

    if !wait_drq() {
        io::debug_write(b"ata: write error\n");
        return;
    }
    unsafe {
        let ptr = data.as_ptr() as *const u8;
        for i in 0..256 {
            let lo = *ptr.add(i * 2) as u16;
            let hi = (*ptr.add(i * 2 + 1) as u16) << 8;
            io::outw(BASE + 0, lo | hi);
        }
    }
    // Flush and wait for completion
    io::outb(BASE + 7, CMD_FLUSH);
    for _ in 0..10_000_000 {
        if status() & BSY == 0 {
            break;
        }
    }
}

pub fn read_block(sector: u64, buf: &mut [u8]) {
    let d = DISK.lock();
    if !d.present {
        return;
    }
    drop(d);
    read_sector(sector, buf);
}

pub fn write_block(sector: u64, buf: &[u8]) {
    let d = DISK.lock();
    if !d.present {
        return;
    }
    drop(d);
    write_sector(sector, buf);
}

pub fn is_present() -> bool {
    DISK.lock().present
}

pub fn capacity_sectors() -> u64 {
    DISK.lock().capacity_sectors
}
