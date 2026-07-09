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

The AI-native OS loop is fully functional:

1. **`gen <prompt>`** — Describe what you want in natural language
2. **Daemon** forwards the prompt to Ollama (`qwen2.5-coder:1.5b`), generates Rust code
3. **Build** — The generated code is compiled into the kernel binary
4. **`reboot`** — QEMU exits and restarts with the new kernel
5. **`run`** — Execute the LLM-generated code

### Demo

```
karnelos> gen write "Hello from AI!" to serial port
BUILD_OK  (Type 'reboot' to load the new kernel)
karnelos> reboot
[kernel rebuilt with new code, QEMU restarts]
karnelos> run
Hello from AI!
```

## Project Structure

```
karnelos/
├── kernel/           # The Rust no_std kernel (boots in QEMU)
│   ├── src/
│   │   ├── main.rs       # Entry point, main loop
│   │   ├── io.rs         # Serial, VGA, console I/O
│   │   ├── interrupts.rs # IDT, PIC, exception/IRQ handlers
│   │   ├── keyboard.rs   # PS/2 keyboard driver
│   │   ├── memory.rs     # Physical frame allocator, heap
│   │   ├── shell.rs      # Shell with command dispatch
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
```

## Prerequisites

```bash
# Rust nightly
rustup install nightly-2025-07-08
rustup target add x86_64-unknown-none --toolchain nightly-2025-07-08
rustup component add llvm-tools-preview --toolchain nightly-2025-07-08

# Bootable image builder
cargo install bootimage

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

## Commands (within the OS)

| Command | Description |
|---------|-------------|
| `help` | Show available commands |
| `memory` | Show memory info (frames, heap, etc.) |
| `clear` | Clear screen |
| `echo <text>` | Echo text back |
| `info` | System information |
| `gen <prompt>` | Generate code via LLM (sends to daemon) |
| `run` | Execute the last generated code |
| `reboot` | Reboot into new kernel (after successful gen) |
| `test-heap` | Run heap allocation test |

## Hardware Profile

Development target is a QEMU VM with:
- x86-64, 4 cores, 4GB RAM
- CPU: Skylake-Server (AVX2)
- Devices: Dual UART serial, PS/2 controller, VGA text mode

## Architecture

- **COM1** (0x3F8) → User terminal (stdio)
- **COM2** (0x2F8) → Daemon connection (TCP :12345)
- **VGA** (0xB8000) → Text mode display (80x25)
- **Debug port** (0xE9) → Bochs debug console

The daemon runs on the host, receives `KARNELOS_GEN:<prompt>` lines from the kernel
via COM2, calls Ollama, writes the generated code to `kernel/src/generated.rs`,
rebuilds the kernel, and sends `BUILD_OK` or `BUILD_FAILED` back.

## Roadmap

See [roadmap/README.md](roadmap/README.md) for the full plan and next steps.

## License

MIT
