#![cfg_attr(target_os = "none", no_std)]

#[macro_use]
extern crate bitflags;

extern crate xous_macros as macros;

pub use macros::xous_main;

pub mod arch;

pub mod carton;
pub mod definitions;
mod messages;
pub mod syscall;

pub use arch::{ProcessArgs, ProcessInit, ProcessKey, ThreadInit};
pub use definitions::*;
pub use messages::*;
pub use syscall::*;

#[cfg(not(target_os = "none"))]
pub use arch::ProcessArgsAsThread;

/// Convert a four-letter string into a 32-bit int.
#[macro_export]
macro_rules! make_name {
    ($fcc:expr) => {{
        let mut c: [u8; 4] = Default::default();
        c.copy_from_slice($fcc.as_bytes());
        u32::from_le_bytes(c) as usize
    }};
}

#[cfg(not(target_os = "none"))]
#[macro_export]
macro_rules! maybe_main {
    () => {
        extern "Rust" {
            fn xous_entry() -> !;
        }
        fn main() {
            xous::arch::ensure_connection().unwrap();
            unsafe { xous_entry() };
        }
    };
}

#[cfg(target_os = "none")]
#[macro_export]
macro_rules! maybe_main {
    () => {
        use core::panic::PanicInfo;

        #[panic_handler]
        fn handle_panic(_arg: &PanicInfo) -> ! {
            // println!("PANIC!");
            // println!("Details: {:?}", arg);
            xous::syscall::wait_event();
            loop {}
        }

        extern "Rust" {
            fn xous_entry() -> !;
        }

        #[export_name = "_start"]
        pub extern "C" fn _start() {
            unsafe { xous_entry() };
        }
    };
}
