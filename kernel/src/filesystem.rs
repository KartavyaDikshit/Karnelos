// Minimal flat filesystem on top of the virtio-blk block device.
// Disk layout (512-byte sectors):
//   Sector 0            : superblock (magic + geometry)
//   Sectors 1..9        : directory (64 entries x 64 bytes)
//   Sectors 9..41       : block bitmap (1 bit per data sector)
//   Sectors 41..        : file data
//
// This is a prototype "AI-native" storage layer: the LLM can later generate
// custom binary formats and richer filesystems; this one just proves persistence.

use crate::io;
use crate::memory;
use crate::ata;

pub const SECTOR_SIZE: usize = 512;
const MAGIC: [u8; 4] = *b"KRNL";

const DIR_START: u32 = 1;
const DIR_SECTORS: u32 = 8;
const DIR_ENTRY_SIZE: usize = 64;
const DIR_ENTRIES: usize = (DIR_SECTORS as usize * SECTOR_SIZE) / DIR_ENTRY_SIZE; // 64
const BITMAP_START: u32 = 9;
const BITMAP_SECTORS: u32 = 32;

fn data_start() -> u32 {
    BITMAP_START + BITMAP_SECTORS
}

fn rd_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

fn wr_u32(buf: &mut [u8], off: usize, val: u32) {
    let b = val.to_le_bytes();
    buf[off] = b[0];
    buf[off + 1] = b[1];
    buf[off + 2] = b[2];
    buf[off + 3] = b[3];
}

fn bitmap_sector_for(idx: u32) -> u32 {
    BITMAP_START + (idx / (SECTOR_SIZE as u32 * 8))
}

pub fn set_bootmode_ephemeral() {
    if !is_formatted() {
        format();
    }
    let mut sb = [0u8; SECTOR_SIZE];
    ata::read_block(0, &mut sb);
    sb[32] = 1; // ephemeral flag
    ata::write_block(0, &sb);
    io::console_write(b"bootmode: ephemeral enabled (next boot will reformat)\r\n");
}

pub fn check_bootmode_ephemeral() -> bool {
    if !is_formatted() {
        return false;
    }
    let mut sb = [0u8; SECTOR_SIZE];
    ata::read_block(0, &mut sb);
    sb[32] == 1
}

pub fn clear_bootmode_ephemeral() {
    let mut sb = [0u8; SECTOR_SIZE];
    ata::read_block(0, &mut sb);
    sb[32] = 0;
    ata::write_block(0, &sb);
}

pub fn is_formatted() -> bool {
    if !ata::is_present() {
        return false;
    }
    let mut sb = [0u8; SECTOR_SIZE];
    ata::read_block(0, &mut sb);
    sb[0..4] == MAGIC
}

pub fn format() {
    if !ata::is_present() {
        io::console_write(b"storage: no block device present\r\n");
        return;
    }
    let mut sb = [0u8; SECTOR_SIZE];
    sb[0..4].copy_from_slice(&MAGIC);
    wr_u32(&mut sb, 4, 1); // version
    wr_u32(&mut sb, 8, ata::capacity_sectors() as u32);
    wr_u32(&mut sb, 12, DIR_START);
    wr_u32(&mut sb, 16, DIR_SECTORS);
    wr_u32(&mut sb, 20, BITMAP_START);
    wr_u32(&mut sb, 24, BITMAP_SECTORS);
    wr_u32(&mut sb, 28, data_start());
    ata::write_block(0, &sb);

    let zero = [0u8; SECTOR_SIZE];
    for s in DIR_START..DIR_START + DIR_SECTORS {
        ata::write_block(s as u64, &zero);
    }
    for s in BITMAP_START..BITMAP_START + BITMAP_SECTORS {
        ata::write_block(s as u64, &zero);
    }
    io::console_write(b"storage: disk formatted (");
    memory::dec(ata::capacity_sectors() as usize);
    io::console_write(b" sectors, ");
    memory::dec((ata::capacity_sectors() - data_start() as u64) as usize);
    io::console_write(b" free for data)\r\n");
}

