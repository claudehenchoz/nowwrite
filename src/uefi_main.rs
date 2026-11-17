//! UEFI entry point. Build with:
//! cargo build --release --bin nowwrite_uefi --target x86_64-unknown-uefi --no-default-features --features uefi

#![no_std]
#![no_main]

use nowwrite::platform::uefi::UefiPlatform;
use nowwrite::platform::Platform;
use uefi::prelude::*;
use uefi::{boot, println, system};

#[entry]
fn main() -> Status {
    // Firmware resets the machine after 5 minutes unless the watchdog is disabled.
    let _ = boot::set_watchdog_timer(0, 0x10000, None);

    // `run` consumes the platform, which restores the console when dropped.
    match UefiPlatform::init().and_then(|(platform, backend)| nowwrite::run(platform, backend)) {
        Ok(()) => Status::SUCCESS,
        Err(err) => {
            println!("NowWrite error: {err}");
            println!("Press any key to exit.");
            let _ = system::with_stdin(|stdin| -> uefi::Result {
                let event = stdin.wait_for_key_event()?;
                boot::wait_for_event(&mut [event]).map_err(|e| e.to_err_without_payload())?;
                Ok(())
            });
            Status::ABORTED
        }
    }
}
