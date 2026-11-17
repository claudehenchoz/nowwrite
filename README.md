# NowWrite

A minimalist, distraction-free markdown editor that runs directly from UEFI firmware without requiring an operating system.

## Features

- **Distraction-free editing**: Pure text-mode interface focused on writing
- **UEFI-native**: Boots directly from firmware, no OS required
- **Automatic backups**: Every 5 minutes and on manual save
- **Markdown support**: Basic syntax highlighting for headings, bold, and italic
- **Multiple keyboard layouts**: US English, German-Swiss, French, Spanish
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

Your test documents live in `output\drive\`, which is created on the first run. The script:
1. builds a writable FAT16 disk image (`output\nowwrite-disk.img`) from that folder plus `NowWrite.efi`;
2. boots it with the UEFI firmware bundled with QEMU for Windows (pass `-Ovmf <file>` to use another);
3. copies everything back into `output\drive\` when QEMU exits, so saved documents and `.nowwrite\` backups appear on the host.

It needs Python 3 (for `scripts\fatimg.py`, standard library only).

### Create Bootable USB Drive

```bash
sudo ./scripts/create-boot-drive.sh /dev/sdX NOWWRITE
```

**Warning**: This will format the specified device!

## Key Bindings

| Key Combination | Alternative | Action |
|-----------------|-------------|--------|
| Arrow keys, Home/End, PgUp/PgDn | | Move cursor |
| Ctrl+S | F2 | Save file (creates a backup first) |
| Ctrl+L | F3 | Open file |
| Ctrl+N | F4 | New file (prompts for a name) |
| Ctrl+K | F5 | Change keyboard layout |
| Ctrl+B | | Bold: wrap the word or selection in `**` |
| Ctrl+I | | Italic: wrap the word or selection in `*` |
| Ctrl+Z / Ctrl+Y | | Undo / redo |
| Escape | | Exit (press twice if there are unsaved changes) |

The F-keys exist because some firmware doesn't report Ctrl key combinations.

The keyboard layout setting (Ctrl+K) only applies to the UEFI version. Firmware always reports US-layout characters, so NowWrite remaps them itself. In a terminal, the operating system's own layout is used.

## Architecture

- **Core library** (`src/lib.rs`): the editor, UI and file handling. It is `no_std` + `alloc`, so the same code runs under UEFI.
- **UI**: [ratatui](https://ratatui.rs) widgets. It uses ratatui's crossterm backend in a terminal and a custom backend over the UEFI text console.
- **Platform layer**: `terminal` (crossterm and `std::fs`) and `uefi` (text console, keyboard, and the FAT volume NowWrite booted from).

## File Structure

Documents live in the root of the boot drive (UEFI) or the current directory (terminal):

```
*.md                 # Your markdown files
.nowwrite/           # Configuration and backups
├── .keyboard.config
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

This project is designed as a reference implementation of a UEFI-native text editor. Feel free to modify and extend as needed.
