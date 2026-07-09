# Karnelos OS

An AI-native operating system where a local LLM generates, compiles, and deploys
every component — from apps to filesystems to key bindings — optimized for your
exact hardware and tailored to your workflow.

## Philosophy

- **The kernel is the foundation.** A real Rust no_std kernel manages hardware,
  memory, processes, and devices. You never need to touch it unless you want to.
- **Everything else is generated.** Apps, tools, compilers, filesystems, protocols —
  all produced by the local LLM to fit your hardware and your needs.
- **Local-first, private.** The LLM runs on your machine. No data leaves your system.
- **You own everything.** Every generated file, every app, every configuration is yours
  to read, modify, and share.

## Project Structure

```
karnelos/
├── kernel/           # The Rust no_std kernel (boots in QEMU)
│   ├── src/main.rs   # Kernel entry point
│   ├── Cargo.toml
│   └── rust-toolchain.toml
├── generator/        # Host-side LLM code generator (Phase 0-6)
│   ├── src/main.rs
│   └── Cargo.toml
├── roadmap/          # Project scope, architecture, phase plan
│   ├── README.md
│   ├── scope.md
│   ├── architecture.md
│   ├── phases.md
│   └── tech-stack.md
├── Makefile
└── README.md
```

## Prerequisites

```bash
# Rust nightly
rustup install nightly
rustup target add x86_64-unknown-none --toolchain nightly
rustup component add llvm-tools-preview --toolchain nightly

# Bootable image builder
cargo install bootimage

# QEMU x86-64 emulator
brew install qemu
```

## Building & Running

```bash
# Quick test (boots and shows output in terminal)
make run-nographic

# Graphical window (macOS native display)
make run

# Debug console (Bochs debug port 0xE9, shows kernel boot even if serial fails)
make run-debug

# Build only
make build

# Clean artifacts
make clean
```

## Generate Code via LLM (Phase 3+)

```bash
# Requires Ollama running locally with a code model
make generate PROMPT="build a calendar app with reminders"
```

## Hardware Profile

The development target is a QEMU VM with:
- x86-64, 4 cores, 4GB RAM
- CPU: Skylake-Server (AVX2)
- Devices: UART serial, PS/2, virtio-blk

## Roadmap

See [roadmap/README.md](roadmap/README.md) for the full plan.

## Testing

Currently only testable in QEMU. Future phases will add:
- Boot-time hardware detection
- Self-generating kernel components
- Persistent user context
- Full application generation

## License

MIT
