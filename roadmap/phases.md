# Karnelos OS — Implementation Phases

## Phase 0: Kernel Skeleton + Build System

**Status: Complete**

### Deliverables
- [x] Rust toolchain (nightly-2025-07-08 + x86_64-unknown-none target)
- [x] Custom `mkimage` build pipeline (bootloader v0.11.15 with `BiosBoot::create_disk_image`)
- [x] Bootloader 0.11.15 patched for cross-compilation (cargo build instead of cargo install)
- [x] Bootloader target JSON files fixed for rustc 1.90 (string target-pointer-width)
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

**Status: Complete**

### Deliverables
- [x] GDT with ring 0/3 segments (CS, DS) + TSS with privilege stack table
- [x] `int 0x80` syscall handler registered with DPL=3
- [x] User program in `.user_prog` section, copied to user page at `0x8000400000`
- [x] `iretq` to ring 3 from kernel
- [x] Stack at `0x807FFFF000` (top of 512GB-1TB range)
- [x] All page table levels (P4, P3, P2, P1) set `PRESENT | WRITABLE | USER_ACCESSIBLE` (0x7)
- [x] ISA hole (frames 160-255) reserved in bitmap allocator
- [x] Syscall 0: exit (returns to shell via TSS stack switch)
- [x] Syscall 1: console_write(buf, len) — write to VGA+serial from userspace
- [x] Syscall 42: hello
- [x] LLM prompts in daemon/generator updated with syscall API documentation
- [x] Return-to-shell after user exit (no reboot needed)
- [x] TSS RSP0 uses properly allocated frame (not hardcoded address)

### Key Details
- User virtual address range: P4[1] → 512GB-1024GB
- Code at `0x8000400000` (512GB + 4MB), stack at `0x807FFFF000` (512GB + 2GB - 4KB)
- All page table entries (including intermediate) must have `USER_ACCESSIBLE` bit
- `map_user_pages()` creates entire page hierarchy in one pass
- `int_80_stub` saves/restores all GPRs, calls `syscall_handler(num, arg1, arg2, arg3, frame)`
- RIP-relative addressing via `lea rbx, [rip + label]` works (section offsets preserved)
- Exit handler switches to TSS RSP0 stack and jumps to `shell_main_loop()`
- Shell is a `static mut` (was local) to survive stack abandonment

---

## Phase 4: Persistent Storage + Filesystem

**Status: Complete (core)**

### Deliverables
- [x] Block device driver (ATA PIO over IDE, secondary channel master)
- [x] Block device abstraction (`read_block` / `write_block` / `is_present` / `capacity_sectors`)
- [x] Flat filesystem (superblock + dir + block bitmap + data sectors)
- [x] `storage` shell command: format / write / read / ls / info
- [x] Persistence across reboot (verified: write → reboot → read)
- [x] Storage syscalls for userspace (read/write by name) — see Phase 5

### Notes
- Planned backend was virtio-blk, but QEMU 11 dropped the legacy virtio
  queue interface (config-space reads still worked, yet the device never advanced
  its `used` ring). ATA PIO gives the same block-level API reliably.
- virtio-blk code was prototyped (`pci.rs` + `virtio_blk.rs`) and taught the
  needed fixes (NEXT-chain flags, 2 contiguous vring pages) for a future
  modern-virtio revisit.

### Test
- `storage write note Hello` → `reboot` → `storage read note` → "Hello" ✓

**Estimated effort:** done

---

## Phase 5: Generated Applications (ELF loader + process model)

**Status: In progress**

The "real OS that writes apps" experience. Instead of recompiling the kernel and
rebooting for every generated snippet, the LLM (host daemon for now; on-device in
Phase 7) generates a **Rust ring-3 ELF app** that the running kernel streams in
over COM2, loads on demand, and executes as an isolated process — no reboot, no
kernel rebuild.

### Architecture
```
host:  prompt ─► daemon ─build(userspace target)─► ELF ─COM2(TCP)─► kernel
                                                        │
        kernel: receive → ELF parse → map P4 → iretq ring3 → run → exit → shell
```
- Per-process **page tables**: new P4 clones the kernel's upper-half entries
  (256..512) and adds user code/stack/heap in the lower half (code `0x400000`,
  stack `0x7FFF_F000`, heap reserved region). This isolates apps from the kernel
  and from each other, replacing the old "map into the kernel's P4" approach.
- Stable **syscall ABI**: `rax`=num, args `rdi,rsi,rdx,r10,r8,r9`, return `rax`,
  dispatched through `int 0x80` (DPL=3).
- Single process at a time (multitasking deferred to Phase 8).

### Deliverables / Milestones
- [ ] **M1 — Userspace toolchain + runtime**
  - [ ] `userspace/karnelos-user.json` target spec (PIE, `disable-redzone`,
        `panic=abort`, based on `x86_64-unknown-none`)
  - [ ] `linker.ld` + `_start` (zero BSS, 16B-aligned stack, call `main`, exit)
  - [ ] `rt/syscall.rs` (`int 0x80` wrapper + `syscall!` macro), `rt/panic.rs`,
        optional bump `GlobalAllocator`
  - [ ] `app/src/main.rs` template overwritten by the daemon
  - [ ] `make userspace` → `cargo build -Z build-std=core,alloc --target ...`
  - [ ] Verify: `readelf -h` PIE, `readelf -r` **no relocations**
- [ ] **M2 — ELF64 loader** (`kernel/src/loader.rs`): parse hdr + program
      headers, map `PT_LOAD` segments, zero BSS, validate entry, assert no relocs
- [ ] **M3 — Process model** (`kernel/src/process.rs`, replaces `userspace.rs`
      demo): `Process` struct, per-process P4 (clone kernel half), dedicated
      kernel stack in `TSS.privilege_stack_table[0]`, `run_process` via `iretq`,
      exit syscall frees frames + restores kernel `CR3` + returns to shell
- [ ] **M4 — Syscall ABI expansion** (`interrupts.rs`): `1 write`, `2 read`,
      `3 exit`, `4 storage_read`, `5 storage_write`, `6 getchar` (keep `0/1/42`)
- [ ] **M5 — COM2 streaming delivery**: daemon writes `app/src/main.rs`, builds
      ELF, sends `KARNELOS_ELF:<u32 len>\n<bytes>` over TCP/COM2; kernel shell
      `gen <prompt>` enters an ELF receive state machine → load → run. Add `run`
      to re-run last ELF. No reboot.
- [ ] **M6 — Demo apps + docs**: generate a working demo via `gen`; optional
      `storage write <app> <elf>` + `run <name>` for persistence; update README
      + this roadmap

### Test
- `make run-daemon` → OS: `gen print the numbers 1 through 5` → app runs
  **without reboot**, exits back to shell.
- `storage format/write/read/ls` still works via M4 syscalls.
- `user` command still demonstrates ring-3 execution.

**Estimated effort:** 2-3 weeks (M1+M2+M3 are the heavy lifting)

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
| 3a - Userspace | 1-2 weeks | Phase 2 | ✅ Complete |
| 4 - Persistent Storage | 1-2 weeks | Phase 1 | ✅ Complete |
| 5 - Applications | 3-4 weeks | Phase 3a, 4 | ❌ Not started |
| 6 - Self-Improving | 3-4 weeks | Phase 5 | ❌ Not started |
| 7 - Self-Hosted | 2-3 weeks | Phase 6 | ❌ Not started |
