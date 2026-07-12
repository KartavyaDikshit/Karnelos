# Karnelos OS — Project Scope

## What Karnelos Is

Karnelos is a complete operating system where:

1. **The kernel is a real, working Rust no_std kernel** that manages x86-64 hardware:
   memory, processes, devices, interrupts, and persistent storage.

2. **A local LLM (via host daemon + Ollama)** generates, compiles, and deploys
   user-requested software in real time. The daemon communicates with the kernel
   over a second serial port (COM2).

3. **Everything beyond the base kernel is generated.** Apps, tools, filesystem layouts,
   IPC protocols, key bindings, compilers, document formats — all produced by the LLM
   to fit the exact hardware and the exact user.

4. **The system improves with use.** Persistent storage preserves user data and
   generated apps across reboots. Future phases will add performance feedback loops
   and user context databases.

5. **Hardware optimization is a first-class concern.** The kernel runs on real x86-64
   hardware with memory management, interrupt handling, and device drivers.

## What's In Scope

### The Base Kernel (shipped with the OS)
- x86-64 boot (bootloader crate, BIOS)
- Physical and virtual memory management
- Single process model (multitasking deferred)
- Interrupt handling (PIC, IDT)
- Device drivers: UART serial (COM1+COM2), PS/2 keyboard, ATA PIO
- Persistent storage (ATA PIO block device → flat filesystem)
- Syscall interface for user-space programs (int 0x80)
- Sandbox for executing generated code (Ring 3)
- Framebuffer console with bitmap font

### The LLM System Service (Host Daemon)
- Ollama integration (qwen2.5-coder:1.5b)
- Code generation, compilation, and deployment pipeline
- ELF streaming over TCP/COM2 with ACK flow control
- Guardrails: code fence stripping, build error detection
- Console/CLI interface (the shell)

### Generated Components
- User applications (calendar, todo, editor, etc.)
- Custom file formats and filesystem layouts
- Custom compilers/parsers (e.g., custom LaTeX variant)
- Key bindings and input handling
- IPC protocols between generated components
- Filesystem organization (directory structure, naming conventions)

### User Experience
- Boot → shell prompt → user types `gen <prompt>` or shell commands
- Real-time code generation, compilation, and execution
- Persistent storage across reboots
- Ability to save and run generated apps from storage

## What's Out of Scope (for now)

### Phase 0-5b
- GUI / graphical display (framebuffer console only)
- Networking (beyond local loopback for LLM)
- Multi-user support
- GPU acceleration for the LLM (CPU inference only)
- Binary compatibility with Linux/POSIX
- Multitasking / preemptive scheduling

### Phase 6+
- In-kernel LLM inference
- SMP optimization and advanced scheduling
- Networking (TCP/IP stack for future cloud use)
- Custom microkernel architecture (if we migrate from monolithic)

## Target Hardware

### Development (QEMU VM)
- x86-64, 4 cores, 4GB RAM
- CPU: Skylake-Server (AVX2)
- Devices: UART (COM1+COM2), PS/2, ATA PIO (IDE)

### Future (bare metal)
- x86-64 with UEFI
- Range: 2GB-64GB RAM, 2-32 cores
- Automatic detection and optimization per machine

## Constraints

- **No cloud dependency.** Everything runs locally. No API calls to external services.
- **The LLM must not break the kernel.** Guardrails enforce hardware safety rules.
- **Boot time under 2 minutes** on the target hardware (including LLM load + kernel gen).
- **Total shipping image under 4GB** (including model weights).
