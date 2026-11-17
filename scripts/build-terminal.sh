#!/bin/bash
# Build the terminal version (Linux/macOS) -> output/nowwrite
set -e

mkdir -p output
cargo build --release --bin nowwrite
cp target/release/nowwrite output/nowwrite
chmod +x output/nowwrite

echo "Terminal build complete: output/nowwrite"
