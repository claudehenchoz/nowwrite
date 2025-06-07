# NowWrite Design Document

## 1. Overview

NowWrite is a minimalist, distraction-free markdown editor that runs directly from UEFI firmware without requiring an operating system. It provides a pure text-mode editing experience with basic markdown highlighting and automatic backup functionality.

### 1.1 Key Characteristics
- **Boot Environment**: UEFI-native application
- **Storage**: FAT32 removable media (USB, SD card)
- **Interface**: Text-mode only with cursor and selection support
- **File Format**: Markdown (.md) files with UTF-8 encoding
- **Platform**: Primary target is UEFI, with Linux shell support for development

## 2. Architecture

### 2.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    NowWrite Application                  │
├─────────────────────────────────────────────────────────┤
│                    Core Editor Engine                    │
├──────────────┬──────────────┬──────────────┬───────────┤
│ File Manager │ Text Buffer  │ UI Renderer  │ Platform  │
│              │ & Operations │              │ Abstraction│
├──────────────┴──────────────┴──────────────┴───────────┤
│                 Platform Implementation                  │
├─────────────────────┬───────────────────────────────────┤
│    UEFI Platform    │         Linux Platform            │
│   Implementation    │        Implementation             │
└─────────────────────┴───────────────────────────────────┘
```

### 2.2 Module Structure

```
nowwrite/
├── src/
│   ├── main.rs                 # Entry point with platform detection
│   ├── editor/
│   │   ├── mod.rs             # Editor core module
│   │   ├── buffer.rs          # Text buffer implementation
│   │   ├── cursor.rs          # Cursor and selection logic
│   │   └── markdown.rs        # Markdown parsing/highlighting
│   ├── ui/
│   │   ├── mod.rs             # UI module
│   │   ├── renderer.rs        # Text-mode rendering
│   │   ├── modal.rs           # Modal dialog system
│   │   └── status_bar.rs      # Status bar implementation
│   ├── file/
│   │   ├── mod.rs             # File operations module
│   │   ├── manager.rs         # File listing and management
│   │   └── backup.rs          # Automatic backup system
│   ├── platform/
│   │   ├── mod.rs             # Platform abstraction layer
│   │   ├── uefi/
│   │   │   ├── mod.rs         # UEFI implementation
│   │   │   ├── display.rs     # UEFI display handling
│   │   │   ├── input.rs       # UEFI keyboard input
│   │   │   ├── filesystem.rs  # UEFI FAT32 operations
│   │   │   └── power.rs       # Power management
│   │   └── linux/
│   │       ├── mod.rs         # Linux implementation
│   │       ├── terminal.rs    # Terminal I/O
│   │       └── filesystem.rs  # Linux file operations
│   └── config/
│       ├── mod.rs             # Configuration management
│       └── keyboard.rs        # Keyboard layout handling
├── build.rs                   # Build script
├── Cargo.toml                # Rust dependencies
└── scripts/
    ├── build-uefi.sh         # UEFI build script
    ├── build-linux.sh        # Linux build script
    └── create-boot-drive.sh  # Bootable drive creation
```

## 3. Core Components

### 3.1 Text Buffer

The text buffer is the central data structure for storing and manipulating text.

```rust
pub struct TextBuffer {
    lines: Vec<String>,
    cursor: Cursor,
    selection: Option<Selection>,
    undo_stack: Vec<Operation>,
    redo_stack: Vec<Operation>,
    modified: bool,
}

pub struct Cursor {
    line: usize,
    column: usize,
}

pub struct Selection {
    start: Cursor,
    end: Cursor,
}

pub enum Operation {
    Insert { position: Cursor, text: String },
    Delete { position: Cursor, text: String },
    Replace { position: Cursor, old: String, new: String },
}
```

### 3.2 Markdown Parser

Simple markdown parser for syntax highlighting:

```rust
pub enum MarkdownToken {
    Heading { level: u8, text: String },
    Bold(String),
    Italic(String),
    PlainText(String),
}

