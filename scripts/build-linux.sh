#!/bin/bash

set -e

echo "Building NowWrite for Linux..."

# Create output directory
mkdir -p output

# Build for Linux
cargo build --release --features linux

# Copy the binary
cp target/release/nowwrite output/nowwrite-linux

echo "Linux build complete: output/nowwrite-linux"

# Make it executable
chmod +x output/nowwrite-linux

echo "You can now run: ./output/nowwrite-linux"