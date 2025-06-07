#![no_main]
#![no_std]

extern crate alloc;

#[cfg(feature = "uefi")]
use uefi::prelude::*;

#[cfg(feature = "uefi")]
use core::panic::PanicInfo;

#[cfg(feature = "uefi")]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[cfg(feature = "uefi")]
#[entry]
fn main(image: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();
    
    // Initialize allocator
    let _ = system_table.boot_services().set_watchdog_timer(0, 0x10000, None);
    
    // This would initialize the NowWrite editor
    // For now, just return success as a minimal implementation
    Status::SUCCESS
}