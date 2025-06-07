#!/bin/bash

set -e

echo "Building NowWrite for UEFI..."

# Create output directory
mkdir -p output

# Install rust target if not present
rustup target add x86_64-unknown-uefi

# Build for UEFI
cargo build --release --target x86_64-unknown-uefi --no-default-features --features uefi

# Copy the EFI file
cp target/x86_64-unknown-uefi/release/nowwrite.efi output/NowWrite.efi

echo "UEFI build complete: output/NowWrite.efi"

# Optionally create a bootable drive structure
if [ "$1" = "--create-boot" ]; then
    echo "Creating bootable drive structure..."
    mkdir -p output/boot/EFI/BOOT
    cp output/NowWrite.efi output/boot/EFI/BOOT/BOOTX64.EFI
    echo "Bootable structure created in output/boot/"
    echo "Copy contents of output/boot/ to FAT32 USB drive to boot"
fi