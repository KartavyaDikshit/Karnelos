FROM rust:latest AS builder

RUN rustup install nightly-2025-07-08 && \
    rustup target add x86_64-unknown-none --toolchain nightly-2025-07-08 && \
    rustup component add llvm-tools-preview --toolchain nightly-2025-07-08

WORKDIR /karnelos
COPY . .

RUN apt-get update && apt-get install -y qemu-system-x86 && \
    make deploy && \
    dd if=/dev/zero of=storage.img bs=1M count=64

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y qemu-system-x86 curl && \
    curl -fsSL https://ollama.com/install.sh | sh && \
    apt-get clean

WORKDIR /karnelos
COPY --from=builder /karnelos/kernel/target/x86_64-unknown-none/debug/bootimage-karnelos-kernel.bin .
COPY --from=builder /karnelos/storage.img .
COPY scripts/run.sh .

EXPOSE 12345
CMD ["./run.sh"]
