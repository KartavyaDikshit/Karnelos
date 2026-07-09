use bootloader::bootinfo::{BootInfo, MemoryRegionType};
use x86_64::PhysAddr;
use x86_64::VirtAddr;
use x86_64::structures::paging::{Size4KiB, PhysFrame};
use x86_64::structures::paging::FrameAllocator as FrameAllocatorTrait;

use crate::io;

pub const PAGE_SIZE: u64 = 4096;
const MAX_PHYSICAL_MEMORY: u64 = 4 * 1024 * 1024 * 1024;
pub const MAX_FRAMES: usize = (MAX_PHYSICAL_MEMORY / PAGE_SIZE) as usize;
const BITMAP_SIZE: usize = MAX_FRAMES / 8;
const HEAP_SIZE: usize = 10 * 1024 * 1024;

fn hex(v: u64) {
    io::serial_write(b"0x");
    for s in (0..16).rev() {
        let nib = ((v >> (s * 4)) & 0xF) as u8;
        io::serial_putc(if nib < 10 { b'0' + nib } else { b'A' + nib - 10 });
    }
}

pub fn dec(v: usize) {
    if v == 0 { io::serial_putc(b'0'); return; }
    let mut b = [0u8; 20];
    let mut i = 20;
    let mut n = v;
    while n > 0 { i -= 1; b[i] = b'0' + (n % 10) as u8; n /= 10; }
    io::serial_write(&b[i..]);
}

pub struct FrameAllocator {
    bitmap: [u8; BITMAP_SIZE],
    total_frames: usize,
    used_frames: usize,
    next_hint: usize,
}

impl FrameAllocator {
    const fn new() -> Self {
        FrameAllocator {
            bitmap: [0; BITMAP_SIZE],
            total_frames: 0,
            used_frames: 0,
            next_hint: 0,
        }
    }

    pub fn init(&mut self, boot_info: &'static BootInfo) {
        let mut max_frame = 0;
        for region in boot_info.memory_map.iter() {
            let end = region.range.end_frame_number as usize;
            if end > max_frame { max_frame = end; }
            if region.region_type != MemoryRegionType::Usable {
                for f in region.range.start_frame_number as usize..end.min(MAX_FRAMES) {
                    self.mark_used(f);
                }
            }
        }
        self.total_frames = max_frame.min(MAX_FRAMES);
        self.used_frames = 0;
        for i in 0..self.total_frames {
            if self.is_used(i) { self.used_frames += 1; }
        }
    }

    fn mark_used(&mut self, frame: usize) {
        self.bitmap[frame / 8] |= 1 << (frame % 8);
    }

    #[allow(dead_code)]
    fn mark_free(&mut self, frame: usize) {
        self.bitmap[frame / 8] &= !(1 << (frame % 8));
    }

    fn is_used(&self, frame: usize) -> bool {
        (self.bitmap[frame / 8] & (1 << (frame % 8))) != 0
    }

    pub fn allocate(&mut self) -> Option<usize> {
        let start = self.next_hint;
        for i in 0..self.total_frames {
            let idx = (start + i) % self.total_frames;
            if !self.is_used(idx) {
                self.mark_used(idx);
                self.used_frames += 1;
                self.next_hint = (idx + 1) % self.total_frames;
                return Some(idx);
            }
        }
        None
    }

    pub fn mark_range_used(&mut self, start: usize, count: usize) {
        for i in start..start + count {
            if !self.is_used(i) {
                self.mark_used(i);
                self.used_frames += 1;
            }
        }
    }

    pub fn print_info(&self, phys_mem_offset: u64) {
        io::serial_write(b"\r\n=== Memory Info ===\r\n");
        io::serial_write(b"Physical memory offset: "); hex(phys_mem_offset); io::serial_write(b"\r\n");
        io::serial_write(b"Total RAM: "); hex(self.total_memory()); io::serial_write(b"\r\n");
        io::serial_write(b"Free RAM:  "); hex(self.free_memory()); io::serial_write(b"\r\n");
        io::serial_write(b"Used RAM:  "); hex(self.used_memory()); io::serial_write(b"\r\n");
        io::serial_write(b"Frames: "); dec(self.total_frames); io::serial_write(b" total, ");
        dec(self.total_frames - self.used_frames); io::serial_write(b" free\r\n");
        io::serial_write(b"Bitmap size: "); dec(BITMAP_SIZE); io::serial_write(b" bytes\r\n");
    }

    fn total_memory(&self) -> u64 { self.total_frames as u64 * PAGE_SIZE }
    fn free_memory(&self) -> u64 { (self.total_frames - self.used_frames) as u64 * PAGE_SIZE }
    fn used_memory(&self) -> u64 { self.total_memory() - self.free_memory() }
}

unsafe impl FrameAllocatorTrait<Size4KiB> for FrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.allocate().map(|n| PhysFrame::containing_address(PhysAddr::new(n as u64 * PAGE_SIZE)))
    }
}

pub static FRAME_ALLOCATOR: spin::Mutex<FrameAllocator> = spin::Mutex::new(FrameAllocator::new());

static PHYS_MEM_OFFSET: spin::Mutex<u64> = spin::Mutex::new(0);

static HEAP_PHYS_START: spin::Mutex<u64> = spin::Mutex::new(0);

pub fn phys_to_virt(addr: PhysAddr) -> VirtAddr {
    VirtAddr::new(addr.as_u64() + *PHYS_MEM_OFFSET.lock())
}

pub fn init(boot_info: &'static BootInfo) {
    let offset = boot_info.physical_memory_offset;
    *PHYS_MEM_OFFSET.lock() = offset;
    FRAME_ALLOCATOR.lock().init(boot_info);

    let heap_frames = HEAP_SIZE / PAGE_SIZE as usize;
    for region in boot_info.memory_map.iter() {
        if region.region_type == MemoryRegionType::Usable {
            let rstart = region.range.start_addr();
            let rend = region.range.end_addr();
            let rsize = rend - rstart;
            if rsize >= HEAP_SIZE as u64 {
                let start_frame = (rstart / PAGE_SIZE) as usize;
                FRAME_ALLOCATOR.lock().mark_range_used(start_frame, heap_frames);
                *HEAP_PHYS_START.lock() = rstart;
                break;
            }
        }
    }

    FRAME_ALLOCATOR.lock().print_info(offset);
    io::serial_write(b"Memory manager initialized\r\n");
}

pub fn init_heap() {
    let heap_phys = *HEAP_PHYS_START.lock();
    let heap_virt = phys_to_virt(PhysAddr::new(heap_phys));
    unsafe {
        HEAP_ALLOCATOR.lock().init(heap_virt.as_u64() as *mut u8, HEAP_SIZE);
    }
    io::serial_write(b"Heap initialized: ");
    hex(heap_virt.as_u64());
    io::serial_write(b" (phys ");
    hex(heap_phys);
    io::serial_write(b", size ");
    dec(HEAP_SIZE);
    io::serial_write(b")\r\n");
}

use linked_list_allocator::LockedHeap;

#[global_allocator]
pub static HEAP_ALLOCATOR: LockedHeap = LockedHeap::empty();
