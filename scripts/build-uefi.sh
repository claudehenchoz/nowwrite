#!/bin/bash
# Build the UEFI application -> output/NowWrite.efi
# --create-boot also lays out output/boot/EFI/BOOT/BOOTX64.EFI for a FAT32 drive.
set -e

rustup target add x86_64-unknown-uefi

mkdir -p output
cargo build --release --bin nowwrite_uefi --target x86_64-unknown-uefi --no-default-features --features uefi
cp target/x86_64-unknown-uefi/release/nowwrite_uefi.efi output/NowWrite.efi
echo "UEFI build complete: output/NowWrite.efi"

if [ "$1" = "--create-boot" ]; then
    mkdir -p output/boot/EFI/BOOT
    cp output/NowWrite.efi output/boot/EFI/BOOT/BOOTX64.EFI
    # The embedded JetBrains Mono font's licence travels with the binary.
    cp assets/fonts/OFL.txt output/boot/EFI/BOOT/OFL.txt
    echo "Boot layout created in output/boot/ (copy its contents to a FAT32 drive)"
fi
