# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

NowWrite is a distraction-free, text-mode Markdown editor in Rust. The primary target is a standalone UEFI application booted from a FAT32 USB drive (no OS). A terminal build (Linux/macOS/Windows) runs the same editor for development. The original requirements live in `docs/PROMPT.md`, and the intended design is in `docs/nowwrite-design-doc.md`. Check both before adding features. The design doc predates the ratatui/`no_std` rewrite, so its module layout is out of date.

## Commands

```bash
# Terminal build (default feature "terminal") - works on Windows, Linux, macOS
cargo run
./scripts/build-terminal.sh         # -> output/nowwrite      (PowerShell: scripts/build-terminal.ps1)

# UEFI build -> output/NowWrite.efi; --create-boot / -CreateBoot adds output/boot/EFI/BOOT/BOOTX64.EFI
./scripts/build-uefi.sh --create-boot   # PowerShell: scripts/build-uefi.ps1 -CreateBoot
cargo build --release --bin nowwrite_uefi --target x86_64-unknown-uefi --no-default-features --features uefi

# Boot in QEMU from a writable disk image synced with output/drive/ (needs output/NowWrite.efi + Python 3)
./scripts/run-qemu.ps1

# Tests
cargo test
cargo test buffer                   # filter by module/test name
cargo test editor::buffer::tests::undo_redo_roundtrip

# Prove the core still builds without std (no platform feature)
cargo check --lib --no-default-features

# Write the bootable USB (formats the device, so never run it without explicit user approval)
sudo ./scripts/create-boot-drive.sh /dev/sdX NOWWRITE
```

The `terminal` and `uefi` features are mutually exclusive. Enabling both fails with a duplicate `panic_impl`. Binaries: `nowwrite` (`src/main.rs`, needs `terminal`) and `nowwrite_uefi` (`src/uefi_main.rs`, needs `uefi`). Before finishing a change, build both and run the `--no-default-features` check.

## Architecture

The crate is a library (`src/lib.rs`) plus two thin binaries. **The library is `no_std` + `alloc`.** Core code must use `alloc::`/`core::` types and the crate's `Error`/`Result` (`src/error.rs`), never `std::`. Only `platform/terminal.rs` may use `std`, behind `feature = "terminal"`. Tests get `std` through `cfg(test)`.

- `Platform` trait (`src/platform/mod.rs`) covers input (`KeyEvent`), file I/O, `now() -> Timestamp`, and battery. `init()` returns the platform **and** a ratatui `Backend`. On UEFI, `REMAP_KEYBOARD_LAYOUT` is true because firmware delivers US-layout chars (plus `KeyEvent::AltGr(c)` when right Alt is held). `SOFTWARE_CURSOR` is also true there: the editor paints and blinks its own block cursor, driven by `KeyEvent::Tick` (every 500 ms on UEFI, every 1 s idle in the terminal, which also gives idle backups). Paths passed to the platform are `/`-separated and relative to the document root (`"."`, `".nowwrite/x.bak"`).
- `editor::Editor` (`src/editor/mod.rs`) is the single owner of the platform and the `ratatui::Terminal`. Its loop is: backup check → `terminal.draw` → `read_key` → layout translation → dispatch by `Mode`:
  - `Editing`, `Files(FileModal)` and `Keyboard(KeyboardModal)`;
  - `Confirm(Pending)`: the "save changes?" prompt;
  - `Help(Box<Mode>)`: the F1 overlay, which restores the previous mode.

  Anything that replaces or closes the document (Ctrl+L/N/Q) goes through `request(Pending)`, which asks first when the buffer is modified. Saving an unnamed document opens the Save As prompt. `after_save` continues the pending action once the save succeeds. Esc never quits. Modals are state + `render` + `handle_key`, drawn as overlays. Renderers return the cursor position instead of setting it, so `Editor::draw` can choose the hardware or software cursor. File-operation errors go to the status-bar `message` instead of aborting.
