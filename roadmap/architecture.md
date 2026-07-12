# Karnelos OS — Architecture

## System Layers

```
┌─────────────────────────────────────────────────────────────┐
│  USER INTERFACE                                              │
│  UART serial console (shell prompt)                          │
│  The user types commands, the OS responds with output        │
├─────────────────────────────────────────────────────────────┤
│  HOST DAEMON (host machine, TCP :12345)                      │
│  ┌───────────────────────────────────────────────────────┐   │
│  │ • Ollama integration (qwen2.5-coder:1.5b)           │   │
│  │ • Code generation pipeline (prompt → LLM → ELF)      │   │
│  │ • ELF streaming over TCP/COM2 with ACK flow control   │   │
│  │ • Build error detection + guardrails                  │   │
│  └───────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│  GENERATED COMPONENTS (Ring 3, user space)                   │
│  ┌─────────┐ ┌──────────┐ ┌───────────┐ ┌────────────────┐  │
│  │  Apps   │ │  Tools   │ │ Compilers │ │ Custom FS      │  │
│  │ (editor,│ │ (file mgr│ │ (latex,   │ │ (generated     │  │
│  │  calc)  │ │  search) │ │  MD→PDF)  │ │  on request)   │  │
│  └─────────┘ └──────────┘ └───────────┘ └────────────────┘  │
│  ┌────────────────────────────────────────────────────────┐   │
│  │  Userspace runtime (rt.rs): _start, syscall! macro,    │   │
│  │  bump allocator, mem ops, panic/alloc handlers          │   │
│  └────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│  KARNELOS BASE KERNEL (Ring 0, Rust no_std)                  │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────────┐  │
│  │ Memory   │ │ Process  │ │ Device   │ │ Syscall / IPC  │  │
│  │ Manager  │ │ Model    │ │ Drivers  │ │ Interface      │  │
│  │ (page    │ │ (single  │ │ (UART,   │ │ (int 0x80:     │  │
│  │  alloc)  │ │  process)│ │  PS/2,   │ │  exit, write,  │  │
│  │          │ │          │ │  ATA)    │ │  read, storage) │  │
│  │          │ │          │ │          │ │                │  │
│  └──────────┘ └──────────┘ └──────────┘ └────────────────┘  │
│  ┌────────────────────────────────────────────────────────┐   │
│  │  Bootloader (bootloader crate v0.11.15, BIOS boot)     │   │
│  │  Provides: framebuffer, physical memory map, page tables │   │
│  └────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│  HARDWARE (x86-64, QEMU VM → bare metal)                    │
│  CPU · RAM · UART (COM1+COM2) · PS/2 · ATA PIO               │
└─────────────────────────────────────────────────────────────┘
```

## Core Data Flow

### AI-Native App Generation
```
User: "gen print the numbers 1 through 5"
        │
        ▼
┌─────────────────────────┐
│  Shell (kernel)         │
│  Sends prompt over COM2 │
└─────────┬───────────────┘
          │
          ▼
┌─────────────────────────┐
│  Host Daemon (:12345)    │
│  1. Forward prompt to    │
│     Ollama               │
│  2. LLM generates code   │
│  3. Write userspace/     │
│     src/main.rs          │
│  4. cargo build (PIE ELF)│
│  5. Stream ELF over TCP  │
│     (256B chunks + ACK)  │
└─────────────────────────┘
          │
          ▼
┌─────────────────────────┐
│  Kernel (shell)         │
│  1. Receive ELF over COM2│
│  2. Parse ELF headers   │
│  3. Map PT_LOAD segments │
│  4. Clone page tables    │
│  5. iretq to ring 3     │
│  6. App runs, exits      │
│  7. Return to shell     │
└─────────────────────────┘
```

## The Kernel AI (LLM System Service)

### Current Architecture (Phase 0-5b)
The LLM runs as a **host-side daemon** communicating over a second serial port (COM2).
This provides the full AI-native OS loop without the complexity of running an LLM
in-kernel. The daemon:
- Listens on TCP :12345 for prompts from the kernel
- Forwards prompts to Ollama (qwen2.5-coder:1.5b)
- Generates userspace Rust code, builds it as a PIE ELF
- Streams the ELF back over TCP/COM2 with 256-byte chunked ACK flow control

### Future (Phase 7+)
- In-kernel LLM inference (llama.cpp or candle linked into kernel)
- Model weights loaded at boot (Q4 quantized)
- Hardware detection engine (CPUID, cache, RAM, SIMD)

### Guardrail Architecture

The LLM has layered constraints to prevent generating code that breaks the system:

**Layer 1 — Hardware Safety (immutable system prompt)**
- Cannot write outside kernel memory space
- Must use approved page allocator API
- Interrupt handlers must complete in <100µs
- Must validate all user input lengths
- Cannot disable interrupts for >50µs

**Layer 2 — Template Constraints**
- Critical structures (page tables, IDT, GDT) are fixed templates
- LLM calls predefined APIs on them, cannot redefine structures

**Layer 3 — Validation Harness**
- `cargo check` on every generated module
- Static analysis for: unsafe block limits, loop bounds, memory leaks
- Sandbox execution of critical paths before deployment
- Max 3 retries per module, then fallback to safe default

**Layer 4 — Ring Separation**
- Kernel (Ring 0) vs user apps (Ring 3)
- LLM generates code for both, but user code goes through defined syscalls
- Generated drivers are validated before Ring 0 deployment

**Layer 5 — Performance Feedback**
- After deployment, kernel profiles execution (cycles, cache misses, allocations)
- Metrics fed back to LLM for next iteration
- System improves at user's specific tasks over time

## Memory Layout

```
+------------------+ 0x0000000000000000
| Reserved         |
+------------------+ 0x0000000000100000 (1MB)
| Kernel (.text)   |
| Kernel (.rodata) |
| Kernel (.data)   |
| Kernel (.bss)    |
+------------------+ kernel_end
| Page tables      |
| Heap arena       |
+------------------+ heap_end
| ...              |
+------------------+ 0x00000000FFFFFFFF (4GB - 32-bit space)
| MMIO / devices   |
+------------------+ 0xFFFFFFFF80000000 (higher half)
```

## Boot Sequence

```
1. Bootloader (bootloader crate v0.11.15) loads kernel ELF from disk
2. Bootloader enters long mode, sets up page tables, framebuffer
3. Kernel entry (_start):
   a. Set up GDT/IDT/TSS
   b. Initialize serial ports (COM1 + COM2)
   c. Initialize memory manager (frame allocator, heap)
   d. Initialize PS/2 keyboard driver
   e. Initialize ATA PIO block driver + filesystem
   f. Present shell prompt to user
   g. (Optional) Host daemon listens on :12345 for gen commands
```
