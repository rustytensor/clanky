//! Linux-only operating-system boundary.
#[cfg(not(target_os = "linux"))]
compile_error!("Clanky M1 supports Linux only");
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;
