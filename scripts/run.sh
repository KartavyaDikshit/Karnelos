#!/bin/bash
# Karnelos OS - Self-Hosted Runner
# Bundles the daemon and QEMU together for a seamless experience
set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_DIR="$( dirname "$SCRIPT_DIR" )"

echo "=== Karnelos OS - Self-Hosted Runner ==="
echo ""

# Check Ollama status
if ! curl -s http://localhost:11434/api/version > /dev/null 2>&1; then
    echo "WARNING: Ollama is not running! Start it with: ollama serve"
    echo "LLM-based code generation will not work."
    echo ""
fi

# Build everything
echo "[1/3] Building kernel..."
make -C "$PROJECT_DIR" build

echo "[2/3] Building userspace apps..."
make -C "$PROJECT_DIR" userspace-bins

echo "[3/3] Building daemon..."
make -C "$PROJECT_DIR" daemon-build

echo ""
echo "=== Starting daemon (background) and QEMU ==="
echo ""

# Kill daemon from previous runs
DAEMON_PID=$(pgrep -f "daemon/target/release" 2>/dev/null || true)
if [ -n "$DAEMON_PID" ]; then
    echo "Killing previous daemon instance..."
    kill -9 $DAEMON_PID 2>/dev/null || true
    sleep 1
fi

# Start the daemon
(cd $PROJECT_DIR/daemon && cargo run --release &)
DAEMON_PID=$!
sleep 2

# Start QEMU in restart loop
echo "QEMU ready on serial console. The daemon is running in background."
echo "To exit: Ctrl-C"
echo ""

# Enter QEMU restart loop
while true; do
    qemu-system-x86_64 \
        -drive format=raw,file=$PROJECT_DIR/kernel/target/x86_64-unknown-none/debug/bootimage-karnelos-kernel.bin \
        -drive file=$PROJECT_DIR/storage.img,format=raw,if=ide,index=2 \
        -m 4G \
        -cpu max \
        -nic none \
        -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
        -serial mon:stdio \
        -serial tcp:127.0.0.1:12345 \
        -nographic \
        -no-reboot || true
    echo "--- QEMU exited (restarting...) ---"
    sleep 1
done
