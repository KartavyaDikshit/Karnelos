# Karnelos OS — Implementation Phases

## Phase 0: Kernel Skeleton + Build System

**Status: Complete**

### Deliverables
- [x] Rust toolchain (nightly-2025-07-08 + x86_64-unknown-none target)
- [x] `bootimage` build pipeline (bootloader v0.9.35 with `map_physical_memory`)
- [x] Entry point that prints banner to VGA buffer + UART
- [x] Makefile with `build`, `run`, `run-daemon`, `clean` targets
- [x] `.gitignore` for build artifacts
- [x] Root README with setup and testing instructions

---

## Phase 1: Memory Manager

**Status: Complete**

### Deliverables
- [x] Physical frame allocator (bitmap, 4GB max, 131072 bytes bitmap)
- [x] Virtual memory support (bootloader provides `map_physical_memory`, phys-to-virt conversion)
- [x] Heap allocator (10MB via `linked_list_allocator::LockedHeap`)
- [x] `alloc` crate support (Vec, String, Box, format! all work)
- [x] Memory info display (total/free/used frames, heap address)

---

## Phase 2: Interrupts + Input

**Status: Complete**

### Deliverables
- [x] GDT, IDT setup (exception handlers with file:line display)
- [x] PIC initialization (8259, IRQ 0-15 remapped to 32-47)
- [x] PS/2 keyboard driver (scancode set 1, modifier tracking, ring buffer)
- [x] UART serial I/O (bidirectional, COM1 + COM2)
- [x] VGA text buffer (80x25, scrolling console, cursor tracking)
- [x] Shell with command dispatch and line editing

---

## Phase 3: LLM Code Generation

**Status: Complete (pragmatic approach)**

Instead of embedding the LLM inside the kernel (original vision), we use a host-side
daemon that communicates over a second serial port (COM2). This provides the full
AI-native OS loop without the complexity of running an LLM in-kernel.

### Deliverables
- [x] Host-side daemon (TCP server on :12345)
- [x] Ollama integration (calls `qwen2.5-coder:1.5b` model)
- [x] Code generation pipeline (prompt → LLM → save → rebuild → signal kernel)
- [x] Standalone generator CLI (`make generate PROMPT="..."`)
- [x] Guardrails: code fence stripping, fn/brace removal, build error detection
- [x] Kernel daemon communication: COM2 send/receive, `BUILD_OK`/`BUILD_FAILED` display
- [x] Reboot cycle: QEMU restart loop via `isa-debug-exit` device
- [x] Prompt engineering: byte strings, available API, examples

### Future (Phase 3b — In-Kernel LLM)
- [ ] llama.cpp or candle linked into kernel
- [ ] Model weights loaded at boot (Q4 quantized)
- [ ] Hardware detection engine (CPUID, cache, RAM, SIMD)

---

## Phase 3a: Userspace Execution

**Status: Not started — NEXT**

**Goal:** Generated code runs as a separate user process with memory protection,
not compiled into the kernel.

### Deliverables
- [ ] TSS setup for privilege level switching
- [ ] GDT with ring 3 segments (user code, user data)
- [ ] Syscall handler (software interrupt or `syscall`/`sysret`)
- [ ] Simple ELF loader or raw binary loader
- [ ] Memory protection: separate page tables, no kernel access
- [ ] Process management: spawn, exit, yield
- [ ] `run` command launches generated code as a user process

### Test
- Generate a "print hello" program, run it as a user process
- Generated code cannot access kernel memory (page fault on violation)
- Multiple user programs can run concurrently

**Estimated effort:** 1-2 weeks

---

## Phase 4: Persistent Storage + Filesystem

**Status: Not started**

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

**Status: Not started**

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

**Status: Not started**

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

**Status: Not started**

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

**Status: Not started**

- [ ] SMP boot (AP startup)
- [ ] Per-CPU data structures
- [ ] Scheduler (generated by LLM based on workload)
- [ ] Lock-free data structures

---

## Phase 9: Networking + Cloud (Post-MVP)

**Status: Not started**

- [ ] virtio-net driver
- [ ] TCP/IP stack (generated or smoltcp)
- [ ] Cloud offering: per-tenant Firecracker microVMs
- [ ] Each tenant gets code optimized for their specific hardware

---

## Total Estimated Timeline

| Phase | Time | Dependencies | Status |
|---|---|---|---|
| 0 - Skeleton | 1-2 days | Rust toolchain | ✅ Complete |
| 1 - Memory | 3-5 days | Phase 0 | ✅ Complete |
| 2 - Interrupts/Input | 3-5 days | Phase 0 | ✅ Complete |
| 3 - LLM Integration | 2-3 weeks | Phase 1, 2 | ✅ Complete (daemon-based) |
| 3a - Userspace | 1-2 weeks | Phase 2 | ⏳ Next |
| 4 - Persistent Storage | 1-2 weeks | Phase 1 | ❌ Not started |
| 5 - Applications | 3-4 weeks | Phase 3a, 4 | ❌ Not started |
| 6 - Self-Improving | 3-4 weeks | Phase 5 | ❌ Not started |
| 7 - Self-Hosted | 2-3 weeks | Phase 6 | ❌ Not started |
