//! Verified 32-bit x86 inline detour & code patcher (pure Rust, zero external crates).

#![allow(dead_code)]

use std::ffi::c_void;

use crate::win32::{
    self, DWORD, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE,
};

/// Compute the 32-bit relative displacement for an x86 `E9 rel32` jump from `from` to `to`.
#[must_use]
pub const fn rel32(from: usize, to: usize) -> i32 {
    (to as isize - (from as isize + 5)) as i32
}

/// Check whether `addr` holds the exact `expected` bytes.
///
/// SAFETY: Caller must ensure `addr..addr+expected.len()` is readable mapped memory.
pub unsafe fn bytes_match(addr: usize, expected: &[u8]) -> bool {
    let slice = unsafe { std::slice::from_raw_parts(addr as *const u8, expected.len()) };
    slice == expected
}

/// Write `bytes` to executable code memory at `addr` after verifying `expected`,
/// restoring page protection and flushing the instruction cache.
///
/// SAFETY: Caller must ensure `addr` is a valid code address in the target image.
pub unsafe fn patch_verified(addr: usize, expected: &[u8], replacement: &[u8]) -> bool {
    if expected.len() != replacement.len() || replacement.is_empty() {
        return false;
    }
    if !unsafe { bytes_match(addr, expected) } {
        return false;
    }
    unsafe { write_code(addr, replacement) }
}

/// Write `bytes` to executable code memory at `addr`.
pub unsafe fn write_code(addr: usize, bytes: &[u8]) -> bool {
    let mut old: DWORD = 0;
    let ptr = addr as *mut c_void;
    if unsafe { win32::VirtualProtect(ptr, bytes.len(), PAGE_EXECUTE_READWRITE, &mut old) } == 0 {
        return false;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), addr as *mut u8, bytes.len());
        let mut dummy: DWORD = 0;
        win32::VirtualProtect(ptr, bytes.len(), old, &mut dummy);
        win32::FlushInstructionCache(win32::GetCurrentProcess(), ptr, bytes.len());
    }
    true
}

/// Install a verified x86 `E9 rel32` inline detour at `target`.
///
/// Allocates an executable trampoline containing `expected_prologue` followed by
/// `jmp target + expected_prologue.len()`, then overwrites `target` with
/// `jmp detour` + NOP padding.
///
/// SAFETY: `expected_prologue` must be >= 5 bytes and contain only position-independent
/// instructions (no PC-relative calls/jumps).
pub unsafe fn install_detour(
    target: usize,
    expected_prologue: &[u8],
    detour: *const c_void,
) -> Option<*const c_void> {
    let n = expected_prologue.len();
    if n < 5 || n > 32 {
        return None;
    }
    if !unsafe { bytes_match(target, expected_prologue) } {
        return None;
    }

    let tramp = unsafe {
        win32::VirtualAlloc(
            std::ptr::null_mut(),
            64,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_EXECUTE_READWRITE,
        ) as *mut u8
    };
    if tramp.is_null() {
        return None;
    }

    unsafe {
        // Trampoline: [original prologue] [E9 rel32 -> target + n]
        std::ptr::copy_nonoverlapping(expected_prologue.as_ptr(), tramp, n);
        let jmp_back = rel32(tramp as usize + n, target + n).to_le_bytes();
        *tramp.add(n) = 0xE9;
        std::ptr::copy_nonoverlapping(jmp_back.as_ptr(), tramp.add(n + 1), 4);
        win32::FlushInstructionCache(win32::GetCurrentProcess(), tramp.cast(), n + 5);

        // Target: [E9 rel32 -> detour] [NOP ...]
        let mut patch = vec![0x90u8; n];
        patch[0] = 0xE9;
        let jmp_to = rel32(target, detour as usize).to_le_bytes();
        patch[1..5].copy_from_slice(&jmp_to);
        if !write_code(target, &patch) {
            return None;
        }
    }

    Some(tramp.cast())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rel32_math_forward_and_backward() {
        assert_eq!(rel32(0x0075_FCB0, 0x0075_FCB5), 0);
        assert_eq!(rel32(0x0075_FCB0, 0x0075_FCC0), 11);
        assert_eq!(rel32(0x0075_FCC0, 0x0075_FCB0), -21);
    }
}
