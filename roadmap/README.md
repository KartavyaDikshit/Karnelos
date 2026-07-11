# Karnelos OS — Roadmap

## The AI-Native Operating System

Karnelos is a new kind of operating system. The kernel is real, working Rust code that
manages hardware, memory, interrupts, and I/O. On top of it runs a local LLM (via a host
daemon) that acts as the user's interface to generate and deploy new code.

**The core idea:** You don't install apps. You describe what you need, and the LLM
generates the code — optimized for your exact hardware and your exact workflow.

### Philosophy

- **The kernel is the foundation.** It ships with the OS. It boots, manages memory,
  handles interrupts, drives I/O. You never need to touch it unless you want to.
- **Everything else is generated.** Apps, tools, filesystems, key bindings,
  document formats — all produced by the local LLM to fit your hardware and your needs.
- **Local-first, private.** The LLM runs on your machine via Ollama.
  No data leaves your system.
- **You own everything.** Every generated file, every app, every configuration is yours
  to read, modify, and share.

### Quick Links

| Document | Description |
|---|---|
| [scope.md](scope.md) | Project scope, what's in and out |
| [architecture.md](architecture.md) | System architecture and design |
| [phases.md](phases.md) | Phase-by-phase implementation plan |
| [tech-stack.md](tech-stack.md) | Technology choices and rationale |

### Current Status

**Phases 0-2: Complete.** The kernel boots, manages memory, handles interrupts,
drives a keyboard and serial I/O, and provides a shell with VGA+serial output.
The build system uses a custom `mkimage` tool with bootloader 0.11.15 (patched
for cross-compilation on aarch64 hosts and rustc 1.90 target spec compatibility).

**Phase 3 (LLM Integration): Partial.** Instead of embedding the LLM inside the kernel
(original plan), we use a pragmatic host-side daemon that communicates with the kernel
over a second serial port (COM2). The full gen→build→reboot→run cycle works end-to-end.

#### Implemented
- Physical frame allocator (bitmap, 4GB max)
- Heap allocator (10MB via `linked_list_allocator`)
- IDT, PIC, PS/2 keyboard driver with ring buffer
- Serial I/O (COM1 user terminal, COM2 daemon link)
- VGA text mode with scrolling console (80x25)
- Shell with command dispatch (help, memory, echo, info, gen, run, reboot, test-heap)
- Daemon: TCP server on :12345, calls Ollama, writes generated.rs, rebuilds kernel
- Generator: standalone CLI for LLM code generation
- Full cycle: `gen` → daemon → Ollama → build → `reboot` → `run` executes new code

#### Next Up
- Userspace execution (ring 3, TSS, syscalls)
- virtio-blk driver + filesystem (Phase 4)
- Improved LLM prompt engineering and error recovery

See [phases.md](phases.md) for the detailed implementation plan.
