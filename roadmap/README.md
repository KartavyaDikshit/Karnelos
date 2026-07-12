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

**Phases 0-5b: Complete.** The kernel boots, manages memory, handles interrupts,
drives keyboard and serial I/O, provides a shell, runs ring-3 ELF apps, and persists
data to a flat filesystem. The build system uses a custom `mkimage` tool with
bootloader 0.11.15.

#### Implemented
- Physical frame allocator (bitmap, 4GB max)
- Heap allocator (10MB via `linked_list_allocator`)
- IDT, PIC, PS/2 keyboard driver with ring buffer
- Serial I/O (COM1 user terminal, COM2 daemon link)
- Framebuffer console with 8×8 bitmap font (80×25 character grid)
- Shell with command dispatch (help, memory, echo, info, gen, run, user, app, storage, reboot)
- Daemon: TCP server on :12345, calls Ollama, generates userspace ELF
- Generator: standalone CLI for LLM code generation
- Full cycle: `gen` → daemon → Ollama → build → stream ELF → `run` executes in ring 3
- Ring 3 userspace: GDT with ring 0/3 segments, TSS, int 0x80 syscalls, iretq
- ELF loader: parses PIE ELF, maps PT_LOAD segments, applies relocations
- Per-process page tables (clone kernel upper half, user lower half)
- ATA PIO block driver + flat filesystem (format, read, write, ls)
- `app save` / `app run` — persist and load generated ELFs from storage
- COM2 ACK flow control (256-byte chunks) for reliable ELF streaming
- Framebuffer console with 8×8 bitmap font (replaces VGA text mode)

#### Next Up
- Showcase apps (calendar, todo, editor) — Phase 5c
- Self-improving OS with performance feedback — Phase 6
- Self-hosted image with in-kernel LLM — Phase 7

See [phases.md](phases.md) for the detailed implementation plan.
