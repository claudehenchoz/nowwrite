# NowWrite

A minimalist, distraction-free markdown editor that runs directly from UEFI firmware without requiring an operating system.

## Features

- **Distraction-free editing**: Pure text-mode interface focused on writing
- **UEFI-native**: Boots directly from firmware, no OS required
- **Automatic backups**: Every 5 minutes and on manual save
- **Markdown support**: Basic syntax highlighting for headings, bold, and italic
- **Multiple keyboard layouts**: US English, German-Swiss, French, Spanish
- **UTF-8 support**: International character support
- **Platform abstraction**: Linux version for development and testing

## Building

### Linux Development Version

```bash
./scripts/build-linux.sh
./output/nowwrite-linux
```

### UEFI Version

```bash
./scripts/build-uefi.sh
```

### Create Bootable USB Drive

```bash
sudo ./scripts/create-boot-drive.sh /dev/sdX NOWWRITE
```

**Warning**: This will format the specified device!

## Key Bindings

| Key Combination | Action |
|----------------|--------|
| Arrow Keys | Move cursor |
| Ctrl+S | Save file |
| Ctrl+L | Load file (show file modal) |
| Ctrl+N | New file |
| Ctrl+B | Toggle bold formatting |
| Ctrl+K | Change keyboard layout |
| Ctrl+Z | Undo |
| Ctrl+Y | Redo |
| Escape | Exit application |

## Architecture

The application is built with a clean platform abstraction layer:

- **Editor Core**: Text buffer, cursor management, undo/redo
- **UI Layer**: Rendering, modals, status bar
- **File System**: File operations with automatic backup
- **Platform Layer**: UEFI and Linux implementations
- **Configuration**: Keyboard layout management

## File Structure

```
documents/           # Your markdown files
.nowwrite/          # Configuration and backups
├── .keyboard.config
├── document.md.20240315_143022.bak
└── ...
```

## Requirements

### UEFI Version
- x86_64 UEFI firmware
- FAT32 formatted storage
- At least 16MB available space

### Linux Version
- Linux with terminal support
- Rust toolchain for building

## Development

The project uses Rust with conditional compilation for different platforms:

```toml
[features]
default = ["linux"]
uefi = ["dep:uefi", "dep:uefi-services"]
linux = ["dep:termios", "dep:terminal_size"]
```

## License

This project is designed as a reference implementation of a UEFI-native text editor. Feel free to modify and extend as needed.