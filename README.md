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

### Framebuffer Graphics (Phase 2b)
The kernel now renders to a **framebuffer** (provided by the bootloader) instead of the
legacy VGA text mode buffer (`0xB8000`). This enables:

- Arbitrary resolutions (bootloader auto-detects the best mode)
- Full pixel control via a built-in 8×8 bitmap font (ASCII 32–126)
- 16-color VGA palette mapped to 24-bit RGB
- Scrolling console (80×25 character grid, pixel-perfect scroll)

### AI-Native App Generation (Phase 5)
The LLM generates **Rust ring-3 ELF apps** — no kernel recompile, no reboot:

1. **`gen <prompt>`** — Describe what you want in natural language
2. **Daemon** forwards the prompt to Ollama (`qwen2.5-coder:1.5b`), generates
   a userspace app (`userspace/src/main.rs`), and builds it as a PIE ELF
3. **ELF streamed over COM2** — The daemon sends `<size>\n<binary>` back to the kernel
4. **`run` (or auto-run)** — The kernel parses the ELF, maps it into a fresh process
   address space, and executes it in ring 3 via `iretq`
5. **App exits** — returns to the shell prompt, ready for the next `gen`

### Userspace Execution (Phase 3a)
Ring 3 userspace is fully operational:

- **GDT** with ring 0/3 code and data segments + TSS for privilege switching
- **`int 0x80` syscall handler** with DPL=3 for controlled entry into the kernel
- **Syscalls implemented:**
  - `0` — Exit program (returns to the shell prompt, no reboot needed)
  - `1` — `console_write(buf, len)` — write to VGA + serial from ring 3
  - `2` — `read(buf, len)` — read from keyboard
  - `4` — `storage_read(name, buf, len)` — read a file
  - `5` — `storage_write(name, data, len)` — write a file
  - `6` — `getchar()` — single char or 0
  - `42` — Print "Hello from ring 3!"
- **Memory isolation:** Each app gets its own page tables (kernel upper-half cloned +
  user lower-half at `0x400000`), with `USER_ACCESSIBLE` bit at all levels

### Persistent Storage (Phase 4)
A block device driver and flat filesystem provide persistence across reboots:

- **ATA PIO block driver** (`ata.rs`) over the IDE controller (secondary channel, master)
- **Flat filesystem** (`filesystem.rs`): superblock + directory (64 entries) + block bitmap
  + data sectors
- **`storage` shell command:**
  - `storage format` — Initialize the disk
  - `storage write <name> <text>` — Write a file
  - `storage read <name>` — Read a file
  - `storage ls` — List files
  - `storage info` — Show disk info

> **Note on block backend:** The plan called for virtio-blk, but QEMU 11 dropped the
> legacy virtio queue interface. ATA PIO is reliable across QEMU versions and provides the
> same block-level API; virtio-blk can be revisited later for performance.

### Demo

```
karnelos> gen print the numbers 1 through 5
Sending to daemon (COM2)...
[daemon generates app → builds ELF → streams over TCP/COM2]
ELF received, loading...
Jumping to ring 3...
1
2
3
4
5

karnelos> run
Running last ELF...
Jumping to ring 3...
1
2
3
4
5

karnelos> user
Jumping to ring 3...
Hello from ring 3!
Syscall 1 works!

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
├── kernel/              # The Rust no_std kernel (boots in QEMU)
│   ├── src/
│   │   ├── main.rs          # Entry point, main loop
│   │   ├── io.rs            # Serial, framebuffer, console I/O
│   │   ├── interrupts.rs    # IDT, PIC, exception/IRQ handlers, syscalls
│   │   ├── keyboard.rs      # PS/2 keyboard driver
│   │   ├── memory.rs        # Physical frame allocator, heap
│   │   ├── shell.rs         # Shell with command dispatch
│   │   ├── loader.rs        # ELF64 parser + page mapper
│   │   ├── process.rs       # Ring-3 process model (P4 clone + iretq)
│   │   ├── ata.rs           # ATA PIO block driver
│   │   └── filesystem.rs    # Flat filesystem
│   ├── builder/             # Bootloader disk-image builder
│   ├── x86_64-karnelos.json # Custom target spec (PIC, soft-float)
│   └── Cargo.toml
├── userspace/          # Ring-3 app template (overwritten by daemon)
│   ├── src/
│   │   ├── main.rs      # App entry (KARNELOS_BODY_START/END markers)
│   │   ├── rt.rs        # _start, BSS zero, bump allocator, mem ops
│   │   └── syscall.rs   # int 0x80 wrappers + syscall! macro
│   ├── karnelos-user.json # Target spec (PIE, small code model)
│   ├── linker.ld        # Linker script (base 0x400000)
│   └── Cargo.toml
├── daemon/             # Host-side TCP daemon (Ollama bridge)
│   ├── src/main.rs     # Listens on :12345, generates userspace ELF
│   └── Cargo.toml
├── generator/          # Standalone CLI code generator
│   ├── src/main.rs
│   └── Cargo.toml
├── tools/
│   └── mkimage/        # Custom disk-image build pipeline
├── roadmap/            # Project scope, architecture, phase plan
├── Makefile
└── README.md