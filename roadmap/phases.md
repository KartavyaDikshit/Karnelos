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

**Status: Complete**

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
- **Framebuffer console** (`io.rs`): bootloader-provided framebuffer rendered
  with a built-in 8×8 bitmap font + 16-color VGA palette, replacing the legacy
  `0xB8000` text-mode buffer.

### Deliverables / Milestones
- [x] **M1 — Userspace toolchain + runtime**
  - [x] `userspace/karnelos-user.json` target spec (PIE, `disable-redzone`,
        `panic=abort`, based on `x86_64-unknown-none`)
  - [x] `linker.ld` + `_start` (zero BSS, call `main`, exit) in `rt.rs`
  - [x] `rt.rs` (`int 0x80` wrapper + `syscall!` macro, bump allocator,
        `memcpy/memset/memcmp/memmove` builtins, panic/alloc handlers)
  - [x] `userspace/src/main.rs` template with `KARNELOS_BODY_START/END` markers
        overwritten by the daemon
  - [x] `make userspace` → `cargo +nightly-2025-07-08 build -Z build-std=core,alloc`
  - [x] Verify: `readelf -h` PIE, `readelf -r` **no relocations**
- [x] **M2 — ELF64 loader** (`kernel/src/loader.rs`): parse hdr + program
      headers, map `PT_LOAD` segments, zero BSS, validate entry, apply
      `R_X86_64_RELATIVE` relocs, reject any other type
- [x] **M3 — Process model** (`kernel/src/process.rs`): `Process` struct,
      per-process P4 (clone kernel half), dedicated kernel stack in
      `TSS.privilege_stack_table[0]`, `run_elf` via `iretq`, exit syscall
      frees frames + restores kernel `CR3` + returns to shell
- [x] **M4 — Syscall ABI expansion** (`interrupts.rs`): `0 exit`, `1 write`,
      `2 read`, `4 storage_read`, `5 storage_write`, `6 getchar` (+ `42 hello`)
- [x] **M5 — COM2 streaming delivery**: daemon writes `userspace/src/main.rs`,
      builds the ELF, sends `<size>\n<bytes>` over TCP/COM2; kernel shell
      `gen <prompt>` enters an ELF receive state machine → load → auto-run.
      `run` re-runs the last received ELF. No reboot.
- [x] **M6 (docs half) — README + this roadmap** updated for the framebuffer
      console + ELF streaming pipeline. (App *persistence* + demo apps moved to
      Phase 5b below.)

