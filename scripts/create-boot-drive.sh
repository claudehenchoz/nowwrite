#!/bin/bash

set -e

if [ $# -eq 0 ]; then
    echo "Usage: $0 <device> [label]"
    echo "Example: $0 /dev/sdb1 NOWWRITE"
    echo ""
    echo "WARNING: This will FORMAT the specified device!"
    echo "Make sure you specify the correct device."
    exit 1
fi

DEVICE=$1
LABEL=${2:-NOWWRITE}

echo "Creating bootable NowWrite drive on $DEVICE with label $LABEL"
echo ""
echo "WARNING: This will ERASE ALL DATA on $DEVICE"
read -p "Are you sure? (y/N): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 1
fi

# Check if device exists
if [ ! -b "$DEVICE" ]; then
    echo "Error: Device $DEVICE does not exist or is not a block device"
    exit 1
fi

# Build UEFI version first
echo "Building UEFI version..."
./scripts/build-uefi.sh

# Format as FAT32
echo "Formatting $DEVICE as FAT32..."
sudo mkfs.fat -F 32 -n "$LABEL" "$DEVICE"

# Create temporary mount point
MOUNT_POINT=$(mktemp -d)

# Mount the device
echo "Mounting $DEVICE to $MOUNT_POINT..."
sudo mount "$DEVICE" "$MOUNT_POINT"

# Create EFI boot structure
echo "Creating EFI boot structure..."
sudo mkdir -p "$MOUNT_POINT/EFI/BOOT"
sudo cp output/NowWrite.efi "$MOUNT_POINT/EFI/BOOT/BOOTX64.EFI"

# Create a simple startup.nsh for easy launching
echo "Creating startup.nsh..."
echo "EFI\\BOOT\\BOOTX64.EFI" | sudo tee "$MOUNT_POINT/startup.nsh" > /dev/null

# Copy some example files
echo "Creating example documents..."
sudo mkdir -p "$MOUNT_POINT/documents"

cat << 'EOF' | sudo tee "$MOUNT_POINT/documents/welcome.md" > /dev/null
# Welcome to NowWrite

This is a **distraction-free** markdown editor that runs directly from UEFI firmware.

## Features

* Minimal interface designed for focus
* Automatic backups every 5 minutes
* UTF-8 support for international characters
* Multiple keyboard layouts
* No operating system required

## Getting Started

1. Use **Ctrl+L** to load/create files
2. Use **Ctrl+S** to save your work
3. Use **Ctrl+K** to change keyboard layout
4. Use **Ctrl+N** to create a new file

## Markdown Support

You can use *italic* and **bold** text formatting.

### Heading levels 1-6 are supported

Start writing and let your thoughts flow without distractions!
EOF

cat << 'EOF' | sudo tee "$MOUNT_POINT/documents/example.md" > /dev/null
# My Ideas

## Project Planning

* Research phase
* Design mockups
* Implementation
* Testing

## Notes

This is a **great** tool for *focused* writing sessions.

### Why NowWrite?

1. No distractions from notifications
2. Ultra-fast boot time
3. Reliable auto-backup
4. Works anywhere with UEFI
EOF

# Sync and unmount
echo "Finalizing..."
sudo sync
sudo umount "$MOUNT_POINT"
rmdir "$MOUNT_POINT"

echo ""
echo "✓ Bootable NowWrite drive created successfully!"
echo ""
echo "To use:"
echo "1. Boot from this USB drive"
echo "2. NowWrite will start automatically"
echo "3. Your documents will be in the 'documents' folder"
echo ""
echo "The drive is now ready to use!"