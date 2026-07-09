.PHONY: all build run run-nographic run-debug run-cocoa run-smp run-daemon generate clean daemon-build

KERNEL_IMG = kernel/target/x86_64-unknown-none/debug/bootimage-karnelos-kernel.bin

all: build

build:
	cd kernel && BOOTLOADER_FEATURES=map_physical_memory cargo bootimage --target x86_64-unknown-none

QEMUFLAGS = -drive format=raw,file=$(KERNEL_IMG) -m 4G -cpu max -nic none

# Serial layout:
#   COM1 (0x3F8) -> stdio: user terminal
#   COM2 (0x2F8) -> TCP localhost:12345: daemon connection
DAEMON_PORT = 12345
QEMUFLAGS_SERIAL = -serial stdio -serial tcp:127.0.0.1:$(DAEMON_PORT)

# Graphical QEMU window (macOS native display)
run: build
	qemu-system-x86_64 $(QEMUFLAGS) $(QEMUFLAGS_SERIAL) -display cocoa

# Terminal mode (serial + VGA rendered as ANSI)
run-nographic: build
	qemu-system-x86_64 $(QEMUFLAGS) $(QEMUFLAGS_SERIAL) -nographic -no-reboot

# Debug console mode (uses Bochs debug port 0xE9)
run-debug: build
	qemu-system-x86_64 $(QEMUFLAGS) -debugcon stdio -display none -no-reboot

# 4 CPU cores
run-smp: build
	qemu-system-x86_64 $(QEMUFLAGS) $(QEMUFLAGS_SERIAL) -smp 4 -nographic -no-reboot

# Start daemon and QEMU together
run-daemon: build daemon-build
	@echo "Starting daemon (background) and QEMU..."
	(cd daemon && cargo run --release &) && \
	sleep 2 && \
	qemu-system-x86_64 $(QEMUFLAGS) $(QEMUFLAGS_SERIAL) -smp 4 -nographic -no-reboot; \
	kill %1 2>/dev/null || true

# Quick smoke test (boots and exits after 10s)
run-test: build
	gtimeout 10 qemu-system-x86_64 $(QEMUFLAGS) -nographic -no-reboot 2>&1; echo "---"

# Build the daemon
daemon-build:
	cd daemon && cargo build --release

generate:
	cd generator && cargo run -- "$(PROMPT)"

clean:
	cd kernel && cargo clean
	cd generator && cargo clean
	cd daemon && cargo clean
	rm -f $(KERNEL_IMG)