pub struct MarkdownLine {
    tokens: Vec<MarkdownToken>,
}
```

### 3.3 Platform Abstraction Layer

```rust
pub trait Platform {
    fn init() -> Result<Self, Error>;
    fn read_key(&mut self) -> Result<KeyEvent, Error>;
    fn get_screen_size(&self) -> (usize, usize);
    fn clear_screen(&mut self) -> Result<(), Error>;
    fn draw_char(&mut self, x: usize, y: usize, ch: char, attr: TextAttribute) -> Result<(), Error>;
    fn list_files(&self, path: &str) -> Result<Vec<FileInfo>, Error>;
    fn read_file(&self, path: &str) -> Result<String, Error>;
    fn write_file(&self, path: &str, content: &str) -> Result<(), Error>;
    fn create_directory(&self, path: &str) -> Result<(), Error>;
    fn get_battery_status(&self) -> Option<BatteryStatus>;
    fn set_power_mode(&mut self, mode: PowerMode) -> Result<(), Error>;
}

pub struct TextAttribute {
    bright: bool,
    background: Color,
}

pub enum Color {
    Default,
    Grey,
}
```

## 4. User Interface

### 4.1 Main Editor View

```
┌─────────────────────────────────────────────────────────┐
│# NowWrite Design Document                               │
│                                                         │
│This is a **distraction-free** markdown editor that     │
│runs directly from UEFI.                                 │
│                                                         │
│## Features                                              │
│                                                         │
│* Minimal interface                                      │
│* Automatic backups                                      │
│* UTF-8 support                                          │
│                                                         │
│                                                         │
│                                                         │
├─────────────────────────────────────────────────────────┤
│ document.md | 42 words | 238 chars | ● | [====] 85%    │
└─────────────────────────────────────────────────────────┘
```

### 4.2 File Selection Modal

```
┌─────────────────────────────────────────────────────────┐
│                    Select File                          │
├─────────────────────────────────────────────────────────┤
│  > document.md                                          │
│    notes.md                                             │
│    ideas.md                                             │
│    project-plan.md                                      │
│                                                         │
│  [↑↓] Navigate  [Enter] Open  [Ctrl+N] New             │
└─────────────────────────────────────────────────────────┘
```

### 4.3 Keyboard Layout Modal

```
┌─────────────────────────────────────────────────────────┐
│                Select Keyboard Layout                    │
├─────────────────────────────────────────────────────────┤
│  > en-US (English - United States)                     │
│    de-CH (German - Switzerland)                        │
│                                                         │
│  [↑↓] Navigate  [Enter] Select                         │
└─────────────────────────────────────────────────────────┘
```

## 5. Key Bindings

| Key Combination | Action |
|----------------|--------|
| Arrow Keys | Move cursor |
| Shift + Arrow Keys | Select text |
| Ctrl + S | Save file |
| Ctrl + L | Load file (show file modal) |
| Ctrl + N | New file |
| Ctrl + I | Toggle italic formatting |
| Ctrl + B | Toggle bold formatting |
| Ctrl + K | Change keyboard layout |
| Ctrl + Z | Undo |
| Ctrl + Y | Redo |
| Home/End | Beginning/end of line |
| Ctrl + Home/End | Beginning/end of document |
| Page Up/Down | Scroll one screen |

## 6. File Operations

### 6.1 File Structure

```
/
├── document.md
├── notes.md
├── ideas.md
└── .nowwrite/
    ├── .keyboard.config
    ├── document.md.20240315_143022.bak
    ├── document.md.20240315_143522.bak
    └── notes.md.20240315_144512.bak
