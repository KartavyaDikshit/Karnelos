.PHONY: all build run run-nographic run-smp generate clean

KERNEL_IMG = kernel/target/x86_64-unknown-none/debug/bootimage-karnelos-kernel.bin

all: build

build:
	cd kernel && cargo bootimage --target x86_64-unknown-none

run: build
	qemu-system-x86_64 \
		-drive format=raw,file=$(KERNEL_IMG) \
		-m 4G \
		-cpu max

run-nographic: build
	qemu-system-x86_64 \
		-drive format=raw,file=$(KERNEL_IMG) \
		-m 4G \
		-cpu max \
		-nographic \
		-no-reboot

run-smp: build
	qemu-system-x86_64 \
		-drive format=raw,file=$(KERNEL_IMG) \
		-m 4G \
		-cpu max \
		-smp 4 \
		-nographic \
		-no-reboot

generate:
	cd generator && cargo run -- "$(PROMPT)"

clean:
	cd kernel && cargo clean
	cd generator && cargo clean
	rm -f $(KERNEL_IMG)
