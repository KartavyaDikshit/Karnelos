use spin::Mutex;

pub struct Metrics {
    pub syscall_count: u64,
    pub syscall_time_ns: u64,
    pub alloc_count: u64,
    pub dealloc_count: u64,
    pub ring3_transitions: u64,
    pub ring3_cycles: u64,
    pub com2_bytes_sent: u64,
    pub com2_bytes_recv: u64,
    pub elfs_loaded: u64,
    pub storage_reads: u64,
    pub storage_writes: u64,
    pub p4_clones: u64,
    pub boot_time_ns: u64,
}

impl Metrics {
    pub const fn new() -> Self {
        Metrics {
            syscall_count: 0,
            syscall_time_ns: 0,
            alloc_count: 0,
            dealloc_count: 0,
            ring3_transitions: 0,
            ring3_cycles: 0,
            com2_bytes_sent: 0,
            com2_bytes_recv: 0,
            elfs_loaded: 0,
            storage_reads: 0,
            storage_writes: 0,
            p4_clones: 0,
            boot_time_ns: 0,
        }
    }
}

pub static METRICS: Mutex<Metrics> = Mutex::new(Metrics::new());

#[inline(always)]
pub fn read_tsc() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() }
}

pub fn ns_from_tsc(ticks: u64) -> u64 {
    let khz = crate::io::cpu_freq_khz();
    ticks * 1000 / khz as u64
}

pub fn record_syscall(start_tsc: u64) {
    let end_tsc = read_tsc();
    let delta = end_tsc - start_tsc;
    let delta_ns = ns_from_tsc(delta);
    let mut m = METRICS.lock();
    m.syscall_count += 1;
    m.syscall_time_ns += delta_ns;
}

pub fn record_ring3_entry() {
    METRICS.lock().ring3_transitions += 1;
}

pub fn record_ring3_exit(start_tsc: u64) {
    let end_tsc = read_tsc();
    let delta = end_tsc - start_tsc;
    let delta_ns = ns_from_tsc(delta);
    let mut m = METRICS.lock();
    m.ring3_cycles += delta_ns;
}

pub fn record_alloc() {
    METRICS.lock().alloc_count += 1;
}

pub fn record_dealloc() {
    METRICS.lock().dealloc_count += 1;
}

pub fn record_elf_loaded() {
    METRICS.lock().elfs_loaded += 1;
}

pub fn record_storage_read() {
    METRICS.lock().storage_reads += 1;
}

pub fn record_storage_write() {
    METRICS.lock().storage_writes += 1;
}

pub fn record_com2_sent(len: u64) {
    METRICS.lock().com2_bytes_sent += len;
}

pub fn record_com2_recv(len: u64) {
    METRICS.lock().com2_bytes_recv += len;
}

pub fn record_p4_clone() {
    METRICS.lock().p4_clones += 1;
}

pub fn format_metrics(clear: bool) -> alloc::vec::Vec<u8> {
    let m = METRICS.lock();
    let s = alloc::format!(
        "Performance Metrics:\r\n\
        --------------------\r\n\
        Boot time:            {} ns\r\n\
        Syscalls:             {}\r\n\
        Syscall time:         {} ns total, {} ns avg\r\n\
        Ring-3 transitions:   {}\r\n\
        Ring-3 cycles:        {} ns total, {} ns avg\r\n\
        Allocations:          {} / deallocations: {}\r\n\
        ELFs loaded:          {}\r\n\
        Storage reads:        {} / writes: {}\r\n\
        COM2:                 {} bytes sent, {} bytes recv\r\n\
        P4 clones:            {}\r\n",
        m.boot_time_ns,
        m.syscall_count,
        m.syscall_time_ns,
        if m.syscall_count > 0 { m.syscall_time_ns / m.syscall_count } else { 0 },
        m.ring3_transitions,
        m.ring3_cycles,
        if m.ring3_transitions > 0 { m.ring3_cycles / m.ring3_transitions } else { 0 },
        m.alloc_count,
        m.dealloc_count,
        m.elfs_loaded,
        m.storage_reads,
        m.storage_writes,
        m.com2_bytes_sent,
        m.com2_bytes_recv,
        m.p4_clones,
    );
    if clear {
        // Dropping the lock before clearing
        drop(m);
        METRICS.lock().boot_time_ns = 0;
    }
    s.into_bytes()
}
