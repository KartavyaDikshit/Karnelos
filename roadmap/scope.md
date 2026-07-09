# Karnelos OS — Project Scope

## What Karnelos Is

Karnelos is a complete operating system where:

1. **The kernel is a real, working Rust no_std kernel** that manages x86-64 hardware:
   memory, processes, devices, interrupts, and persistent storage.

2. **A local LLM (the "Kernel AI")** runs as a privileged system service, listening
   on the console. It generates, compiles, and deploys user-requested software in
   real time.

3. **Everything beyond the base kernel is generated.** Apps, tools, filesystem layouts,
   IPC protocols, key bindings, compilers, document formats — all produced by the LLM
   to fit the exact hardware and the exact user.

4. **The system improves with use.** Persistent user context (SQLite + vector RAG)
   records preferences, past tasks, and performance data. The LLM consults this
   context to produce better results over time.

5. **Hardware optimization is a first-class concern.** Before generating any code,
   the LLM knows the CPU microarchitecture, cache topology, SIMD capabilities, and
   memory size. Generated code is compiled with `-march=native -O3` and profiled.

## What's In Scope

### The Base Kernel (shipped with the OS)
- x86-64 boot (Limine or UEFI)
- Physical and virtual memory management
- Process/thread scheduler
- Interrupt handling (APIC, I/O APIC)
- Device drivers: UART serial, PS/2 keyboard, virtio-blk, virtio-net
- Persistent storage (block device → filesystem)
- Syscall interface for user-space programs
- Sandbox for executing generated code (Ring 3)

### The LLM System Service
- Local LLM runtime (llama.cpp or candle, statically linked)
- Hardware detection and profiling
- User context database (SQLite + vector embeddings)
- Code generation, compilation, and deployment pipeline
- Guardrails: validation, static analysis, performance benchmarking
- Console/CLI interface (the "shell")

### Generated Components
- User applications (calendar, todo, editor, etc.)
- Custom file formats and filesystem layouts
- Custom compilers/parsers (e.g., custom LaTeX variant)
- Key bindings and input handling
- IPC protocols between generated components
- Filesystem organization (directory structure, naming conventions)

### User Experience
- Boot → LLM starts → user describes what they need
- Real-time code generation, compilation, and execution
- Persistent context across sessions
- Ability to modify or extend any generated component

## What's Out of Scope (for now)

### Phase 0-3
- GUI / graphical display (serial console only)
- Networking (beyond local loopback for LLM)
- Multi-user support
- GPU acceleration for the LLM (CPU inference only)
- Binary compatibility with Linux/POSIX

### Phase 4+
- GUI (VirtIO GPU, framebuffer)
- Networking (TCP/IP stack for future cloud use)
- SMP optimization and advanced scheduling
- Custom microkernel architecture (if we migrate from monolithic)

## Target Hardware

### Development (QEMU VM)
- x86-64, 4 cores, 4GB RAM
- CPU: Skylake-Server (AVX2)
- Devices: UART, PS/2, virtio-blk, virtio-net, virtio-gpu

### Future (bare metal)
- x86-64 with UEFI
- Range: 2GB-64GB RAM, 2-32 cores
- Automatic detection and optimization per machine

## Constraints

- **No cloud dependency.** Everything runs locally. No API calls to external services.
- **The LLM must not break the kernel.** Guardrails enforce hardware safety rules.
- **Boot time under 2 minutes** on the target hardware (including LLM load + kernel gen).
- **Total shipping image under 4GB** (including model weights).
