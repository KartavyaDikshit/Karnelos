# Karnelos OS — Technology Stack

## Kernel

| Component | Choice | Rationale |
|---|---|---|
| **Language** | Rust (no_std, nightly) | Memory safety without GC, LLVM backend, zero-cost abstractions |
| **Target** | `x86_64-unknown-none` | Standard bare-metal x86-64 target |
| **Bootloader** | `bootloader` crate v0.11.15 | Well-documented, handles BIOS, GDT, long mode transition, framebuffer |
| **CPU Architecture** | x86-64 | Ubiquitous, well-understood by LLMs, QEMU support |
| **Build Tool** | `mkimage` (custom) + `cargo` | Creates bootable disk image from kernel ELF using bootloader's `BiosBoot::create_disk_image` |

### Key Crates
- `x86_64` — x86-64 CPU primitives (GDT, IDT, page tables, MSRs)
- `uart_16550` — UART serial driver
- `bootloader` — Bootloader library (v0.11.15)
- `linked_list_allocator` — Heap allocator
- `acpi` — ACPI table parsing (for bare metal)

## LLM System Service (Host Daemon)

| Component | Choice | Rationale |
|---|---|---|
| **Inference Engine** | Ollama (host-side) | Simple integration, no in-kernel complexity |
| **Default Model** | Qwen2.5-Coder 1.5B | Good codegen quality, fits in 4GB RAM |
| **Communication** | TCP :12345 → COM2 serial | Reliable streaming with ACK flow control |

### Model Tiers
| Tier | Model | Size | RAM Req | Quality |
|---|---|---|---|---|
| Low | Qwen2.5-Coder 0.5B Q4 | ~350MB | 1GB+ | Basic codegen |
| Medium (default) | Qwen2.5-Coder 1.5B Q4 | ~1GB | 2.5GB+ | Good |
| High | DeepSeek-Coder 6.7B Q4 | ~4GB | 8GB+ | Excellent |
| Max | Qwen2.5-Coder 14B Q4 | ~8GB | 16GB+ | Best |

## Generated Code

| Language | Use Case | Compiler | Optimization |
|---|---|---|---|
| Rust | System components, apps | `rustc` | `-C target-cpu=native -C opt-level=3` |
| C | Low-level drivers | `clang` (LLVM) | `-march=native -O3 -flto` |
| (Future) | Any language LLM chooses | Bundled compiler | Per-task optimization |

## Development Toolchain

| Tool | Version | Purpose |
|---|---|---|
| Rust | nightly (1.99+) | Kernel development |
| `bootloader` crate | 0.11.15 | Bootable image creation via `mkimage` |
| QEMU | 11.0+ | VM for testing |
| `rust-lld` | Bundled | Linker for kernel ELF |
| LLVM | Bundled with Rust | Code generation backend |

## Host Development Environment

For Phase 0-6, development happens on the host machine:
- Rust code is written in `kernel/` directory
- LLM (Ollama) runs on the host
- Generated code is cross-compiled on the host
- QEMU boots the kernel image

The daemon (`daemon/`) orchestrates this:
```
User prompt → kernel shell → COM2 → daemon → Ollama → generated .rs → cargo build → ELF → COM2 → kernel
```

## Future (Phase 7+)

| Component | Target | Notes |
|---|---|---|
| **Self-hosting** | Everything runs inside the VM | LLM, compiler, generator all bundled in boot image |
| **Bare metal** | x86-64 UEFI | Replace `bootloader` crate with UEFI support |
| **GPU** | Vulkan/ROCm/CUDA | LLM inference acceleration |
| **Cloud** | Firecracker microVMs | Per-tenant hardware-optimized instances |
