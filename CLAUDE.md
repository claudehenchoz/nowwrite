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
./scripts/run-qemu.sh                 # PowerShell: scripts/run-qemu.ps1

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

- `Platform` trait (`src/platform/mod.rs`) covers input (`KeyEvent`), file I/O, `now() -> Timestamp`, and battery. Both platforms produce the same `KeyEvent`s:
  - Ctrl shortcuts (`CtrlS`…`CtrlQ`) and `Help` (F1);
  - `Shift{Up,Down,Left,Right,Home,End}` for selection;
  - `CtrlLeft`/`CtrlRight` for word movement, and `CtrlShiftLeft`/`CtrlShiftRight` to select by word;
  - F-key aliases (F2 save, F3 open, F4 new, F5 layout, F10 quit), for firmware that drops Ctrl.

  `init()` returns the platform **and** a ratatui `Backend`. On UEFI, `REMAP_KEYBOARD_LAYOUT` is true because firmware delivers US-layout chars (plus `KeyEvent::AltGr(c)` when right Alt is held). `SOFTWARE_CURSOR` is also true there: the editor paints and blinks its own block cursor, driven by `KeyEvent::Tick` (every 500 ms on UEFI, every 1 s idle in the terminal, which also gives idle backups). After any key the editor skips one tick (`skip_tick`), so the cursor stays visible for at least a full blink period while typing. Paths passed to the platform are `/`-separated and relative to the document root (`"."`, `".nowwrite/x.bak"`).
- `editor::Editor` (`src/editor/mod.rs`) is the single owner of the platform and the `ratatui::Terminal`. Its loop is: backup check → `terminal.draw` → `read_key` → layout translation → dispatch by `Mode`:
  - `Editing`, `Files(FileModal)` and `Keyboard(KeyboardModal)`;
  - `Confirm(Pending)`: the "save changes?" prompt;
  - `Help(Box<Mode>)`: the F1 overlay, which restores the previous mode.

  Anything that replaces or closes the document (Ctrl+L/N/Q) goes through `request(Pending)`, which asks first when the buffer is modified. Saving an unnamed document opens the Save As prompt. `after_save` continues the pending action once the save succeeds. Esc never quits. Modals are state + `render` + `handle_key`, drawn as overlays. Renderers return the cursor position instead of setting it, so `Editor::draw` can choose the hardware or software cursor. File-operation errors go to the status-bar `message` instead of aborting.
- `editor::buffer::TextBuffer` stores lines with **char-index** columns, converted to byte offsets internally. Undo/redo records `Insert`/`Delete { position, text }` and replays them through raw helpers that don't touch the undo stack. Consecutive typing merges into one undo step per word (`merge_open`); cursor moves end a step. `toggle_wrap` adds or removes `*`/`**` around the selection or word: `*` = italic (also `***`), `**` = bold. It keeps the cursor on the same letter, or keeps the same text selected, so pressing the shortcut again undoes it.
  - Word movement (`word_left`/`word_right`) works on buffer lines, not screen rows.
  - Ctrl+Right skips the rest of the word, then spaces, punctuation and markdown markers, stopping at the next word start. At the end of a line it goes to the next line's start.
  - Ctrl+Left mirrors that, going to the previous line's end from column 0.
  - A word is alphanumerics plus `'` and `-` (`is_word_char`), so `don't` is one word. The same rule decides which word Ctrl+B/Ctrl+I wrap; keep the two in sync.
