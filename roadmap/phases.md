# Karnelos OS — Implementation Phases

## Phase 0: Kernel Skeleton + Build System

**Goal:** A Rust no_std kernel that boots in QEMU, prints to UART/screen, and can be
rebuilt with `make`. The foundation everything builds on.

### Deliverables
- [x] Rust toolchain (nightly + x86_64-unknown-none target)
- [x] `bootimage` build pipeline (bootable kernel image)
- [ ] Bootloader integration (Limine or `bootloader` crate)
- [ ] Minimal entry point that prints "Karnelos v0.1" to VGA buffer + UART
- [ ] Linker script and memory layout
- [ ] Makefile with `build`, `run`, `clean` targets
- [ ] `.gitignore` for build artifacts
- [ ] Root README with setup and testing instructions

### Build & Test
```bash
cd kernel
cargo bootimage
qemu-system-x86_64 -drive format=raw,file=target/x86_64-unknown-none/debug/bootimage-karnelos-kernel.bin
```

**Estimated effort:** 1-2 days

---

## Phase 1: Memory Manager

**Goal:** The kernel has a working physical + virtual memory manager.

### Deliverables
- [ ] Physical frame allocator (bitmap or buddy system)
- [ ] Virtual memory manager (page tables, higher-half mapping)
- [ ] Heap allocator (bump → slab/buddy)
- [ ] Memory-mapped I/O for device access
- [ ] LLM can regenerate the allocator (e.g., switch from bump to slab)

### Test
- Kernel boots and prints memory layout (total RAM, free pages, heap address)
- User can say "show memory map" and see page table structure

**Estimated effort:** 3-5 days

---

## Phase 2: Interrupts + Input

**Goal:** The kernel handles interrupts and takes keyboard input.

### Deliverables
- [ ] GDT, IDT, TSS setup
- [ ] PIC/APIC initialization
- [ ] PS/2 keyboard driver
- [ ] Keyboard ring buffer → text input
- [ ] UART serial I/O (bidirectional)
- [ ] LLM can regenerate key binding mappings

### Test
- Type at the console, see characters echoed back
- User says "remap capslock to ctrl" → LLM generates new keymap → it works

**Estimated effort:** 3-5 days

---

## Phase 3: LLM System Service

**Goal:** The local LLM runs inside the kernel and generates code.

### Deliverables
- [ ] llama.cpp or candle statically linked into kernel
- [ ] Model weights loaded at boot (Q4 quantized, 1.5B default)
- [ ] Hardware detection engine (CPUID, cache, RAM, SIMD)
- [ ] User context database (SQLite, embedded)
- [ ] Code generation pipeline (prompt → LLM → save .rs → cargo check → compile)
- [ ] Guardrail enforcement (validation, static analysis, retry logic)
- [ ] CLI shell backed by the LLM

### Test
- Boot → LLM starts → user types "print hello" → LLM generates code → it runs
- User types "make a todo app" → LLM generates, compiles, runs it

**Estimated effort:** 2-3 weeks

---

## Phase 4: Persistent Storage + Filesystem

**Goal:** User data persists across reboots.

### Deliverables
- [ ] virtio-blk driver
- [ ] Block device abstraction
- [ ] Filesystem (tmpfs at boot, persistent on virtio-blk)
- [ ] Generated storage formats (LLM can create custom binary formats)
- [ ] Directory structure generated from user context

### Test
- Create a file, reboot, file is still there
- User says "save my calendar data in /home/apps/calendar" → LLM creates storage

**Estimated effort:** 1-2 weeks

---

## Phase 5: Generated Applications

**Goal:** Users can build real applications by conversation.

### Deliverables
- [ ] Calendar app with reminders
- [ ] Todo app with categories
- [ ] Custom LaTeX compiler (the writer's use case: `7exp7=8x()x`)
- [ ] Terminal text editor
- [ ] File manager
- [ ] Each app is generated, compiled, and deployable via conversation

### Test
- User says "build me a calendar with daily reminders stored in a custom binary format"
  → LLM designs format → generates code → it works
- User says "change the format to JSON" → LLM regenerates storage layer

**Estimated effort:** 3-4 weeks

---

## Phase 6: Self-Improving OS

**Goal:** The OS profiles itself and regenerates components for better performance.

### Deliverables
- [ ] Performance monitoring (execution cycles, cache misses, alloc patterns)
- [ ] Feedback loop: metrics → user context → LLM regeneration
- [ ] Boot-time generation (kernel regenerates optimal components from user profile)
- [ ] Ephemeral boot mode (fresh OS every boot, only user data persists)

### Test
- Run the same task twice → second run uses cached/better code
- Boot once, create a workflow, reboot → LLM regenerates optimal components
- User says "optimize the scheduler for my compilation-heavy workload"
  → LLM generates a new scheduler → benchmarks show improvement

**Estimated effort:** 3-4 weeks

---

## Phase 7: Self-Hosted Image

**Goal:** The entire OS ships as a single bootable image, no host tools needed.

### Deliverables
- [ ] Single `.img` file with bootloader + kernel + LLM weights + user data partition
- [ ] Bootstrap sequence: boot → detect HW → load LLM → generate production kernel
- [ ] No dependency on host machine for generation
- [ ] The generator (previously on host) now runs inside the OS

### Test
- Copy `.img` → boot in QEMU → everything works end-to-end
- Full cycle: boot → LLM starts → user types goals → code is generated → apps run

**Estimated effort:** 2-3 weeks

---

## Phase 8: SMP + Advanced Scheduling (Post-MVP)

**Goal:** Multi-core support with a generated scheduler.

- [ ] SMP boot (AP startup)
- [ ] Per-CPU data structures
- [ ] Scheduler (generated by LLM based on workload)
- [ ] Lock-free data structures

---

## Phase 9: Networking + Future Cloud (Post-MVP)

**Goal:** Generated networking stack, cloud provider model.

- [ ] virtio-net driver
- [ ] TCP/IP stack (generated or smoltcp)
- [ ] Cloud offering: per-tenant Firecracker microVMs
- [ ] Each tenant gets code optimized for their specific hardware

---

## Total Estimated Timeline

| Phase | Time | Dependencies |
|---|---|---|
| 0 - Skeleton | 1-2 days | Rust toolchain |
| 1 - Memory | 3-5 days | Phase 0 |
| 2 - Interrupts/Input | 3-5 days | Phase 0 |
| 3 - LLM Service | 2-3 weeks | Phase 1, 2 |
| 4 - Persistent Storage | 1-2 weeks | Phase 1 |
| 5 - Applications | 3-4 weeks | Phase 3, 4 |
| 6 - Self-Improving | 3-4 weeks | Phase 5 |
| 7 - Self-Hosted | 2-3 weeks | Phase 6 |

**Total to Phase 7:** ~12-16 weeks of focused development.
