.PHONY: all build run run-nographic run-debug run-cocoa run-smp generate clean

KERNEL_IMG = kernel/target/x86_64-unknown-none/debug/bootimage-karnelos-kernel.bin

all: build

build:
	cd kernel && cargo bootimage --target x86_64-unknown-none

QEMUFLAGS = -drive format=raw,file=$(KERNEL_IMG) -m 4G -cpu max -nic none

# Graphical QEMU window (macOS native display)
run: build
	qemu-system-x86_64 $(QEMUFLAGS) -display cocoa

# Terminal mode (serial + VGA rendered as ANSI)
run-nographic: build
	qemu-system-x86_64 $(QEMUFLAGS) -nographic -no-reboot

# Debug console mode (uses Bochs debug port 0xE9)
run-debug: build
	qemu-system-x86_64 $(QEMUFLAGS) -debugcon stdio -display none -no-reboot

# 4 CPU cores
run-smp: build
	qemu-system-x86_64 $(QEMUFLAGS) -smp 4 -nographic -no-reboot

# Quick smoke test (boots and exits after 10s)
run-test: build
	gtimeout 10 qemu-system-x86_64 $(QEMUFLAGS) -nographic -no-reboot 2>&1; echo "---"

generate:
	cd generator && cargo run -- "$(PROMPT)"

clean:
	cd kernel && cargo clean
	cd generator && cargo clean
	rm -f $(KERNEL_IMG)
