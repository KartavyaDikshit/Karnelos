# Karnelos OS — Roadmap

## The AI-Native Operating System

Karnelos is a new kind of operating system. The kernel is real, working Rust code that
manages hardware, memory, processes, and devices. On top of it runs a local LLM that
acts as the user's direct interface to the machine.

**The core idea:** You don't install apps. You describe what you need, and the LLM
generates the code — optimized for your exact hardware and your exact workflow.

### Philosophy

- **The kernel is the foundation.** It ships with the OS. It boots, manages memory,
  schedules processes, drives devices. You never need to touch it unless you want to.
- **Everything else is generated.** Apps, tools, compilers, filesystems, key bindings,
  document formats — all produced by the local LLM to fit your hardware and your needs.
- **Local-first, private.** The LLM runs on your machine. No data leaves your system.
- **User context is persistent.** The more you use Karnelos, the better it knows your
  preferences, your workflow, and your hardware. The OS adapts to you over time.
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

**Phase 0: Kernel Skeleton — in progress**

The base kernel boots in QEMU, prints to serial, and sets up the foundation for the LLM
generation system. See [phases.md](phases.md) for details.