### Test
- `make userspace` → PIE ELF produced with no relocations.
- `user` command → ring-3 inline demo runs ("Hello from ring 3!" + "Syscall 1
  works!"), returns to shell.
- `make run-test` → QEMU boots, banner + `karnelos> ` prompt on serial.

**Estimated effort:** done

---

## Phase 5b: Generated Applications — Persistence + Demos

**Status: Complete**

Turn the working ELF pipeline into a usable app platform. Scope chosen for this
phase: **persistence + lightweight demos** (no LLM-quality-dependent showcase
apps yet). Live `gen` requires `ollama serve` + `qwen2.5-coder:1.5b`; all
build/test work here is verifiable **without** a running LLM.

### Deliverables / Milestones
- [x] **M6a — COM2 flow control (correctness fix).** The kernel polls COM2
      one byte at a time in `shell_main_loop`; QEMU's UART has only a 16-byte
      FIFO, so a multi-KB ELF blasted by the daemon **overflows and drops
      bytes**. Added 256-byte chunked transfer with ACK flow control: the kernel
      writes an ACK byte back on COM2 after each chunk; the daemon waits for the
      ACK before sending the next chunk.
- [x] **M6b — App persistence** (`app save` / `app run`): the flat FS already
      stores raw bytes (`filesystem::write_file(name, &[u8])` /
      `read_file(name, &mut [u8]) -> usize`), so:
  - `app save <name>` → `write_file(name, &shell.last_elf[..last_elf_len])`
  - `app run <name>` → `read_file` into a buffer, then `process::run_elf(slice)`
  - Add `app` dispatch in `shell::execute` + `cmd_app`; update `help`
- [x] **M6c — Lightweight demo validation (no LLM needed):**
  - `user` command (ring-3 inline demo) exercises the `run_elf`/`iretq`/exit path
  - `make userspace` builds the checked-in counter app → confirm PIE, no relocs
  - Feed that built ELF through `app save <name>` / `app run <name>` to prove
    persistence end-to-end reproducibly
  - (Optional) a checked-in `echo`/interactive app template using syscall `2`/`6`

### Test
- `make build` + `make userspace` + `cd daemon && cargo build --release` all pass.
- `make run-test` boots, prints banner + prompt (no ollama).
- At the shell: `user` → demo runs and returns; `app save demo` then `app run demo`
  → same ELF reloads and runs from storage.
- (Precondition, out of scope for automated test) `ollama serve` running →
  `make run-daemon`, then `gen <prompt>` → app streams + runs, no reboot.

**Estimated effort:** ~1 week

---

## Phase 5c: Generated Applications — Showcase Apps

**Status: In progress**

Real LLM-generated apps on top of the Phase 5b platform:

### Deliverables
- [x] **Todo app with categories** — Full CLI todo app with add/list/done/delete
      commands, category filtering, persistent storage via syscalls 4/5
- [x] **File manager** — List, read, write, and delete files on persistent storage
      (uses new syscalls 7/8 for list and delete)
- [x] **Syscall 7 (storage_list)** — Returns formatted list of files from userspace
- [x] **Syscall 8 (storage_delete)** — Delete a file by name from userspace
- [x] **Terminal text editor** — Line-based file editor with insert/delete/replace,
      save/load, and line listing (commands: :i, :d, :r, :l, :w, :q)
- [x] **Calendar app with reminders** — Event management with add/list/delete,
      today reminders, date-based organization
- [x] **Math compiler (mathc)** — Custom expression parser and evaluator supporting
      +, -, *, /, ^, parentheses, variables; generates Rust code from math expressions
- [x] **Multi-app binary targets** — All showcase apps coexist as separate Cargo
      bin targets: `todo`, `files`, `editor`, `calendar`, `mathc`
- [x] Each app is generated, compiled, and deployable via conversation

### Test
- User says "build me a calendar with daily reminders stored in a custom binary format"
  → LLM designs format → generates code → it works
- User says "change the format to JSON" → LLM regenerates storage layer

**Estimated effort:** 3-4 weeks

---

## Phase 6: Self-Improving OS

**Status: In progress**

### Deliverables
- [x] **Performance metrics module** (`metrics.rs`): tracks syscall count/time,
      ring-3 transitions/time, ELFs loaded, storage operations, COM2 traffic,
      P4 clones, boot time
- [x] **Syscall 9 (get_metrics)**: retrieve formatted metrics from userspace
- [x] **Perf dashboard app** (`userspace/src/bin/perf.rs`): displays metrics
      from userspace via syscall 9, supports showing/clearing
- [x] **`perf` shell command**: show, clear, save/load metrics to/from storage
- [x] Integration: metrics recorded on syscalls, ring-3 transitions, ELF loads,
      storage ops, P4 clones, COM2 traffic
- [x] **System profile** (`system_profile` file): key-value metrics saved to
      storage on every boot for LLM consumption; `perf profile` command regenerates it
- [x] **Daemon prompt updated**: system prompt now documents all 9 syscalls
      (including storage_list, storage_delete, get_metrics); builds all app bins
- [x] **Ephemeral boot mode**: `bootmode ephemeral` setting reformats storage
      on next boot; `bootmode normal` restores persistent mode
- [x] **Feedback loop**: kernel uploads `system_profile` to daemon on every boot
      via `KARNELOS_PROFILE:` COM2 protocol; daemon stores profile and includes
      it in subsequent LLM code generation prompts for hardware-aware optimization
- [x] **Boot-time generation**: kernel auto-sends profile to daemon during boot
      sequence; daemon has the context ready for the first `gen` command
- [x] **`perf upload` shell command**: manually re-upload profile to daemon

### Test
- Run `perf` in the shell → shows boot time, syscall stats, ring-3 transitions
- Run `perf save` → metrics persisted to storage, `perf load` retrieves them
- Run the same task twice → second run uses cached/better code
- Boot once, create a workflow, reboot → LLM regenerates optimal components
- User says "optimize the scheduler for my compilation-heavy workload"
  → LLM generates a new scheduler → benchmarks show improvement

**Estimated effort:** 3-4 weeks

---

## Phase 7: Self-Hosted Image

**Status: In progress**

### Deliverables
- [x] **`make deploy` target** — builds kernel + all userspace bins + daemon in one command
- [x] **`make run-selfhosted`** — starts daemon + QEMU together with auto-restart loop
- [x] **`scripts/run.sh`** — full self-hosted runner script with daemon lifecycle management
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
| 5 - ELF loader + process model | 2-3 weeks | Phase 3a, 4 | ✅ Complete |
| 5b - App persistence + demos | ~1 week | Phase 5 | ✅ Complete |
| 5c - Showcase apps | 3-4 weeks | Phase 5b | 🔶 In progress |
| 6 - Self-Improving | 3-4 weeks | Phase 5b | 🔶 In progress |
| 7 - Self-Hosted | 2-3 weeks | Phase 6 | 🔶 In progress |