fn bitmap_bit(idx: u32) -> bool {
    let sec = bitmap_sector_for(idx);
    let mut buf = [0u8; SECTOR_SIZE];
    ata::read_block(sec as u64, &mut buf);
    let off = (idx % (SECTOR_SIZE as u32 * 8)) as usize;
    buf[off / 8] & (1 << (off % 8)) != 0
}

fn bitmap_set(idx: u32) {
    let sec = bitmap_sector_for(idx);
    let mut buf = [0u8; SECTOR_SIZE];
    ata::read_block(sec as u64, &mut buf);
    let off = (idx % (SECTOR_SIZE as u32 * 8)) as usize;
    buf[off / 8] |= 1 << (off % 8);
    ata::write_block(sec as u64, &buf);
}

fn bitmap_clear(idx: u32) {
    let sec = bitmap_sector_for(idx);
    let mut buf = [0u8; SECTOR_SIZE];
    ata::read_block(sec as u64, &mut buf);
    let off = (idx % (SECTOR_SIZE as u32 * 8)) as usize;
    buf[off / 8] &= !(1 << (off % 8));
    ata::write_block(sec as u64, &buf);
}

fn alloc_contiguous(count: u32) -> Option<u32> {
    let total = ata::capacity_sectors() as u32 - data_start();
    if total < count {
        return None;
    }
    let mut run_start = 0u32;
    let mut run = 0u32;
    for i in 0..total {
        if !bitmap_bit(i) {
            if run == 0 {
                run_start = i;
            }
            run += 1;
            if run == count {
                for j in run_start..run_start + count {
                    bitmap_set(j);
                }
                return Some(run_start);
            }
        } else {
            run = 0;
        }
    }
    None
}

pub fn read_dir_raw() -> [u8; DIR_SECTORS as usize * SECTOR_SIZE] {
    read_dir()
}

pub fn rd_u32_from_dir(dir: &[u8], off: usize) -> u32 {
    rd_u32(dir, off)
}

fn read_dir() -> [u8; DIR_SECTORS as usize * SECTOR_SIZE] {
    let mut dir = [0u8; DIR_SECTORS as usize * SECTOR_SIZE];
    for s in 0..DIR_SECTORS {
        ata::read_block(
            (DIR_START + s) as u64,
            &mut dir[s as usize * SECTOR_SIZE..],
        );
    }
    dir
}

fn write_dir(dir: &[u8]) {
    for s in 0..DIR_SECTORS {
        ata::write_block((DIR_START + s) as u64, &dir[s as usize * SECTOR_SIZE..]);
    }
}

pub fn write_file(name: &[u8], data: &[u8]) {
    if !is_formatted() {
        format();
    }
    let mut dir = read_dir();
    let name_len = name.len().min(56);

    let mut entry = None;
    for i in 0..DIR_ENTRIES {
        let off = i * DIR_ENTRY_SIZE;
        if dir[off] != 0 && &dir[off..off + name_len] == name {
            // overwrite: free old data blocks first
            let old_start = rd_u32(&dir, off + 60);
            let old_size = rd_u32(&dir, off + 56);
            let old_sects = (old_size + SECTOR_SIZE as u32 - 1) / SECTOR_SIZE as u32;
            for j in 0..old_sects {
                bitmap_clear(old_start + j);
            }
            entry = Some(i);
            break;
        }
        if dir[off] == 0 && entry.is_none() {
            entry = Some(i);
        }
    }

    let ei = match entry {
        Some(e) => e,
        None => {
            io::console_write(b"storage: directory full\r\n");
            return;
        }
    };

    let needed = (data.len() as u32 + SECTOR_SIZE as u32 - 1) / SECTOR_SIZE as u32;
    let start = match alloc_contiguous(needed.max(1)) {
        Some(s) => s,
        None => {
            io::console_write(b"storage: out of space\r\n");
            return;
        }
    };

    for i in 0..needed {
        let mut sec_buf = [0u8; SECTOR_SIZE];
        let chunk_start = (i as usize) * SECTOR_SIZE;
        let end = ((i as usize) + 1) * SECTOR_SIZE;
        let end = end.min(data.len());
        sec_buf[..end - chunk_start].copy_from_slice(&data[chunk_start..end]);
        ata::write_block((data_start() + start + i) as u64, &sec_buf);
    }

    let off = ei * DIR_ENTRY_SIZE;
    for i in 0..56 {
        dir[off + i] = 0;
    }
    dir[off..off + name_len].copy_from_slice(&name[..name_len]);
    wr_u32(&mut dir, off + 56, data.len() as u32);
    wr_u32(&mut dir, off + 60, start);
    write_dir(&dir);

    io::console_write(b"storage: wrote '");
    io::console_write(name);
    io::console_write(b"' (");
    memory::dec(data.len());
    io::console_write(b" bytes)\r\n");
}

