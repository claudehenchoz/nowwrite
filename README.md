# NowWrite

A minimalist, distraction-free markdown editor that runs directly from UEFI firmware without requiring an operating system.

## Features

- **Distraction-free editing**: A full-screen, keyboard-only interface focused on writing
- **Crisp graphics on UEFI**: Anti-aliased JetBrains Mono with real bold and italic, falling back to the firmware's text console where graphics aren't available
- **Themes**: Five built-in colour themes (Ctrl+T), previewed live
- **UEFI-native**: Boots directly from firmware, no OS required
- **Automatic backups**: Every 5 minutes and on manual save
- **Markdown support**: Basic syntax highlighting for headings, bold, and italic
- **Keyboard layouts**: US English and Swiss German (UEFI version)
- **UTF-8 support**: International character support
- **Terminal version**: The same editor runs in a Linux, macOS or Windows terminal for development and testing

## Building

### Terminal Version (Linux, macOS, Windows)

```bash
cargo run                       # build and run in the current directory
./scripts/build-terminal.sh     # Linux/macOS -> output/nowwrite
```

```powershell
.\scripts\build-terminal.ps1    # Windows -> output\nowwrite.exe
```

### UEFI Version

```bash
./scripts/build-uefi.sh --create-boot        # Linux/macOS
```

```powershell
.\scripts\build-uefi.ps1 -CreateBoot         # Windows
```

This produces `output/NowWrite.efi` and, with the boot option, `output/boot/EFI/BOOT/BOOTX64.EFI`. Copy the contents of `output/boot/` to a FAT32 drive to boot it.

### Testing in QEMU

```powershell
.\scripts\build-uefi.ps1 -CreateBoot
.\scripts\run-qemu.ps1
```

On Linux:

```bash
./scripts/build-uefi.sh
./scripts/run-qemu.sh
```

Your test documents live in `output\drive\`, which is created on the first run. The script:
1. builds a writable FAT16 disk image (`output\nowwrite-disk.img`) from that folder plus `NowWrite.efi`;
2. boots it with the UEFI firmware bundled with QEMU for Windows, or the system OVMF package on Linux (pass `-Ovmf <file>` / `--ovmf <file>` to use another);
3. copies everything back into `output\drive\` when QEMU exits, so saved documents and `.nowwrite\` backups appear on the host.

It needs Python 3 (for `scripts\fatimg.py`, standard library only).

To try other screen sizes, pass a resolution preset or any `WIDTHxHEIGHT`:

```bash
./scripts/run-qemu.sh --resolution fhd        # PowerShell: .\scripts\run-qemu.ps1 -Resolution fhd
./scripts/run-qemu.sh -r 1600x900
./scripts/run-qemu.sh --resolution list       # show the presets
```

| Preset | Size | What it shows |
|--------|------|---------------|
| `vga`, `svga` | 640x480, 800x600 | Firmware starts small; NowWrite switches to a larger mode |
| `xga` | 1024x768 | Smallest size NowWrite keeps as is |
| `wxga` | 1280x800 | QEMU's default |
| `hd` | 1360x768 | Budget laptop panel (1366x768 panels report 1360 to firmware) |
| `fhd`, `wuxga` | 1920x1080, 1920x1200 | Full HD |
| `qhd` | 2560x1440 | Larger than NowWrite's 1920x1200 limit for switching modes |

Widths that aren't a multiple of 8 are rounded down, because QEMU's firmware can't display them correctly. The first boot at a new resolution takes a few seconds longer.

### Create Bootable USB Drive

```bash
sudo ./scripts/create-boot-drive.sh /dev/sdX NOWWRITE
```

**Warning**: This will format the specified device!

## Key Bindings

| Key Combination | Alternative | Action |
|-----------------|-------------|--------|
| F1 | | Help: every shortcut on one screen |
| Arrow keys, Home/End, PgUp/PgDn | | Move the cursor (Up/Down/Home/End follow wrapped lines on screen) |
| Ctrl+Left / Ctrl+Right | | Move to the previous / next word |
| Shift+Arrows, Shift+Home/End, Ctrl+Shift+Left/Right | | Select text |
| Ctrl+S | F2 | Save (creates a backup first; asks for a name the first time) |
| Ctrl+L | F3 | Open a document |
| Ctrl+N | F4 | New document (asks for a name) |
| Ctrl+K | F5 | Change keyboard layout (UEFI version) |
| Ctrl+T | F6 | Choose a theme (↑↓ previews it live, Enter keeps it, Esc goes back) |
| Ctrl+B | | Bold on/off for the word or selection (`**`) |
| Ctrl+I | | Italic on/off for the word or selection (`*`) |
| Ctrl+Z / Ctrl+Y | | Undo / redo (word by word) |
| Ctrl+Q | F10 | Quit |
| Esc | | Close a box |

Opening, creating or quitting with unsaved changes asks whether to save first. Long lines wrap at word boundaries; the file itself keeps one line per paragraph.

The F-keys exist because some firmware doesn't report Ctrl key combinations.

## Themes and Display

Ctrl+T (or F6) opens the theme picker. Moving through the list switches the whole screen to that theme at once. Enter keeps it, and Esc goes back to the theme you had. The choice is saved in `.nowwrite/.theme.config`.

| Theme | Look |
|-------|------|
| Nocturne (default) | Deep blue-black with soft lavender headings, gold bold and violet italics |
| Classic | The original grey-on-black text console |
| Paper | Dark ink on warm off-white |
| Amber CRT | Amber on near-black, like an old terminal |
| Green Phosphor | Green on black |

On UEFI, NowWrite draws its own pixels when the firmware offers graphics output (GOP): 24-bit colour, the JetBrains Mono font with real bold and italic, and characters such as `€` and curly quotes. It keeps the firmware's resolution, unless that's below 1024x768. In that case it switches to the largest mode up to 1920x1200, then picks the biggest font size that still fits 80x25 characters.

Without graphics output it falls back to the firmware's text console, and maps each theme to the 16 colours that console offers. To force the text console (e.g. if graphics look wrong on your machine), put a file `.nowwrite/.display.config` containing the word `text` on the drive.

The terminal version uses the same themes in 24-bit colour, in terminals that support it.

The keyboard layout setting (Ctrl+K) only applies to the UEFI version. Firmware always reports US-layout characters, so NowWrite remaps them itself. That covers shifted keys, AltGr characters (`@ # | € [ ] { }`) and dead keys (`` ^ ` ´ ¨ ~ ``, e.g. `^` then `e` gives `ê`). In a terminal, the operating system's own layout is used.

