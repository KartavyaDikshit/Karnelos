# Karnelos OS

An AI-native operating system where a local LLM generates, compiles, and deploys
every component — from apps to tools to filesystems — optimized for your
exact hardware and tailored to your workflow.

## Philosophy

- **The kernel is the foundation.** A real Rust `no_std` kernel manages hardware,
  memory, interrupts, and I/O. You never need to touch it unless you want to.
- **Everything else is generated.** Apps, tools, filesystems, protocols —
  all produced by the local LLM to fit your hardware and your needs.
- **Local-first, private.** The LLM runs on your machine via Ollama.
  No data leaves your system.
- **You own everything.** Every generated file, every app, every configuration is yours
  to read, modify, and share.

## Current Status

### AI-Native OS Loop (Phase 0-3)
The AI-native OS loop is fully functional:

1. **`gen <prompt>`** — Describe what you want in natural language
2. **Daemon** forwards the prompt to Ollama (`qwen2.5-coder:1.5b`), generates Rust code
3. **Build** — The generated code is compiled into the kernel binary
4. **`reboot`** — QEMU exits and restarts with the new kernel
5. **`run`** — Execute the LLM-generated code

### Userspace Execution (Phase 3a)
Ring 3 userspace is fully operational:

- **GDT** with ring 0/3 code and data segments + TSS for privilege switching
- **`int 0x80` syscall handler** with DPL=3 for controlled entry into the kernel
- **Syscalls implemented:**
  - `0` — Exit program (returns to the shell prompt, no reboot needed)
  - `1` — `console_write(buf, len)` — write to VGA + serial from ring 3
  - `42` — Print "Hello from ring 3!"
- **`user` command** — Runs a hardcoded demo program in ring 3 that tests all syscalls
- **Memory isolation:** User code runs on separate pages at `0x8000400000` (P4[1]),
  outside the kernel's address space, with `USER_ACCESSIBLE` bit set at all page table levels

### Persistent Storage (Phase 4)
A block device driver and flat filesystem provide persistence across reboots:

- **ATA PIO block driver** (`ata.rs`) over the IDE controller (secondary channel, master).
  QEMU exposes the disk via `-drive if=ide,index=2,file=storage.img`.
- **Block device abstraction:** `read_block(sector, buf)` / `write_block(sector, buf)`
  plus `is_present()` / `capacity_sectors()`.
- **Flat filesystem** (`filesystem.rs`): superblock + directory (64 entries) + block bitmap
  + data sectors. Files persist across reboots.
- **`storage` shell command:**
  - `storage format` — Initialize the disk
  - `storage write <name> <text>` — Write a file
  - `storage read <name>` — Read a file
  - `storage ls` — List files
  - `storage info` — Show disk info

> **Note on block backend:** The plan called for virtio-blk, but QEMU 11 dropped the
> legacy virtio queue interface (config-space reads still worked, but the device never
> advanced its `used` ring). ATA PIO is reliable across QEMU versions and provides the
> same block-level API; virtio-blk can be revisited later for performance.

### On-Demand Apps (Phase 5 — in progress)

The LLM generates **Rust ring-3 ELF apps** that the running kernel streams in over
COM2, loads, and runs as an isolated process — **no kernel recompile, no reboot**.
Each app gets its own page tables (kernel upper-half cloned + user lower-half) and
talks to the kernel via a stable `int 0x80` syscall ABI (`rax`=num, args
`rdi,rsi,rdx,r10,r8,r9`, return `rax`).

```
karnelos> gen print the numbers 1 through 5
[Sending to daemon → builds userspace ELF → streams over COM2]
1
2
3
4
5
[app exited — back to shell, no reboot]
```

### Demo

```
karnelos> gen write "Hello from AI!" to serial port
BUILD_OK  (Type 'reboot' to load the new kernel)
karnelos> reboot
[kernel rebuilt with new code, QEMU restarts]
karnelos> run
Hello from AI!

karnelos> user
Jumping to ring 3...
Hello from ring 3!
Syscall 1 works!
User program exited

karnelos> storage format
storage: disk formatted (131072 sectors, 131031 free for data)
karnelos> storage write note Hello from persistent storage
storage: wrote 'note' (29 bytes)
karnelos> storage read note
Hello from persistent storage
karnelos> storage ls
Files on persistent storage:
  note  (29 bytes)
```

## Prerequisites

```bash
# Rust nightly
rustup install nightly-2025-07-08
rustup target add x86_64-unknown-none --toolchain nightly-2025-07-08
rustup component add llvm-tools-preview --toolchain nightly-2025-07-08

# QEMU x86-64 emulator
brew install qemu

# Ollama (for code generation)
brew install ollama
ollama pull qwen2.5-coder:1.5b
```

## Building & Running

```bash
# Build only
make build

# Terminal mode (serial console)
make run-nographic

# Graphical window (macOS native display)
make run

# Full AI-native OS loop (daemon + QEMU in restart loop)
make run-daemon

# Generate code via LLM (standalone CLI)
make generate PROMPT="print hello world"

# Clean
make clean
```

## Project Structure

```
karnelos/
├── kernel/           # The Rust no_std kernel (boots in QEMU)
│   ├── src/
│   │   ├── main.rs       # Entry point, main loop
│   │   ├── io.rs         # Serial, VGA, console I/O
│   │   ├── interrupts.rs # IDT, PIC, exception/IRQ handlers, syscalls
│   │   ├── keyboard.rs   # PS/2 keyboard driver
│   │   ├── memory.rs     # Physical frame allocator, heap
│   │   ├── shell.rs      # Shell with command dispatch
│   │   ├── userspace.rs  # GDT+TSS, page table setup, ring 3 execution
│   │   └── generated.rs  # Auto-generated code from LLM
│   ├── Cargo.toml
│   └── rust-toolchain.toml
├── daemon/           # Host-side TCP daemon (Ollama bridge)
│   ├── src/main.rs  # Listens on :12345, calls Ollama, rebuilds kernel
│   └── Cargo.toml
├── generator/        # Standalone CLI code generator
│   ├── src/main.rs
│   └── Cargo.toml
├── roadmap/          # Project scope, architecture, phase plan
├── Makefile
└── README.md