```

### 6.2 Backup Strategy

- Automatic backup every 5 minutes if file has been modified
- Backup on every manual save (Ctrl+S)
- Backup filename format: `{original_name}.{timestamp}.bak`
- Timestamp format: `YYYYMMDD_HHMMSS`
- Backups stored in `.nowwrite/` directory

### 6.3 Configuration Storage

Keyboard layout configuration stored in `.nowwrite/.keyboard.config`:
```
layout=de-CH
```

## 7. UEFI Implementation Details

### 7.1 UEFI Services Required

- **Simple Text Input Protocol**: Keyboard input
- **Simple Text Output Protocol**: Screen output
- **Simple File System Protocol**: FAT32 access
- **ACPI Table Protocol**: Power management
- **Battery Protocol** (if available): Battery status

### 7.2 Power Management

```rust
pub enum PowerMode {
    Normal,
    PowerSaving,
    Sleep,
}

// UEFI implementation
impl UefiPlatform {
    fn configure_power_saving(&mut self) -> Result<(), Error> {
        // Disable unnecessary devices
        self.disable_wifi()?;
        self.disable_bluetooth()?;
        self.set_cpu_frequency(CpuFrequency::Low)?;
        self.dim_screen_backlight(50)?;
        Ok(())
    }
    
    fn handle_lid_close(&mut self) -> Result<(), Error> {
        self.save_state()?;
        self.enter_s3_sleep()?;
        Ok(())
    }
}
```

### 7.3 Boot Process

1. UEFI firmware loads NowWrite.efi from FAT32 volume
2. NowWrite initializes UEFI protocols
3. Configure power-saving mode
4. Initialize text-mode display
5. Load keyboard configuration or use default
6. Show file selection modal
7. Enter main editor loop

## 8. Linux Implementation Details

For development and testing:

```rust
impl LinuxPlatform {
    fn init() -> Result<Self, Error> {
        // Set terminal to raw mode
        let original_termios = termios::tcgetattr(0)?;
        let mut raw = original_termios.clone();
        termios::cfmakeraw(&mut raw);
        termios::tcsetattr(0, termios::TCSANOW, &raw)?;
        
        // Enable alternate screen buffer
        print!("\x1B[?1049h");
        
        Ok(LinuxPlatform {
            original_termios,
            // ...
        })
    }
}
```

## 9. Build System

### 9.1 Dependencies (Cargo.toml)

```toml
[package]
name = "nowwrite"
version = "0.1.0"
edition = "2021"

[dependencies]
uefi = { version = "0.27", features = ["alloc"], optional = true }
uefi-services = { version = "0.24", optional = true }
termios = { version = "0.3", optional = true }
chrono = "0.4"

[features]
default = ["linux"]
uefi = ["dep:uefi", "dep:uefi-services"]
linux = ["dep:termios"]

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
```

### 9.2 Build Scripts

**build-uefi.sh:**
```bash
#!/bin/bash
cargo build --release --target x86_64-unknown-uefi --no-default-features --features uefi
cp target/x86_64-unknown-uefi/release/nowwrite.efi output/NowWrite.efi
```

**build-linux.sh:**
```bash
#!/bin/bash
cargo build --release --features linux
cp target/release/nowwrite output/nowwrite-linux
```

## 10. Testing Strategy

### 10.1 Unit Tests
- Text buffer operations
- Markdown parsing
- File backup logic
- Keyboard input handling

### 10.2 Integration Tests
- File I/O operations
- Modal dialog behavior
- Full editing workflows

### 10.3 Platform Tests
- UEFI: Test in QEMU with OVMF
- Linux: Terminal compatibility testing

## 11. Performance Considerations

### 11.1 Memory Usage
- Minimize allocations in hot paths
- Use fixed-size buffers where possible
- Implement efficient undo/redo with operation coalescing

### 11.2 Rendering Optimization
- Only redraw changed portions of screen
- Batch character writes
- Cache markdown parsing results per line

### 11.3 File Operations
- Async auto-save to prevent blocking
- Memory-mapped file access for large files
- Efficient UTF-8 handling

## 12. Security Considerations

- Validate all file paths to prevent directory traversal
- Sanitize filenames for FAT32 compatibility
- Clear sensitive data from memory on exit
- Verify UEFI secure boot compatibility
