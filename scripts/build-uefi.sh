#!/bin/bash

set -e

echo "Building NowWrite for UEFI..."

# Create output directory
mkdir -p output

# Check if rustup is available
if ! command -v rustup >/dev/null 2>&1; then
    echo "ERROR: rustup is required to build UEFI targets"
    echo ""
    echo "To install rustup and build for UEFI:"
    echo "1. Install rustup: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo "2. Install UEFI target: rustup target add x86_64-unknown-uefi"
    echo "3. Re-run this script"
    echo ""
    echo "Creating placeholder UEFI file for now..."
    
    # Create a placeholder file
    cat > output/NowWrite.efi << 'EOF'
This is a placeholder UEFI file.
To build the actual UEFI binary, you need:
1. rustup installed
2. x86_64-unknown-uefi target added
3. Rust 1.75+ with UEFI support
EOF
    
    echo "Placeholder created: output/NowWrite.efi"
    echo "Note: This is not a functional UEFI binary"
    exit 1
fi

# Install UEFI target
echo "Installing x86_64-unknown-uefi target..."
rustup target add x86_64-unknown-uefi

# Check Rust version and handle uguid compatibility
RUST_VERSION=$(rustc --version | grep -o '[0-9]\+\.[0-9]\+' | head -1)
echo "Detected Rust version: $RUST_VERSION"

# Build for UEFI
echo "Building UEFI binary..."
if cargo build --release --bin nowwrite_uefi --target x86_64-unknown-uefi --no-default-features --features uefi; then
    # Copy the EFI file
    cp target/x86_64-unknown-uefi/release/nowwrite_uefi.efi output/NowWrite.efi
    echo "UEFI build complete: output/NowWrite.efi"
else
    echo "UEFI build failed. This may be due to:"
    echo "1. Missing UEFI target (install with: rustup target add x86_64-unknown-uefi)"
    echo "2. Incompatible Rust version (requires 1.75+)"
    echo "3. Missing dependencies"
    echo ""
    echo "The UEFI implementation is provided as a reference architecture."
    echo "For development and testing, use the Linux version: ./scripts/build-linux.sh"
    exit 1
fi

# Optionally create a bootable drive structure
if [ "$1" = "--create-boot" ]; then
    echo "Creating bootable drive structure..."
    mkdir -p output/boot/EFI/BOOT
    cp output/NowWrite.efi output/boot/EFI/BOOT/BOOTX64.EFI
    echo "Bootable structure created in output/boot/"
    echo "Copy contents of output/boot/ to FAT32 USB drive to boot"
fi