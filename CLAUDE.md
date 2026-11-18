# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

NowWrite is a distraction-free Markdown editor in Rust. The primary target is a standalone UEFI application booted from a FAT32 USB drive (no OS). A terminal build (Linux/macOS/Windows) runs the same editor for development. The original requirements live in `docs/PROMPT.md`, and the intended design is in `docs/nowwrite-design-doc.md`. Check both before adding features. The design doc predates the ratatui/`no_std` rewrite, so its module layout is out of date.

## Commands

```bash
# Terminal build (default feature "terminal") - works on Windows, Linux, macOS
cargo run
./scripts/build-terminal.sh         # -> output/nowwrite      (PowerShell: scripts/build-terminal.ps1)

# UEFI build -> output/NowWrite.efi; --create-boot / -CreateBoot adds output/boot/EFI/BOOT/BOOTX64.EFI
./scripts/build-uefi.sh --create-boot   # PowerShell: scripts/build-uefi.ps1 -CreateBoot
cargo build --release --bin nowwrite_uefi --target x86_64-unknown-uefi --no-default-features --features uefi
# build.rs bakes the JetBrains Mono bitmap font only for UEFI builds (prints its size as a cargo warning)

# Boot in QEMU from a writable disk image synced with output/drive/ (needs output/NowWrite.efi + Python 3)
./scripts/run-qemu.sh                 # PowerShell: scripts/run-qemu.ps1

# Tests
cargo test
cargo test buffer                   # filter by module/test name
cargo test editor::buffer::tests::undo_redo_roundtrip

# Profile the UEFI main loop: writes .nowwrite/perf.log (count/avg/max per phase) on exit
cargo build --release --bin nowwrite_uefi --target x86_64-unknown-uefi --no-default-features --features uefi,perf

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
  - `CtrlT` (theme picker);
  - F-key aliases (F2 save, F3 open, F4 new, F5 layout, F6 theme, F10 quit), for firmware that drops Ctrl.

  `init()` returns the platform **and** a ratatui `Backend`. On UEFI, `REMAP_KEYBOARD_LAYOUT` is true because firmware delivers US-layout chars (plus `KeyEvent::AltGr(c)` when right Alt is held). `SOFTWARE_CURSOR` is also true there: the editor paints and blinks its own block cursor, driven by `KeyEvent::Tick` (every 500 ms on UEFI, every 1 s idle in the terminal, which also gives idle backups). After any key the editor skips one tick (`skip_tick`), so the cursor stays visible for at least a full blink period while typing. Paths passed to the platform are `/`-separated and relative to the document root (`"."`, `".nowwrite/x.bak"`).
- `editor::Editor` (`src/editor/mod.rs`) is the single owner of the platform and the `ratatui::Terminal`. Its loop is: backup check (only on `Tick` or every 64 keys, because reading the firmware clock is slow) → `terminal.draw` → `read_key` → layout translation → dispatch by `Mode`, then the same for every key `Platform::poll_key` says is already waiting (up to 32), so a burst of keys costs one frame:
  - `Editing`, `Files(FileModal)`, `Keyboard(KeyboardModal)` and `Theme(ThemeModal)`;
  - `Confirm(Pending)`: the "save changes?" prompt;
  - `Help(Box<Mode>)`: the F1 overlay, which restores the previous mode.

  Anything that replaces or closes the document (Ctrl+L/N/Q) goes through `request(Pending)`, which asks first when the buffer is modified. Saving an unnamed document opens the Save As prompt. `after_save` continues the pending action once the save succeeds. Esc never quits. Modals are state + `render` + `handle_key`, drawn as overlays. Renderers return the cursor position instead of setting it, so `Editor::draw` can choose the hardware or software cursor. File-operation errors go to the status bar instead of aborting: use `say()` for outcomes and `fail()` for errors (shown in the theme's error colour).
- `editor::buffer::TextBuffer` stores lines with **char-index** columns, converted to byte offsets internally. Undo/redo records `Insert`/`Delete { position, text }` and replays them through raw helpers that don't touch the undo stack. Consecutive typing merges into one undo step per word (`merge_open`); cursor moves end a step. `toggle_wrap` adds or removes `*`/`**` around the selection or word: `*` = italic (also `***`), `**` = bold. It keeps the cursor on the same letter, or keeps the same text selected, so pressing the shortcut again undoes it. `word_count`/`char_count` are running totals that `raw_insert`/`raw_remove` keep up to date (`uncount` before changing a line, `count_lines` after), so the status bar never recounts the document; keep any new line mutation inside those helpers.
  - Word movement (`word_left`/`word_right`) works on buffer lines, not screen rows.
  - Ctrl+Right skips the rest of the word, then spaces, punctuation and markdown markers, stopping at the next word start. At the end of a line it goes to the next line's start.
  - Ctrl+Left mirrors that, going to the previous line's end from column 0.
  - A word is alphanumerics plus `'` and `-` (`is_word_char`), so `don't` is one word. The same rule decides which word Ctrl+B/Ctrl+I wrap; keep the two in sync.
- `editor::markdown::highlight` splits a line into styled spans **including** the markers, so screen columns equal cursor columns. Don't strip markers when rendering. The `#`, `*`, `**`, `***` and `_` characters come back as `SpanKind::Marker(Emphasis)`, which records what they surround, so themes can colour them separately. `***text***` is `BoldItalic`.
- `file/` uses free functions taking `&mut P`. `manager` handles list/load/save. `BackupManager` writes `.nowwrite/<name>.<YYYYMMDD_HHMMSS>.bak` every 5 min and before each save. `config/keyboard.rs` stores layouts as static tables mapping US chars to an `Output` (`Char` or `Dead` accent), with separate normal and AltGr tables. `KeyboardConfig::translate` handles dead-key composition. The selected layout is persisted to `.nowwrite/.keyboard.config`. Only en-US and de-CH exist, per PROMPT.md. OVMF reports nothing for the ISO key (`<` `>` `\`), so de-CH maps AltGr+`,` `.` `-` to `<` `>` `\`.
- `ui/editor_view.rs` soft-wraps lines into screen rows (`wrap_line`/`line_rows`, unit-tested). Up/Down/PageUp/PageDown/Home/End move by screen row via `EditorView::vertical`/`row_edge`, and the editor keeps a `goal_x` column. The file still stores one line per paragraph. Rows are addressed as `RowPos { line, sub }` (`top` is the first row on screen), and the `Rows` iterator wraps lines lazily, so rendering, scrolling and movement cost a screenful of work regardless of document size. Never lay out the whole document. It writes cells directly into the frame buffer (one char per cell, control chars as spaces) instead of using `Paragraph`.
- `ui/theme.rs` defines the themes: `'static` `Theme` structs of colour roles (`background`, `text`, `heading`, `bold`, `italic`, `marker: Option`, `hint`, `selection`, `cursor`, `bar`, `bar_key`, `dialog*`, `directory`, `error`, `rounded`) as `Color::Rgb`. `THEMES` holds Classic (the original VGA look), Nocturne (default), Paper, Amber CRT and Green Phosphor. The active one is an index in an `AtomicUsize`; render code and the UEFI backends read it with `theme::current()`. Tests that change it take `theme::TEST_LOCK`. It also has the pure 16-colour console mapping (`console_colors`), unit-tested. `config/theme.rs` persists the choice to `.nowwrite/.theme.config` (`theme=<id>`).
- `ui/` also has the style helpers (in `ui/mod.rs`), `editor_view`, `status_bar` and `modal` (file box, Save As/New name prompt, layout picker, theme picker, confirm prompt, help overlay).
  - **Every colour comes from the theme**, through the `ui/mod.rs` helpers (`base()`, `heading()`, `muted()`, `selection()`, `dialog_block()`…). Never hard-code a `Color`.
  - Paint every cell: the editor area uses `base()` and modals use `dialog_block()`, so no cell is left as `Color::Reset`.
  - UI chrome uses only glyphs from ASCII, box drawing, block elements and arrows, so the text console can still show it. Documents may contain anything.
  - Keep the selection, cursor, bar and dialog-selected backgrounds distinct from the main background. The text console relies on this to keep them visible.
  - The theme picker (`ThemeModal`) previews themes live by setting the global, and restores the original on Esc. After any theme change the editor sets `repaint`, so the next draw calls `terminal.clear()`, because every cell's colours changed.
- `ui/status_bar.rs`: the left side shows the document name (`[new document]` if unnamed) with ` (modified)` and word/char counts. The counts drop out while a message is shown or space is short. The right side, from `StatusInfo`, shows the message, pending dead key (`^ + letter`), layout code (UEFI only), `Ln x, Col y`, battery and `F1 Help`.
- `platform/terminal.rs` uses crossterm via the `ratatui::crossterm` re-export, not a direct dependency. `Drop` and a panic hook restore the terminal.
- `font/` reads the bitmap font that `build.rs` bakes from `assets/fonts/JetBrainsMono-*.ttf` with fontdue: Regular/Bold/Italic/BoldItalic at 16, 20, 24, 28 and 32 px, Latin-1, Latin Extended-A and some typography/arrows. Each glyph is a tight 8-bit coverage box placed in the cell; gamma 0.85 is applied at bake time. The format is defined once in `font/writer.rs`, which `build.rs` includes via `#[path]`; `font::Font` binary-searches the blob in place. Terminal builds get an empty blob. `font/boxdraw.rs` draws box drawing and block elements procedurally (unit-tested joins), because font glyphs don't tile when cells are taller than the font.
- `platform/uefi/` targets uefi 0.41 (global allocator + panic handler come from crate features).
  - `backend.rs`: `UefiBackend` is an enum, `Text(TextBackend)` or `Graphics(GopBackend)`, delegating `Backend`, so `Platform::Backend` stays one type. `init` opens the file system first; `.nowwrite/.display.config` containing `text` forces text mode (the escape hatch for broken GOP).
  - `gop.rs` (graphics mode): opens GOP non-exclusively (`GetProtocol`), so the firmware text console still works for `uefi_main`'s error message. It keeps the firmware's mode unless that is under 1024x768; then it switches to the largest mode up to 1920x1200, and restores the original mode on drop. It picks the largest font size that gives at least 80x25 cells and centres the grid; edge cells paint the margins. Cells are rendered into a RAM back buffer (`BltPixel`s): fill the background, blend coverage, with `Reset` resolved through the theme. `flush` copies each run of dirty rows (padded by 2 px for scaled viewers): straight into the linear framebuffer when its format is BGR (same layout as `BltPixel`), else with one `blt` per rectangle, which also covers `BltOnly` firmware. `clear()` only marks the whole screen dirty, so a repaint is copied once. Glyph blending uses cached 256-entry tables per fg/bg pair, and box-drawing glyphs are rendered once per char. Missing glyphs draw a hollow box.
  - The global allocator is `heap::SizeClassHeap` over `uefi::allocator::Allocator` (the crate's `global_allocator` feature is off): blocks up to 4 KiB come from size-class free lists in 256 KiB chunks, because each UEFI pool call costs about 1.75 us.
  - `text.rs` (fallback): ratatui `Backend` over Simple Text Output. It maps theme colours to the nearest of the 16 console colours via `theme::console_colors` (backgrounds limited to 8), batches same-colour runs, draws rounded corners square, and skips the bottom-right cell, which would scroll the screen.
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

### Performance

The UEFI target has no SSE (soft-float), and the CPU is often cold when a key arrives, so every cycle shows up as latency. Keep these:
- Work per frame must scale with the screen, not the document (see `editor_view`, the running counts in `TextBuffer`).
- Don't use ratatui `Layout` in per-frame code: without `std` its cache is off and the constraint solver runs on every call. Use `ui::centered`/`split_last_row` (tested against `Layout`).
- Avoid `Paragraph`/`List` for large or per-frame areas; write cells directly.
- Measure with the `perf` feature and the `.nowwrite/perf.log` it writes. On the dev machine (Intel N150, KVM, 1920x1080, 150 KB document) typing frames average ~1.2 ms, down from ~9.4 ms. Note: QEMU numbers include the CPU waking cold; a warm host run of the same frame is ~4x faster.
- The release profile uses `opt-level = 3`.

### Testing under QEMU

`run-qemu.sh` / `run-qemu.ps1` use `scripts/fatimg.py`, a stdlib-only FAT16 image tool (MBR + one partition, long file names, subdirectories):
- `create` builds `output/nowwrite-disk.img` from `output/drive/` plus `\EFI\BOOT\BOOTX64.EFI`.
- `extract --skip EFI` copies the image back to `output/drive/` after QEMU exits. It also runs at startup, to recover a run that was killed.

Don't use QEMU's `fat:rw:<dir>` drive: it doesn't reliably write changes back to the host.

QEMU's default display gives OVMF 1280x800. Use `-- -vga none -device VGA,xres=1920,yres=1080` for a 1080p GOP mode, or `xres=800,yres=600` to exercise the mode switch.

Headless runs: `./scripts/run-qemu.sh -- -display none -monitor tcp:127.0.0.1:4444,server,nowait` (PowerShell: `-QemuArgs '-display','none','-monitor','tcp:127.0.0.1:4444,server,nowait'`). Drive them through the monitor:
- `sendkey ctrl-s`, `sendkey f4` and similar send keys. `shift-right` arrives without Shift, but `shift-end`/`shift-home` select;
- `screendump file.ppm` captures the screen; convert it to PNG to view;
- `quit` ends QEMU, after which the script syncs files back.

The firmware clock is UTC, so backup timestamps in the VM are UTC.

### Not yet implemented (from PROMPT.md)

- Battery level (needs ACPI/EC access).
- Turning off radios to save power.
- Sleep on lid close.
- In text mode (no GOP, or `.display.config` = `text`), OVMF's font lacks some glyphs, e.g. `€` and curly quotes, which show as blanks. Graphics mode renders them, and the characters are always saved correctly.