- `editor::buffer::TextBuffer` stores lines with **char-index** columns, converted to byte offsets internally. Undo/redo records `Insert`/`Delete { position, text }` and replays them through raw helpers that don't touch the undo stack. Consecutive typing merges into one undo step per word (`merge_open`); cursor moves end a step. `toggle_wrap` adds or removes `*`/`**` around the selection or word.
- `editor::markdown::highlight` splits a line into styled spans **including** the `*`/`#` markers, so screen columns equal cursor columns. Don't strip markers when rendering.
- `file/` uses free functions taking `&mut P`. `manager` handles list/load/save/unused names. `BackupManager` writes `.nowwrite/<name>.<YYYYMMDD_HHMMSS>.bak` every 5 min and before each save. `config/keyboard.rs` stores layouts as static tables mapping US chars to an `Output` (`Char` or `Dead` accent), with separate normal and AltGr tables. `KeyboardConfig::translate` handles dead-key composition. The selected layout is persisted to `.nowwrite/.keyboard.config`. Only en-US and de-CH exist, per PROMPT.md. OVMF reports nothing for the ISO key (`<` `>` `\`), so de-CH maps AltGr+`,` `.` `-` to `<` `>` `\`.
- `ui/editor_view.rs` soft-wraps lines into screen `Row`s (`wrap_line`/`layout`, unit-tested). Up/Down/PageUp/PageDown/Home/End move by screen row via `EditorView::vertical`/`row_edge`, and the editor keeps a `goal_x` column. The file still stores one line per paragraph.
- `ui/` has the theme helpers (in `ui/mod.rs`), `editor_view`, `status_bar` and `modal` (file box, Save As/New name prompt, layout picker, confirm prompt, help overlay). Keep styles to Reset/White(+bold)/DarkGray backgrounds and glyphs to ASCII, box drawing and arrows. That is what the UEFI console can show.
- `platform/terminal.rs` uses crossterm via the `ratatui::crossterm` re-export, not a direct dependency. `Drop` and a panic hook restore the terminal.
- `platform/uefi/` targets uefi 0.41 (global allocator + panic handler come from crate features).
  - `backend.rs` implements ratatui `Backend` over Simple Text Output. It batches same-color runs, maps any background to LightGray with black text, and skips the bottom-right cell, which would scroll the screen.
  - `input.rs` prefers `InputEx`, which reports Ctrl and AltGr state, then falls back to control chars. F2–F5 are aliases for Ctrl+S/L/N/K. It waits on the key event and a periodic timer event, and returns `Tick` when the timer fires.
  - The firmware cursor is disabled because OVMF's doesn't blink and gets erased by redraws.
  - `mod.rs` uses `uefi::fs::FileSystem` on the image's boot volume and converts paths to `\`-rooted UEFI paths. Don't use `FileSystem::create_dir_all`: for a root-level path it walks up to an empty parent and fails with a path error. `create_directory` creates each level itself.
  - Firmware quirks found in QEMU/OVMF:
    - The console splitter returns UNSUPPORTED from `set_cursor_position` when a narrower attached console can't reach a column, so cursor moves are best-effort.
    - `output_string` warnings (e.g. unknown glyph) are not fatal.

### Testing under QEMU

`run-qemu.ps1` uses `scripts/fatimg.py`, a stdlib-only FAT16 image tool (MBR + one partition, long file names, subdirectories):
- `create` builds `output/nowwrite-disk.img` from `output/drive/` plus `\EFI\BOOT\BOOTX64.EFI`.
- `extract --skip EFI` copies the image back to `output/drive/` after QEMU exits. It also runs at startup, to recover a run that was killed.

Don't use QEMU's `fat:rw:<dir>` drive: it doesn't reliably write changes back to the host.

Headless runs: `./scripts/run-qemu.ps1 -QemuArgs '-display','none','-monitor','tcp:127.0.0.1:4444,server,nowait'`. Drive them through the monitor:
- `sendkey ctrl-s`, `sendkey f4` and similar send keys;
- `screendump file.ppm` captures the screen; convert it to PNG to view;
- `quit` ends QEMU, after which the script syncs files back.

The firmware clock is UTC, so backup timestamps in the VM are UTC.

### Not yet implemented (from PROMPT.md)

- Battery level (needs ACPI/EC access).
- Turning off radios to save power.
- Sleep on lid close.
- OVMF's font lacks some glyphs, e.g. `€`, which shows as a box. The characters are still saved correctly.