- `editor::markdown::highlight` splits a line into styled spans **including** the `*`/`#` markers, so screen columns equal cursor columns. Don't strip markers when rendering.
- `file/` uses free functions taking `&mut P`. `manager` handles list/load/save. `BackupManager` writes `.nowwrite/<name>.<YYYYMMDD_HHMMSS>.bak` every 5 min and before each save. `config/keyboard.rs` stores layouts as static tables mapping US chars to an `Output` (`Char` or `Dead` accent), with separate normal and AltGr tables. `KeyboardConfig::translate` handles dead-key composition. The selected layout is persisted to `.nowwrite/.keyboard.config`. Only en-US and de-CH exist, per PROMPT.md. OVMF reports nothing for the ISO key (`<` `>` `\`), so de-CH maps AltGr+`,` `.` `-` to `<` `>` `\`.
- `ui/editor_view.rs` soft-wraps lines into screen `Row`s (`wrap_line`/`layout`, unit-tested). Up/Down/PageUp/PageDown/Home/End move by screen row via `EditorView::vertical`/`row_edge`, and the editor keeps a `goal_x` column. The file still stores one line per paragraph.
- `ui/` has the theme helpers (in `ui/mod.rs`), `editor_view`, `status_bar` and `modal` (file box, Save As/New name prompt, layout picker, confirm prompt, help overlay).
  - Use only the `ui/mod.rs` theme styles and glyphs from ASCII, box drawing and arrows. That's what the UEFI console can show.
  - Styles: plain grey text; bright white for headings, bold and italic; `muted()` dark grey for hints.
  - The grey background is reserved for the status bar, the selection and the software cursor. Don't use it for text styling, or the cursor disappears.
- `ui/status_bar.rs`: the left side shows the document name (`[new document]` if unnamed) with ` (modified)` and word/char counts. The counts drop out while a message is shown or space is short. The right side, from `StatusInfo`, shows the message, pending dead key (`^ + letter`), layout code (UEFI only), `Ln x, Col y`, battery and `F1 Help`.
- `platform/terminal.rs` uses crossterm via the `ratatui::crossterm` re-export, not a direct dependency. `Drop` and a panic hook restore the terminal.
- `platform/uefi/` targets uefi 0.41 (global allocator + panic handler come from crate features).
  - `backend.rs` implements ratatui `Backend` over Simple Text Output. It batches same-color runs, maps any background to LightGray with black text, and skips the bottom-right cell, which would scroll the screen.
  - `input.rs` prefers `InputEx`, which reports Ctrl, Shift and AltGr state, then falls back to control chars. Without InputEx there is no Shift selection or AltGr, but the F-keys still work. It waits on the key event and a periodic timer event, and returns `Tick` when the timer fires.
  - The firmware cursor is disabled because OVMF's doesn't blink and gets erased by redraws.
  - `mod.rs` uses `uefi::fs::FileSystem` on the image's boot volume and converts paths to `\`-rooted UEFI paths. Don't use `FileSystem::create_dir_all`: for a root-level path it walks up to an empty parent and fails with a path error. `create_directory` creates each level itself.
  - Firmware quirks found in QEMU/OVMF:
    - The console splitter returns UNSUPPORTED from `set_cursor_position` when a narrower attached console can't reach a column, so cursor moves are best-effort.
    - `output_string` warnings (e.g. unknown glyph) are not fatal.

### UX conventions

These came out of a first-time-user review; keep them when adding features.
- **Every new shortcut gets:**
  - an F-key alias if it's essential;
  - a line in the F1 help overlay (`modal::render_help`);
  - a row in the README key table;
  - a mapping in both `platform/terminal.rs` and `platform/uefi/input.rs`.
- **Anything that replaces or closes the document goes through `Editor::request(Pending)`,** so unsaved changes are never dropped silently. Never pick a file name for the user; use the name prompt (`FileModal::naming`).
- **Esc only closes boxes.** Quitting is Ctrl+Q / F10.
- **Report outcomes and errors through `Editor::message`.** It clears on the next key. Keep messages short: the status bar is one line, often 80 columns.
- **Modals say what keys do** in their bottom border, e.g. `Enter Open  Esc Cancel`.

### Testing under QEMU

`run-qemu.sh` / `run-qemu.ps1` use `scripts/fatimg.py`, a stdlib-only FAT16 image tool (MBR + one partition, long file names, subdirectories):
- `create` builds `output/nowwrite-disk.img` from `output/drive/` plus `\EFI\BOOT\BOOTX64.EFI`.
- `extract --skip EFI` copies the image back to `output/drive/` after QEMU exits. It also runs at startup, to recover a run that was killed.

Don't use QEMU's `fat:rw:<dir>` drive: it doesn't reliably write changes back to the host.

Headless runs: `./scripts/run-qemu.sh -- -display none -monitor tcp:127.0.0.1:4444,server,nowait` (PowerShell: `-QemuArgs '-display','none','-monitor','tcp:127.0.0.1:4444,server,nowait'`). Drive them through the monitor:
- `sendkey ctrl-s`, `sendkey f4` and similar send keys;
- `screendump file.ppm` captures the screen; convert it to PNG to view;
- `quit` ends QEMU, after which the script syncs files back.

The firmware clock is UTC, so backup timestamps in the VM are UTC.

### Not yet implemented (from PROMPT.md)

- Battery level (needs ACPI/EC access).
- Turning off radios to save power.
- Sleep on lid close.
- OVMF's font lacks some glyphs, e.g. `€`, which shows as a box. The characters are still saved correctly.