Firmware doesn't report the key left of `Y` on Swiss keyboards, so with de-CH use **AltGr+`,`** for `<`, **AltGr+`.`** for `>` and **AltGr+`-`** for `\`.

## Architecture

- **Core library** (`src/lib.rs`): the editor, UI and file handling. It is `no_std` + `alloc`, so the same code runs under UEFI.
- **UI**: [ratatui](https://ratatui.rs) widgets. A terminal uses ratatui's crossterm backend. On UEFI, a custom backend draws pixels through GOP with a font baked in at build time (`build.rs`), or falls back to the text console.
- **Platform layer**: `terminal` (crossterm and `std::fs`) and `uefi` (text console, keyboard, and the FAT volume NowWrite booted from).

## File Structure

Documents live in the root of the boot drive (UEFI) or the current directory (terminal):

```
*.md                 # Your markdown files
.nowwrite/           # Configuration and backups
├── .keyboard.config
├── .theme.config
├── .display.config  # optional: "text" forces the text console
├── document.md.20240315_143022.bak
└── ...
```

## Requirements

### UEFI Version
- x86_64 UEFI firmware
- FAT32 formatted storage
- Rust 1.91+ with the `x86_64-unknown-uefi` target (the build scripts install it)

### Terminal Version
- Any terminal supported by crossterm (Linux, macOS, Windows Terminal/console)
- Rust 1.91+

## Development

```toml
[features]
default = ["terminal"]
terminal = ["ratatui/crossterm", "dep:chrono"]
uefi = ["dep:uefi"]
```

The features are mutually exclusive. UEFI builds need `--no-default-features --features uefi --target x86_64-unknown-uefi`.

## License

NowWrite embeds the [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono) font (in `assets/fonts/`) for its UEFI graphics mode. The font is licensed under the SIL Open Font License 1.1 (no Reserved Font Name); see `assets/fonts/OFL.txt`, which the build scripts also copy next to the EFI file.


This project is designed as a reference implementation of a UEFI-native text editor. Feel free to modify and extend as needed.
