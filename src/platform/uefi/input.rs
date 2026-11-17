//! Keyboard input. Prefers Simple Text Input Ex, which reports Ctrl/Shift
//! state; falls back to plain Simple Text Input and control characters.

use crate::error::{Error, Result};
use crate::platform::KeyEvent;
use alloc::format;
use core::time::Duration;
use uefi::boot::{
    self, EventType, OpenProtocolAttributes, OpenProtocolParams, ScopedProtocol, TimerTrigger, Tpl,
};
use uefi::proto::console::text::{InputEx, Key, ScanCode};
use uefi::{system, Event, Handle};

/// `EFI_RIGHT_CONTROL_PRESSED | EFI_LEFT_CONTROL_PRESSED` in the key shift state.
const CONTROL_PRESSED: u32 = 0x0000_0004 | 0x0000_0008;
/// `EFI_RIGHT_ALT_PRESSED`: AltGr on international keyboards.
const RIGHT_ALT_PRESSED: u32 = 0x0000_0010;

/// How often `read_key` returns `KeyEvent::Tick` while idle (cursor blink).
const TICK: Duration = Duration::from_millis(500);

pub struct UefiInput {
    input_ex: Option<ScopedProtocol<InputEx>>,
    timer: Event,
}

impl UefiInput {
    pub fn new() -> Result<Self> {
        // SAFETY: a plain timer event with no notification function.
        let timer = unsafe { boot::create_event(EventType::TIMER, Tpl::CALLBACK, None, None) }
            .map_err(err)?;
        boot::set_timer(&timer, TimerTrigger::Periodic(TICK)).map_err(err)?;
        Ok(Self { input_ex: open_input_ex(), timer })
    }

    /// Wait for the key event or the tick timer. Returns true if a key is ready.
    fn wait_for_key(&self, key_event: Event) -> Result<bool> {
        // SAFETY: the clone only lives for this call; `self.timer` outlives it.
        let timer = unsafe { self.timer.unsafe_clone() };
        let index = boot::wait_for_event(&mut [key_event, timer])
            .map_err(|e| err(e.to_err_without_payload()))?;
        Ok(index == 0)
    }

    pub fn read_key(&mut self) -> Result<KeyEvent> {
        loop {
            let (key, ctrl, altgr) = if self.input_ex.is_some() {
                let key_event = self.input_ex.as_ref().unwrap().wait_for_key_event().map_err(err)?;
                if !self.wait_for_key(key_event)? {
                    return Ok(KeyEvent::Tick);
                }
                match self.input_ex.as_mut().unwrap().read_key().map_err(err)? {
                    Some(data) => {
                        let shift = data.key_state.key_shift_state.map_or(0, |s| s.bits());
                        (data.key, shift & CONTROL_PRESSED != 0, shift & RIGHT_ALT_PRESSED != 0)
                    }
                    None => continue,
                }
            } else {
                let key_event = system::with_stdin(|stdin| stdin.wait_for_key_event()).map_err(err)?;
                if !self.wait_for_key(key_event)? {
                    return Ok(KeyEvent::Tick);
                }
                match system::with_stdin(|stdin| stdin.read_key()).map_err(err)? {
                    Some(key) => (key, false, false),
                    None => continue,
                }
            };
            if let Some(mapped) = map_key(key, ctrl, altgr) {
                return Ok(mapped);
            }
        }
    }
}

impl Drop for UefiInput {
    fn drop(&mut self) {
        let _ = boot::set_timer(&self.timer, TimerTrigger::Cancel);
        // SAFETY: we own the event and nothing else uses it after this.
        let _ = boot::close_event(unsafe { self.timer.unsafe_clone() });
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

fn map_key(key: Key, ctrl: bool, altgr: bool) -> Option<KeyEvent> {
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
                c if altgr => Some(KeyEvent::AltGr(c)),
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
