# Karnelos OS — Technology Stack

## Kernel

| Component | Choice | Rationale |
|---|---|---|
| **Language** | Rust (no_std, nightly) | Memory safety without GC, LLVM backend, zero-cost abstractions |
| **Target** | `x86_64-unknown-none` | Standard bare-metal x86-64 target |
| **Bootloader** | `bootloader` crate (via `bootimage`) | Well-documented, handles BIOS/UEFI, GDT, long mode transition |
| **CPU Architecture** | x86-64 | Ubiquitous, well-understood by LLMs, QEMU support |
| **Build Tool** | `bootimage` + `cargo` | Creates bootable disk image from kernel ELF |

### Key Crates
- `x86_64` — x86-64 CPU primitives (GDT, IDT, page tables, MSRs)
- `uart_16550` — UART serial driver
- `acpi` — ACPI table parsing (for bare metal)

## LLM System Service

| Component | Choice | Rationale |
|---|---|---|
| **Inference Engine** | llama.cpp (C++ → C FFI) or `candle` (Rust native) | Both are local-first, MIT/APACHE licensed |
| **Default Model** | Qwen2.5-Coder 1.5B (Q4 quantized, ~1GB) | Good codegen quality, fits in 4GB RAM |
| **User Context** | SQLite via `rusqlite` | Embedded, reliable, no server needed |
| **Vector Search** | `fastembed` (Rust) or custom cosine similarity | Lightweight local embeddings for RAG |

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
| `bootimage` | 0.10+ | Bootable image creation |
| QEMU | 11.0+ | VM for testing |
| `rust-lld` | Bundled | Linker for kernel ELF |
| LLVM | Bundled with Rust | Code generation backend |

## Host Development Environment

For Phase 0-6, development happens on the host machine:
- Rust code is written in `kernel/` directory
- LLM (Ollama) runs on the host
- Generated code is cross-compiled on the host
- QEMU boots the kernel image

The generator CLI (`generator/`) orchestrates this:
```
User prompt → generator → Ollama → generated .rs → cargo bootimage → QEMU
```

## Future (Phase 7+)

| Component | Target | Notes |
|---|---|---|
| **Self-hosting** | Everything runs inside the VM | LLM, compiler, generator all bundled in boot image |
| **Bare metal** | x86-64 UEFI | Replace `bootloader` with `limine` for UEFI support |
| **GPU** | Vulkan/ROCm/CUDA | LLM inference acceleration |
| **Cloud** | Firecracker microVMs | Per-tenant hardware-optimized instances |
