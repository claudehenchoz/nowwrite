//! Keyboard input. Prefers Simple Text Input Ex, which reports Ctrl/Shift
//! state; falls back to plain Simple Text Input and control characters.

use crate::error::{Error, Result};
use crate::platform::KeyEvent;
use alloc::format;
use uefi::boot::{self, OpenProtocolAttributes, OpenProtocolParams, ScopedProtocol};
use uefi::proto::console::text::{InputEx, Key, ScanCode};
use uefi::{system, Handle};

/// `EFI_RIGHT_CONTROL_PRESSED | EFI_LEFT_CONTROL_PRESSED` in the key shift state.
const CONTROL_PRESSED: u32 = 0x0000_0004 | 0x0000_0008;

pub struct UefiInput {
    input_ex: Option<ScopedProtocol<InputEx>>,
}

impl UefiInput {
    pub fn new() -> Self {
        Self { input_ex: open_input_ex() }
    }

    pub fn read_key(&mut self) -> Result<KeyEvent> {
        loop {
            let (key, ctrl) = match &mut self.input_ex {
                Some(input) => {
                    let event = input.wait_for_key_event().map_err(err)?;
                    boot::wait_for_event(&mut [event]).map_err(|e| err(e.to_err_without_payload()))?;
                    match input.read_key().map_err(err)? {
                        Some(data) => {
                            let ctrl = data
                                .key_state
                                .key_shift_state
                                .is_some_and(|s| s.bits() & CONTROL_PRESSED != 0);
                            (data.key, ctrl)
                        }
                        None => continue,
                    }
                }
                None => {
                    let key = system::with_stdin(|stdin| -> uefi::Result<Option<Key>> {
                        let event = stdin.wait_for_key_event()?;
                        boot::wait_for_event(&mut [event]).map_err(|e| e.to_err_without_payload())?;
                        stdin.read_key()
                    })
                    .map_err(err)?;
                    match key {
                        Some(key) => (key, false),
                        None => continue,
                    }
                }
            };
            if let Some(mapped) = map_key(key, ctrl) {
                return Ok(mapped);
            }
        }
    }
}

fn err(e: uefi::Error) -> Error {
    Error::Io(format!("{e:?}"))
}

fn open_input_ex() -> Option<ScopedProtocol<InputEx>> {
    let st = uefi::table::system_table_raw()?;
    // SAFETY: the system table pointer is valid while boot services run.
    let raw_handle = unsafe { st.as_ref().stdin_handle };
    // SAFETY: the firmware-provided console-in handle is a valid handle.
    let handle = unsafe { Handle::from_ptr(raw_handle) }?;
    // SAFETY: we only use the protocol from this single-threaded app, and the
    // console-in handle is not uninstalled while boot services run.
    unsafe {
        boot::open_protocol::<InputEx>(
            OpenProtocolParams { handle, agent: boot::image_handle(), controller: None },
            OpenProtocolAttributes::GetProtocol,
        )
    }
    .ok()
}

fn map_key(key: Key, ctrl: bool) -> Option<KeyEvent> {
    match key {
        Key::Printable(c16) => {
            let ch = char::from(c16);
            if ctrl && ch.is_ascii_alphabetic() {
                return ctrl_letter(ch.to_ascii_lowercase());
            }
            match ch {
                '\r' | '\n' => Some(KeyEvent::Enter),
                '\x08' => Some(KeyEvent::Backspace),
                '\t' => Some(KeyEvent::Tab),
                // Firmware without InputEx may send Ctrl+letter as a control char.
                '\x01'..='\x1a' => ctrl_letter((b'a' + (ch as u8 - 1)) as char),
                c if c.is_control() => None,
                c => Some(KeyEvent::Char(c)),
            }
        }
        Key::Special(scan) => Some(match scan {
            ScanCode::UP => KeyEvent::Up,
            ScanCode::DOWN => KeyEvent::Down,
            ScanCode::LEFT => KeyEvent::Left,
            ScanCode::RIGHT => KeyEvent::Right,
            ScanCode::HOME => KeyEvent::Home,
            ScanCode::END => KeyEvent::End,
            ScanCode::PAGE_UP => KeyEvent::PageUp,
            ScanCode::PAGE_DOWN => KeyEvent::PageDown,
            ScanCode::DELETE => KeyEvent::Delete,
            ScanCode::ESCAPE => KeyEvent::Escape,
            // Aliases for firmware that doesn't report Ctrl.
            ScanCode::FUNCTION_2 => KeyEvent::CtrlS,
            ScanCode::FUNCTION_3 => KeyEvent::CtrlL,
            ScanCode::FUNCTION_4 => KeyEvent::CtrlN,
            ScanCode::FUNCTION_5 => KeyEvent::CtrlK,
            _ => return None,
        }),
    }
}

fn ctrl_letter(ch: char) -> Option<KeyEvent> {
    Some(match ch {
        's' => KeyEvent::CtrlS,
        'l' => KeyEvent::CtrlL,
        'n' => KeyEvent::CtrlN,
        'i' => KeyEvent::CtrlI,
        'b' => KeyEvent::CtrlB,
        'k' => KeyEvent::CtrlK,
        'z' => KeyEvent::CtrlZ,
        'y' => KeyEvent::CtrlY,
        _ => return None,
    })
}
