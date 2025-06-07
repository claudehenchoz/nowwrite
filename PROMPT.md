# NowWrite

NowWrite is a distractionless text-mode markdown editor that runs entirely from a UEFI file on a FAT32 removable storage device (USB stick, flash card, etc.), allowing loading and saving to the same FAT32 volume.

## Features

* Loads and runs straight from UEFI, does not need an operating system
* Runs entirely in text-mode with an white font-color, but highlights things formatted as headings (lines prefixed with one or more #) or bold/italics (sections surrounded by one or two *) as bright wite
* Upon loading, shows a modal dialog that shows the .md files in the root of the FAT32 storage device that it was booting from, where a file can be selected for loading with the arrow keys and Enter, or a new one can be created with CTRL-N (where it prompts for a filename)
* In the editor, the bottom row should be with a grey background, showing currently open filename, word count, character count, and an indicator of whether the file has been saved in the current form. Also include a battery indicator.
* It should have a blinking cursor, and should support selecting text with Shift-[arrow keys]
* When hitting CTRL-I and the cursor is inside a word, enclose that word with *, same for CTRL-B and enclosing it with **
* When hitting CTRL-I or CTRL-B and there is a selection, enclose the whole selection with * or **
* Every 5 minutes, create a backup copy of the currently open document in a .nowwrite folder in the root of the FAT32 drive used to boot, with a timestamp appended to the filename.
* CTRL-S saves the currently open document (after creating another timestamped backup in the .nowwrite folder)
* CTRL-L brings up the loading modal dialog
* CTRL-K should bring up another modal dialog that allows for keyboard layout selection (support only en-US and de-CH for now), which should be stored in the .nowwrite folder in a file called .keyboard.config (subsequent launches should use the stored keyboard layout)
* Support UTF-8 encoding, including any UTF-8 chars.

## Technical Design

* Use Rust to implement the application
* Make it UEFI-first, because that is the main target, but allow for using it in any Linux shell (with minimal dependencies) for debugging/tweaking of the UI
* When running from UEFI, turn off any unnecessary devices (wifi/bt radios) and apply configuration that increases battery life to the max
* When running from UEFI, make it so that when the laptop lid is closed, the laptop goes to low-power sleep mode and resumes again when opening it
* Create helper scripts to compile for both targets
* Create a README that describes the editor itself and how to create a bootable drive
