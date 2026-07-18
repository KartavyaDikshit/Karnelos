use crate::io;

static CPU_COUNT: spin::Once<usize> = spin::Once::new();

pub fn cpu_count() -> usize {
    *CPU_COUNT.get().unwrap_or(&1)
}

pub fn init(_rsdp_hint: Option<u64>) {
    io::debug_write(b"SMP: running single-core\n");
    CPU_COUNT.call_once(|| 1);
}