pub fn read_file(name: &[u8], out: &mut [u8]) -> usize {
    if !is_formatted() {
        return 0;
    }
    let dir = read_dir();
    let name_len = name.len().min(56);
    for i in 0..DIR_ENTRIES {
        let off = i * DIR_ENTRY_SIZE;
        if dir[off] != 0 && &dir[off..off + name_len] == name {
            let size = rd_u32(&dir, off + 56) as usize;
            let start = rd_u32(&dir, off + 60);
            let n = size.min(out.len());
            let sects = (size + SECTOR_SIZE - 1) / SECTOR_SIZE;
            for s in 0..sects {
                let mut buf = [0u8; SECTOR_SIZE];
                ata::read_block((data_start() + start + s as u32) as u64, &mut buf);
                let copy = ((s * SECTOR_SIZE + SECTOR_SIZE).min(size)) - s * SECTOR_SIZE;
                out[s * SECTOR_SIZE..s * SECTOR_SIZE + copy]
                    .copy_from_slice(&buf[..copy]);
            }
            return n;
        }
    }
    0
}

pub fn delete_file(name: &[u8]) -> bool {
    if !is_formatted() {
        return false;
    }
    let mut dir = read_dir();
    let name_len = name.len().min(56);
    for i in 0..DIR_ENTRIES {
        let off = i * DIR_ENTRY_SIZE;
        if dir[off] != 0 && &dir[off..off + name_len] == name {
            let old_start = rd_u32(&dir, off + 60);
            let old_size = rd_u32(&dir, off + 56);
            let old_sects = (old_size + SECTOR_SIZE as u32 - 1) / SECTOR_SIZE as u32;
            for j in 0..old_sects {
                bitmap_clear(old_start + j);
            }
            for k in 0..56 { dir[off + k] = 0; }
            write_dir(&dir);
            return true;
        }
    }
    false
}

pub fn list() {
    if !is_formatted() {
        io::console_write(b"storage: not formatted (run 'storage format')\r\n");
        return;
    }
    let dir = read_dir();
    io::console_write(b"\r\nFiles on persistent storage:\r\n");
    let mut count = 0;
    for i in 0..DIR_ENTRIES {
        let off = i * DIR_ENTRY_SIZE;
        if dir[off] != 0 {
            count += 1;
            let name_end = dir[off..off + 56].iter().position(|&b| b == 0).unwrap_or(56);
            io::console_write(b"  ");
            io::console_write(&dir[off..off + name_end]);
            io::console_write(b"  (");
            memory::dec(rd_u32(&dir, off + 56) as usize);
            io::console_write(b" bytes)\r\n");
        }
    }
    if count == 0 {
        io::console_write(b"  (empty)\r\n");
    }
}

pub fn info() {
    io::console_write(b"\r\nStorage info:\r\n");
    if !ata::is_present() {
        io::console_write(b"  No block device detected\r\n");
        return;
    }
    io::console_write(b"  Device: virtio-blk (legacy)\r\n");
    io::console_write(b"  Capacity: ");
    memory::dec((ata::capacity_sectors() * 512) as usize);
    io::console_write(b" bytes (");
    memory::dec(ata::capacity_sectors() as usize);
    io::console_write(b" sectors)\r\n");
    io::console_write(b"  Formatted: ");
    io::console_write(if is_formatted() { b"yes\r\n" } else { b"no (run 'storage format')\r\n" });
}
