#!/bin/bash
# Karnelos OS Bootstrap
# Sets up everything needed to run the OS self-hosted
set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_DIR="$( dirname "$SCRIPT_DIR" )"

echo "============================================"
echo " Karnelos OS - Bootstrap"
echo "============================================"
echo ""

# Step 1: Check prerequisites
echo "[1/7] Checking prerequisites..."

# Rust toolchain
if ! command -v cargo &> /dev/null; then
    echo "ERROR: Rust is not installed. Install from https://rustup.rs"
    exit 1
fi

# Nightly toolchain
if ! rustup toolchain list 2>/dev/null | grep -q "nightly-2025-07-08"; then
    echo "Installing Rust nightly-2025-07-08..."
    rustup install nightly-2025-07-08
fi

# QEMU
if ! command -v qemu-system-x86_64 &> /dev/null; then
    echo "Installing QEMU..."
    if [[ "$OSTYPE" == "darwin"* ]]; then
        brew install qemu
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        sudo apt-get install -y qemu-system-x86-64
    else
        echo "WARNING: Install QEMU manually"
    fi
fi

# Ollama
if ! command -v ollama &> /dev/null; then
    echo "Installing Ollama..."
    if [[ "$OSTYPE" == "darwin"* ]]; then
        brew install ollama
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        curl -fsSL https://ollama.com/install.sh | sh
    fi
fi

# Step 2: Install target
echo ""
echo "[2/7] Installing Rust target..."
rustup target add x86_64-unknown-none --toolchain nightly-2025-07-08
rustup component add llvm-tools-preview --toolchain nightly-2025-07-08

# Step 3: Pull LLM model
echo ""
echo "[3/7] Pulling LLM model (qwen2.5-coder:1.5b)..."
ollama pull qwen2.5-coder:1.5b

# Step 4: Build kernel
echo ""
echo "[4/7] Building kernel..."
make -C "$PROJECT_DIR" build

# Step 5: Build userspace apps
echo ""
echo "[5/7] Building userspace apps..."
make -C "$PROJECT_DIR" userspace-bins

# Step 6: Build daemon
echo ""
echo "[6/7] Building daemon..."
make -C "$PROJECT_DIR" daemon-build

# Step 7: Create storage image
echo ""
echo "[7/7] Creating storage image..."
if [ ! -f "$PROJECT_DIR/storage.img" ]; then
    dd if=/dev/zero of="$PROJECT_DIR/storage.img" bs=1M count=64 status=none
    echo "Created 64MB storage image"
fi

echo ""
echo "============================================"
echo " Bootstrap complete!"
echo ""
echo "Start the OS with:  make run-selfhosted"
echo "or:                scripts/run.sh"
echo "============================================"
