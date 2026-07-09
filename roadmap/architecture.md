# Karnelos OS — Architecture

## System Layers

```
┌─────────────────────────────────────────────────────────────┐
│  USER INTERFACE                                              │
│  UART serial console (Phase 0-2) → Terminal UI (Phase 3+)    │
│  The user types goals, the OS responds with generated code   │
├─────────────────────────────────────────────────────────────┤
│  LLM SYSTEM SERVICE (Ring 0/1, privileged)                   │
│  ┌───────────────────────────────────────────────────────┐   │
│  │ • Local LLM inference engine (llama.cpp / candle)     │   │
│  │ • Hardware profiler (CPUID, cache, SIMD, RAM detect)  │   │
│  │ • User context + RAG (SQLite + vector embeddings)     │   │
│  │ • Code generator → compiler → deploy pipeline         │   │
│  │ • Validation + guardrail enforcement                  │   │
│  └───────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│  GENERATED COMPONENTS (Ring 3, user space)                   │
│  ┌─────────┐ ┌──────────┐ ┌───────────┐ ┌────────────────┐  │
│  │  Apps   │ │  Tools   │ │ Compilers │ │ Custom FS      │  │
│  │ (editor,│ │ (file mgr│ │ (latex,   │ │ (generated     │  │
│  │  calc)  │ │  search) │ │  MD→PDF)  │ │  on request)   │  │
│  └─────────┘ └──────────┘ └───────────┘ └────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│  KARNELOS BASE KERNEL (Ring 0, Rust no_std)                  │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────────┐  │
│  │ Memory   │ │ Sched-   │ │ Device   │ │ Syscall / IPC  │  │
│  │ Manager  │ │ uler     │ │ Drivers  │ │ Interface      │  │
│  │ (page    │ │ (coop →  │ │ (UART,   │ │ (read, write,  │  │
│  │  alloc)  │ │  preempt)│ │  PS/2,   │ │  exec, mmap)   │  │
│  │          │ │          │ │  virtio) │ │                │  │
│  └──────────┘ └──────────┘ └──────────┘ └────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│  HARDWARE (x86-64, QEMU VM → bare metal)                    │
│  CPU · RAM · UART · PS/2 · virtio-blk · virtio-net          │
└─────────────────────────────────────────────────────────────┘
```

## Core Data Flow

```
User: "build me a calendar with reminders"
        │
        ▼
┌─────────────────────────┐
│  LLM System Service     │
│                         │
│  1. Read user context    │
│     (past tasks, prefs) │
│  2. Read hardware profile│
│     (AVX2, 4 cores,     │
│      4GB RAM, cache)    │
│  3. Generate code        │
│     - calendar.rs       │
│     - storage.rs        │
│     - Cargo.toml        │
│  4. Validate code        │
│     (cargo check,       │
│      static analysis)   │
│  5. Compile              │
│     (rustc -march=...   │
│      -C opt-level=3)    │
│  6. Deploy to /apps/     │
│  7. Update user context  │
└─────────────────────────┘
        │
        ▼
User sees calendar running in terminal
```

## The Kernel AI (LLM System Service)

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
1. Limine loads kernel ELF from disk
2. Bootloader enters long mode, sets up page tables
3. Kernel entry (_start):
   a. Set up GDT/IDT/TSS
   b. Initialize serial port (UART)
   c. Initialize memory manager
   d. Initialize scheduler
   e. Load LLM weights + start inference engine
   f. Read user context from persistent storage
   g. Generate initial shell/boot-time components
   h. Present CLI to user
```
