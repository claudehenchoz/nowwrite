# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

NowWrite is a distraction-free, text-mode Markdown editor in Rust. The primary target is a standalone UEFI application booted from a FAT32 USB drive (no OS); a Linux terminal build exists for developing and tweaking the UI. The original requirements live in `docs/PROMPT.md`, and the intended design (module layout, UI mockups, power management, boot process) is in `docs/nowwrite-design-doc.md`. Check both before adding features.

## Commands

The build scripts are bash. On this Windows machine, run them from Git Bash/WSL or call `cargo` directly.

```bash
# Linux/dev build (default feature = "linux") -> output/nowwrite-linux
./scripts/build-linux.sh
cargo build --features linux

# UEFI build -> output/NowWrite.efi (add --create-boot for output/boot/EFI/BOOT/BOOTX64.EFI)
./scripts/build-uefi.sh
cargo build --release --bin nowwrite_uefi --target x86_64-unknown-uefi --no-default-features --features uefi

# Tests (unit tests in src/editor/markdown.rs and src/config/keyboard.rs)
cargo test
cargo test markdown            # filter by module/test name
cargo test editor::markdown::tests::<name>

# Write the bootable USB (formats the device, so never run it without explicit user approval)
sudo ./scripts/create-boot-drive.sh /dev/sdX NOWWRITE
```

The `linux` and `uefi` features are mutually exclusive. Always pass `--no-default-features` when enabling `uefi`. Each binary is gated by `required-features`: `nowwrite` (`src/main.rs`) needs `linux` and `nowwrite_uefi` (`src/uefi_main.rs`) needs `uefi`.

## Architecture

All editor logic is generic over the `Platform` trait in `src/platform/mod.rs`. The trait covers key input (`KeyEvent`), char-cell drawing with `TextAttribute` (bright/grey background), file I/O, battery status, and power mode. Platform-specific code should stay behind this trait.

- `editor::Editor<P: Platform + Clone>` (`src/editor/mod.rs`) owns the main loop: backup check, render, read key, keyboard-layout remap, dispatch. It **clones the platform value** into each subsystem (`Renderer`, `FileManager`, `BackupManager`, `KeyboardConfig`), so platform implementations have to be cheap, shareable handles. `LinuxPlatform` is an empty struct, and any UEFI handle state has to survive cloning the same way.
- `editor::buffer::TextBuffer` stores `Vec<String>` lines, the cursor and selection, the modified flag, and undo/redo. `editor::markdown` parses lines into highlight spans (headings, `*italic*`, `**bold**`), which `ui::renderer` draws as bright text.
- `ui/` contains the renderer, the bottom status bar (filename, word/char counts, saved indicator, battery), and the modals (`FileModal` for `.md` file selection/creation, `KeyboardModal`), which return `ModalResult`.
- `file/` has `FileManager` for load/save and `BackupManager`, which writes timestamped copies (`<name>.YYYYMMDD_HHMMSS.bak`) into `.nowwrite/` every 5 minutes and before each Ctrl+S save.
- `config/keyboard.rs` holds the keyboard layouts as char remaps applied to `KeyEvent::Char`. The selected layout is persisted to `.nowwrite/.keyboard.config`.
- `platform/linux/` uses ANSI escapes with the alternate screen plus `libc`/`terminal_size`. `platform/uefi/` is split into `display`, `input`, `filesystem`, `power`.

### Current state of the UEFI path

The UEFI target is incomplete:
- `src/uefi_main.rs` is `#![no_std]`/`#![no_main]` and only initializes `uefi_services`. It does not construct `Editor` yet.
- `UefiPlatform::init()` is `todo!()`.
- The shared code (the `Platform` trait, editor, and file modules) uses `std::io::Result`, `std::path`, `String`/`Vec` from std, and `chrono`. None of these work under `no_std`. Getting the editor running on UEFI means moving the shared code to `alloc`/`core` types or a custom error type.

The Linux build is the working path for exercising editor behavior.
