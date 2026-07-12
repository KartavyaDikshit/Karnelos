.PHONY: all build run run-nographic run-debug run-cocoa run-smp run-daemon generate clean daemon-build userspace userspace-test userspace-clean

KERNEL_IMG = kernel/target/x86_64-unknown-none/debug/bootimage-karnelos-kernel.bin
STORAGE_IMG = storage.img

all: build

# Create a 64MB persistent storage image if it doesn't exist
$(STORAGE_IMG):
	@if [ ! -f $(STORAGE_IMG) ]; then \
		echo "Creating $(STORAGE_IMG) (64MB)..."; \
		dd if=/dev/zero of=$(STORAGE_IMG) bs=1M count=64 status=none; \
	fi

build:
	cd kernel && cargo +nightly-2025-07-08 build --target x86_64-unknown-none
	cd tools/mkimage && cargo +nightly-2025-07-08 run

QEMUFLAGS = -drive format=raw,file=$(KERNEL_IMG) -drive file=$(STORAGE_IMG),format=raw,if=ide,index=2 -m 4G -cpu max -nic none -device isa-debug-exit,iobase=0xf4,iosize=0x04

# Serial layout:
#   COM1 (0x3F8) -> stdio: user terminal
#   COM2 (0x2F8) -> TCP localhost:12345: daemon connection
DAEMON_PORT = 12345
QEMUFLAGS_SERIAL = -serial mon:stdio -serial tcp:127.0.0.1:$(DAEMON_PORT)

# Graphical QEMU window (macOS native display)
run: build $(STORAGE_IMG)
	qemu-system-x86_64 $(QEMUFLAGS) $(QEMUFLAGS_SERIAL) -display cocoa

# Terminal mode (serial + VGA rendered as ANSI)
run-nographic: build $(STORAGE_IMG)
	qemu-system-x86_64 $(QEMUFLAGS) $(QEMUFLAGS_SERIAL) -nographic -no-reboot

# Debug console mode (uses Bochs debug port 0xE9)
run-debug: build $(STORAGE_IMG)
	qemu-system-x86_64 $(QEMUFLAGS) -debugcon stdio -display none -no-reboot

# 4 CPU cores
run-smp: build $(STORAGE_IMG)
	qemu-system-x86_64 $(QEMUFLAGS) $(QEMUFLAGS_SERIAL) -smp 4 -nographic -no-reboot

# Start daemon and QEMU together (auto-restarts after gen→reboot)
run-daemon: build daemon-build $(STORAGE_IMG)
	@echo "Starting daemon (background) and QEMU (restart loop)..."
	(cd daemon && cargo run --release &) && \
	sleep 2 && \
	while true; do \
		qemu-system-x86_64 $(QEMUFLAGS) $(QEMUFLAGS_SERIAL) -smp 4 -nographic -no-reboot || true; \
		echo "--- QEMU exited (restarting...) ---"; \
		sleep 1; \
	done

# Quick smoke test (boots and exits after 10s)
run-test: build $(STORAGE_IMG)
	gtimeout 10 qemu-system-x86_64 $(QEMUFLAGS) -nographic -no-reboot 2>&1; echo "---"

# Build the daemon
daemon-build:
	cd daemon && cargo build --release

generate:
	cd generator && cargo run -- "$(PROMPT)"

# Build a userspace ring-3 app (PIE ELF) for the kernel loader.
USERSRC = userspace
USERSRC_BIN = $(USERSRC)/target/karnelos-user/debug/karnelos-user

userspace:
	cd $(USERSRC) && cargo +nightly-2025-07-08 build -Z build-std=core,alloc --target karnelos-user.json

# Inspect the produced ELF (should be PIE with no relocations).
userspace-test: userspace
	@echo "--- ELF header ---"; readelf -h $(USERSRC_BIN) | grep -E "Type|Entry"
	@echo "--- Relocations (should be none) ---"; readelf -r $(USERSRC_BIN) | tail -3

userspace-clean:
	cd $(USERSRC) && cargo clean

clean:
	cd kernel && cargo clean
	cd generator && cargo clean
	cd daemon && cargo clean
	rm -f $(KERNEL_IMG) $(STORAGE_IMG)
