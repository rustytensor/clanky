//! Owned, non-debuggable storage for bytes that may contain secrets.
use std::sync::atomic::{Ordering, compiler_fence};

pub(crate) struct Secret {
    bytes: Box<[u8]>,
}

impl Secret {
    pub(crate) fn zeroed(len: usize) -> Self {
        Self {
            bytes: vec![0; len].into_boxed_slice(),
        }
    }

    pub(crate) fn copy(bytes: &[u8]) -> Self {
        let mut result = Self::zeroed(bytes.len());
        result.bytes.copy_from_slice(bytes);
        result
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(crate) fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }
}

/// Isolated zeroization primitive. The slice is live and exclusively borrowed,
/// so every volatile write is in bounds and targets initialized writable memory.
/// The fence prevents the compiler from moving subsequent memory use before it.
pub(crate) fn wipe(bytes: &mut [u8]) {
    for byte in bytes {
        // SAFETY: `byte` is a valid exclusive reference for this write.
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
    compiler_fence(Ordering::SeqCst);
}

impl Drop for Secret {
    fn drop(&mut self) {
        wipe(&mut self.bytes);
    }
}
