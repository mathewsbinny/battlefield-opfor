//! `trie` subsystem compatibility guards for `opfor-host`.
#![allow(dead_code, non_snake_case, unused_imports)]

use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use opfor_vfs::ModeId;

use super::*;

// Trie parent-walk self-cycle guard (`0x004BCBE7`: `mov ecx, [esi+0xc]; test ecx, ecx`)
const TRIE_PARENT_CYCLE: usize = 0x004B_CBE7;

const TRIE_PARENT_CYCLE_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x0C, 0x85, 0xC9];

const TRIE_PARENT_CYCLE_CONT: usize = 0x004B_CBEC;

// Trie::insert thread-safety lock (`0x004C36D0`)
const TRIE_INSERT_LOCK: usize = 0x004C_36D0;

const TRIE_INSERT_LOCK_PROLOGUE: [u8; 8] = [0x83, 0xEC, 0x10, 0x53, 0x8B, 0x5C, 0x24, 0x20];

// Small-block pool slab recovery (`0x0041A1FC`, `0x0041A306`, `0x00427406`)
// Recovers cleanly with a fresh 4 KB slab if a corrupted free-list pointer is encountered.
const POOL_POP1_GUARD: usize = 0x0041_A1FC;

const POOL_POP1_PROLOGUE: [u8; 5] = [0x8B, 0x7E, 0x20, 0x8B, 0x07];

const POOL_POP1_CONT: usize = 0x0041_A201;

const POOL_POP1_REFILL: usize = 0x0041_A1E3;

const POOL_POP2_GUARD: usize = 0x0041_A306;

const POOL_POP2_PROLOGUE: [u8; 5] = [0x8B, 0x46, 0x20, 0x8B, 0x08];

const POOL_POP2_CONT: usize = 0x0041_A30B;

const POOL_POP2_REFILL: usize = 0x0041_A2ED;

const POOL_COUNT_GUARD: usize = 0x0042_7406;

const POOL_COUNT_PROLOGUE: [u8; 5] = [0x8B, 0x00, 0x83, 0xC5, 0x01];

const POOL_COUNT_CONT: usize = 0x0042_740B;

const POOL_COUNT_DONE: usize = 0x0042_740F;

const POOL_COLLECT_GUARD: usize = 0x0042_7510;

const POOL_COLLECT_PROLOGUE: [u8; 7] = [0x89, 0x01, 0x8B, 0x00, 0x83, 0xC1, 0x04];

const POOL_COLLECT_CONT: usize = 0x0042_7517;

const POOL_COLLECT_DONE: usize = 0x0042_751B;

// Trie sibling-unlink null-end guard (`0x004C36A1`: `mov eax, [eax+4]; cmp [eax+4], esi`)
const TRIE_UNLINK_GUARD: usize = 0x004C_36A1;

const TRIE_UNLINK_PROLOGUE: [u8; 6] = [0x8B, 0x40, 0x04, 0x39, 0x70, 0x04];

const TRIE_UNLINK_CONT: usize = 0x004C_36A7;

const TRIE_UNLINK_SKIP: usize = 0x004C_36AC;

// SP Trie::find null string guard (`0x004C3ABC`: `mov edx, edi; mov eax, ebp; sub edx, ebp`)
const SP_TRIE_FIND_GUARD: usize = 0x004C_3ABC;

const SP_TRIE_FIND_PROLOGUE: [u8; 6] = [0x8B, 0xD7, 0x8B, 0xC5, 0x2B, 0xD5];

const SP_TRIE_FIND_CONT: usize = 0x004C_3AC2;

const SP_TRIE_FIND_SKIP: usize = 0x004C_3AD8;

// SP Trie::insert null string guard (`0x004C37BB`: `mov ecx, ebx; mov [esp+0x10], ebx`)
const SP_TRIE_INS_GUARD: usize = 0x004C_37BB;

const SP_TRIE_INS_PROLOGUE: [u8; 6] = [0x8B, 0xCB, 0x89, 0x5C, 0x24, 0x10];

const SP_TRIE_INS_CONT: usize = 0x004C_37C1;

const SP_TRIE_INS_SKIP: usize = 0x004C_37F1;

// Recursive node-chain cycle/ptr guard (`0x004BCBE0`: `mov eax, [esp+4]; push esi; mov esi, ecx`)
const SP_NODE_WALK_GUARD: usize = 0x004B_CBE0;

const SP_NODE_WALK_PROLOGUE: [u8; 7] = [0x8B, 0x44, 0x24, 0x04, 0x56, 0x8B, 0xF1];

// Trie sibling-unlink corrupted-ptr guard (`0x004C3678`: `mov [eax+0xc], edi; mov eax, [eax+4]`)
const TRIE_UNLINK2_GUARD: usize = 0x004C_3678;

const TRIE_UNLINK2_PROLOGUE: [u8; 6] = [0x89, 0x78, 0x0C, 0x8B, 0x40, 0x04];

const TRIE_UNLINK2_CONT: usize = 0x004C_367E;

const TRIE_UNLINK2_SKIP: usize = 0x004C_36AC;

// Trie::find corrupted node pointer guard (`0x004C3AA0`: `cmp esi, ebx; je 0x4c3afa; test byte [esi+8], 1`)
const TRIE_FIND_PTR_GUARD: usize = 0x004C_3AA0;

const TRIE_FIND_PTR_PROLOGUE: [u8; 8] = [0x3B, 0xF3, 0x74, 0x56, 0xF6, 0x46, 0x08, 0x01];

const TRIE_FIND_PTR_CONT: usize = 0x004C_3AA8;

const TRIE_FIND_PTR_NOTFOUND: usize = 0x004C_3AFA;

// Trie::firstLeaf corrupted node pointer guard (`0x004BCBC2`: `mov eax, ecx; and eax, -4; mov ecx, [eax+8]`)
const TRIE_LEAF_GUARD: usize = 0x004B_CBC2;

const TRIE_LEAF_PROLOGUE: [u8; 8] = [0x8B, 0xC1, 0x83, 0xE0, 0xFC, 0x8B, 0x48, 0x08];

const TRIE_LEAF_CONT: usize = 0x004B_CBCA;

// TrieNode::next parent-cycle guard (`0x004BCBA3`: `mov edx, [ecx+0xc]; test edx, edx`)
const TRIE_NEXT_CYCLE_GUARD: usize = 0x004B_CBA3;

const TRIE_NEXT_CYCLE_PROLOGUE: [u8; 5] = [0x8B, 0x51, 0x0C, 0x85, 0xD2];

const TRIE_NEXT_CYCLE_CONT: usize = 0x004B_CBA8;

const TRIE_NEXT_CYCLE_DONE: usize = 0x004B_CBD4;

// Trie::insert corrupted child-node pointer guard (`0x004C37A0`: `test byte [ebp+8], 1; jne +5`)
const TRIE_INS_PTR_GUARD: usize = 0x004C_37A0;

const TRIE_INS_PTR_PROLOGUE: [u8; 6] = [0xF6, 0x45, 0x08, 0x01, 0x75, 0x05];

const TRIE_INS_PTR_ZERO: usize = 0x004C_37A6;

const TRIE_INS_PTR_ONE: usize = 0x004C_37AB;

const TRIE_INS_PTR_END: usize = 0x004C_37FC;

// TrieNode::appendKey corrupt key-pointer guard (`0x004BCB5D`: `mov edx, [ecx]; mov eax, edx; lea esi, [eax+1]`)
const TRIE_KEY_4BCB5D_GUARD: usize = 0x004B_CB5D;

const TRIE_KEY_4BCB5D_PROLOGUE: [u8; 7] = [0x8B, 0x11, 0x8B, 0xC2, 0x8D, 0x70, 0x01];

const TRIE_KEY_4BCB5D_CONT: usize = 0x004B_CB64;

// Bounds-checked TrieNode::appendKey (`0x004BCB30`) + depth-capped/locked TrieNode::buildPath (`0x004BCBE0`)
const TRIE_APPEND_KEY_GUARD: usize = 0x004B_CB30;

const TRIE_APPEND_KEY_PROLOGUE: [u8; 5] = [0xF6, 0x41, 0x08, 0x01, 0x56];

const TRIE_BUILD_PATH_GUARD: usize = 0x004B_CBE0;

const TRIE_BUILD_PATH_PROLOGUE: [u8; 5] = [0x8B, 0x44, 0x24, 0x04, 0x56];

// Trie::find (`0x004C3A80`) & Trie::remove (`0x004C3B10`) thread-safety locks (shared with TRIE_INSERT_LOCK)
const TRIE_FIND_LOCK: usize = 0x004C_3A80;

const TRIE_FIND_LOCK_PROLOGUE: [u8; 5] = [0x83, 0xEC, 0x08, 0x53, 0x55];

const TRIE_REMOVE_LOCK: usize = 0x004C_3B10;

const TRIE_REMOVE_LOCK_PROLOGUE: [u8; 7] = [0x8B, 0x54, 0x24, 0x08, 0x83, 0xEC, 0x08];

// Resource ref-list corrupted `esi` guard (`0x004968C0`: `mov eax, [esi+0xc]; test eax, eax`)
const SP_LIST_4968_GUARD: usize = 0x0049_68C0;

const SP_LIST_4968_PROLOGUE: [u8; 5] = [0x8B, 0x46, 0x0C, 0x85, 0xC0];

const SP_LIST_4968_CONT: usize = 0x0049_68C5;

const SP_LIST_4968_EXIT: usize = 0x0049_69A2;

// Small-block pool ASCII-pointer write guard (`0x004CF9A2`: `test eax, eax; jz +0xb; mov ecx, [eax-8]`)
const SP_POOL_4CF9_GUARD: usize = 0x004C_F9A2;

const SP_POOL_4CF9_PROLOGUE: [u8; 7] = [0x85, 0xC0, 0x74, 0x0B, 0x8B, 0x48, 0xF8];

const SP_POOL_4CF9_AFTER: usize = 0x004C_F9B1;

// Trie::find null-parent `this=0x38` guard (`0x004C8950`: `mov eax, [esp+8]; sub esp, 0x400`)
const SP_TRIE_4C8950_GUARD: usize = 0x004C_8950;

const SP_TRIE_4C8950_PROLOGUE: [u8; 10] = [0x8B, 0x44, 0x24, 0x08, 0x81, 0xEC, 0x00, 0x04, 0x00, 0x00];

const SP_TRIE_4C8950_CONT: usize = 0x004C_895A;

// Sibling Trie null-parent `this` guards (`0x004C89B0`, `0x004C8A10`, `0x004C8A70`, `0x004C8AD0`, `0x004C8C80`)
const SP_TRIE_4C89B0_GUARD: usize = 0x004C_89B0;

const SP_TRIE_4C89B0_PROLOGUE: [u8; 10] = [0x8B, 0x44, 0x24, 0x04, 0x81, 0xEC, 0x00, 0x04, 0x00, 0x00];

const SP_TRIE_4C89B0_CONT: usize = 0x004C_89BA;

const SP_TRIE_4C8A10_GUARD: usize = 0x004C_8A10;

const SP_TRIE_4C8A10_PROLOGUE: [u8; 10] = [0x8B, 0x44, 0x24, 0x04, 0x81, 0xEC, 0x00, 0x04, 0x00, 0x00];

const SP_TRIE_4C8A10_CONT: usize = 0x004C_8A1A;

const SP_TRIE_4C8A70_GUARD: usize = 0x004C_8A70;

const SP_TRIE_4C8A70_PROLOGUE: [u8; 10] = [0x8B, 0x44, 0x24, 0x04, 0x81, 0xEC, 0x00, 0x04, 0x00, 0x00];

const SP_TRIE_4C8A70_CONT: usize = 0x004C_8A7A;

const SP_TRIE_4C8AD0_GUARD: usize = 0x004C_8AD0;

const SP_TRIE_4C8AD0_PROLOGUE: [u8; 7] = [0x83, 0xEC, 0x0C, 0x53, 0x56, 0x8B, 0xF1];

const SP_TRIE_4C8AD0_CONT: usize = 0x004C_8AD7;

const SP_TRIE_4C8C80_GUARD: usize = 0x004C_8C80;

const SP_TRIE_4C8C80_PROLOGUE: [u8; 6] = [0x81, 0xEC, 0xF4, 0x00, 0x00, 0x00];

const SP_TRIE_4C8C80_CONT: usize = 0x004C_8C86;

// Trie/allocator ASCII pointer guard (`0x004CF9A6`: `mov ecx, [eax-8]; mov [eax], ecx`)
const SP_4CF9A6_GUARD: usize = 0x004C_F9A6;

const SP_4CF9A6_PROLOGUE: [u8; 5] = [0x8B, 0x48, 0xF8, 0x89, 0x08];

const SP_4CF9A6_CONT: usize = 0x004C_F9AB;

const SP_4CF9A6_SKIP: usize = 0x004C_F9B1;

#[cfg(target_arch = "x86")]
type TrieInsertFn = unsafe extern "fastcall" fn(usize, *mut c_void, *mut c_void, *mut c_void, *const u8) -> *mut c_void;

#[cfg(target_arch = "x86")]
static ORIG_TRIE_INSERT_LOCK: OnceLock<TrieInsertFn> = OnceLock::new();

#[cfg(target_arch = "x86")]
type TrieFindFn = unsafe extern "fastcall" fn(usize, *mut c_void, *mut c_void, *const u8) -> *mut c_void;

#[cfg(target_arch = "x86")]
static ORIG_TRIE_FIND_LOCK: OnceLock<TrieFindFn> = OnceLock::new();

#[cfg(target_arch = "x86")]
type TrieRemoveFn = unsafe extern "fastcall" fn(usize, *mut c_void, *mut c_void, *const u8) -> usize;

#[cfg(target_arch = "x86")]
static ORIG_TRIE_REMOVE_LOCK: OnceLock<TrieRemoveFn> = OnceLock::new();

#[cfg(target_arch = "x86")]
type TrieClearFn = unsafe extern "fastcall" fn(usize, *mut c_void, u32, u32);

#[cfg(target_arch = "x86")]
static ORIG_TRIE_CLEAR_LOCK: OnceLock<TrieClearFn> = OnceLock::new();

#[cfg(target_arch = "x86")]
type TrieCompactFn = unsafe extern "fastcall" fn(usize, *mut c_void);

#[cfg(target_arch = "x86")]
static ORIG_TRIE_COMPACT_LOCK: OnceLock<TrieCompactFn> = OnceLock::new();

#[cfg(target_arch = "x86")]
static TRIE_LOCK_OWNER: AtomicUsize = AtomicUsize::new(0);

#[cfg(target_arch = "x86")]
static TRIE_LOCK_DEPTH: AtomicUsize = AtomicUsize::new(0);

#[cfg(target_arch = "x86")]
#[inline]
fn trie_lock_acquire() {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadId() -> u32;
    }
    let tid = unsafe { GetCurrentThreadId() } as usize;
    if TRIE_LOCK_OWNER.load(Ordering::Acquire) != tid {
        while TRIE_LOCK_OWNER
            .compare_exchange_weak(0, tid, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            std::hint::spin_loop();
        }
    }
    TRIE_LOCK_DEPTH.fetch_add(1, Ordering::Relaxed);
}

#[cfg(target_arch = "x86")]
#[inline]
fn trie_lock_release() {
    if TRIE_LOCK_DEPTH.fetch_sub(1, Ordering::Relaxed) == 1 {
        TRIE_LOCK_OWNER.store(0, Ordering::Release);
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_trie_insert_lock(
    this: usize,
    edx: *mut c_void,
    out: *mut c_void,
    root: *mut c_void,
    name: *const u8,
) -> *mut c_void {
    trie_lock_acquire();
    let ret = if let Some(&orig) = ORIG_TRIE_INSERT_LOCK.get() {
        unsafe { orig(this, edx, out, root, name) }
    } else {
        out
    };
    trie_lock_release();
    ret
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_trie_find_lock(
    this: usize,
    edx: *mut c_void,
    root: *mut c_void,
    name: *const u8,
) -> *mut c_void {
    trie_lock_acquire();
    let ret = if let Some(&orig) = ORIG_TRIE_FIND_LOCK.get() {
        unsafe { orig(this, edx, root, name) }
    } else {
        std::ptr::null_mut()
    };
    trie_lock_release();
    ret
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_trie_remove_lock(
    this: usize,
    edx: *mut c_void,
    root: *mut c_void,
    name: *const u8,
) -> usize {
    trie_lock_acquire();
    let ret = if let Some(&orig) = ORIG_TRIE_REMOVE_LOCK.get() {
        unsafe { orig(this, edx, root, name) }
    } else {
        0
    };
    trie_lock_release();
    ret
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_trie_clear_lock(
    this: usize,
    edx: *mut c_void,
    a: u32,
    b: u32,
) {
    if this < 0x10000 {
        return;
    }
    trie_lock_acquire();
    if let Some(&orig) = ORIG_TRIE_CLEAR_LOCK.get() {
        unsafe { orig(this, edx, a, b) };
    }
    trie_lock_release();
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_trie_compact_lock(this: usize, edx: *mut c_void) {
    if this < 0x10000 {
        return;
    }
    trie_lock_acquire();
    if let Some(&orig) = ORIG_TRIE_COMPACT_LOCK.get() {
        unsafe { orig(this, edx) };
    }
    trie_lock_release();
}

#[cfg(target_arch = "x86")]
#[inline]
unsafe fn trie_node_valid(p: usize) -> bool {
    if p < 0x10000 || p >= 0x7fff_0000 || (p & 3) != 0 {
        return false;
    }
    if p >= 0x6000_0000 && unsafe { opfor_is_bad_write_ptr(p) } != 0 {
        return false;
    }
    true
}

#[cfg(target_arch = "x86")]
unsafe fn trie_append_key_bounded(node: usize, out: *mut u8, end: *mut u8) -> *mut u8 {
    let out_addr = out as usize;
    if out_addr < 0x10000 {
        return out;
    }
    let end_addr = end as usize;
    let limit = if end_addr > out_addr && end_addr - out_addr <= 0x10000 {
        end_addr - 1
    } else {
        out_addr
    };
    let mut cur = out_addr;
    if unsafe { trie_node_valid(node) } {
        let flags = unsafe { *((node + 8) as *const u32) };
        if (flags & 1) != 0 {
            for i in 0..4usize {
                let b = unsafe { *((node + i) as *const u8) };
                if b == 0 || cur >= limit {
                    break;
                }
                unsafe { *(cur as *mut u8) = b };
                cur += 1;
            }
        } else {
            let s = unsafe { *(node as *const usize) };
            let s_ok = s >= 0x10000
                && s < 0x7fff_0000
                && (s < 0x6000_0000 || unsafe { opfor_is_bad_ptr(s) } == 0);
            if s_ok {
                let mut i = 0usize;
                while cur < limit && i < 512 {
                    let b = unsafe { *((s + i) as *const u8) };
                    if b == 0 {
                        break;
                    }
                    unsafe { *(cur as *mut u8) = b };
                    cur += 1;
                    i += 1;
                }
            }
        }
    }
    if cur < end_addr || cur == out_addr {
        unsafe { *(cur as *mut u8) = 0 };
    }
    cur as *mut u8
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_trie_append_key(
    node: usize,
    _edx: *mut c_void,
    out: *mut u8,
    end: *mut u8,
) -> *mut u8 {
    unsafe { trie_append_key_bounded(node, out, end) }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_trie_build_path(
    node: usize,
    _edx: *mut c_void,
    out: *mut u8,
    end: *mut u8,
) -> *mut u8 {
    trie_lock_acquire();
    let mut chain = [0usize; 64];
    let mut len = 0usize;
    let mut cur = node;
    while len < 64 && unsafe { trie_node_valid(cur) } {
        let mut seen = false;
        for &prev in &chain[..len] {
            if prev == cur {
                seen = true;
                break;
            }
        }
        if seen {
            break;
        }
        chain[len] = cur;
        len += 1;
        cur = unsafe { *((cur + 0x0c) as *const usize) };
    }
    let mut write_ptr = out;
    if len == 0 {
        let out_addr = out as usize;
        if out_addr >= 0x10000 {
            unsafe { *out = 0 };
        }
    } else {
        for i in (0..len).rev() {
            write_ptr = unsafe { trie_append_key_bounded(chain[i], write_ptr, end) };
        }
    }
    trie_lock_release();
    write_ptr
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn pool_pop1_guard_entry() {
    std::arch::naked_asm!(
        "pushfd",
        "mov edi, dword ptr [esi + 0x20]",
        "cmp edi, 0x02000000",
        "jb 4f",
        "cmp edi, 0x7fff0000",
        "ja 4f",
        "test edi, 3",
        "jnz 4f",
        "cmp edi, 0x60000000",
        "jb 5f",
        "push eax",
        "push ecx",
        "push edx",
        "push edi",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "pop eax",
        "jnz 4f",
        "5:",
        "mov eax, dword ptr [edi]",
        "test eax, eax",
        "jz 2f",
        "cmp eax, 0x02000000",
        "jb 3f",
        "cmp eax, 0x7fff0000",
        "ja 3f",
        "test eax, 3",
        "jnz 3f",
        "cmp eax, 0x60000000",
        "jb 2f",
        "push eax",
        "push ecx",
        "push edx",
        "push eax",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "pop eax",
        "jnz 3f",
        "2:",
        "popfd",
        "push {cont}",
        "ret",
        "3:",
        "xor eax, eax",
        "popfd",
        "push {cont}",
        "ret",
        "4:",
        "mov dword ptr [esi + 0x20], 0",
        "popfd",
        "push {refill}",
        "ret",
        bad = sym opfor_is_bad_pool_ptr,
        cont = const POOL_POP1_CONT,
        refill = const POOL_POP1_REFILL,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn pool_pop2_guard_entry() {
    std::arch::naked_asm!(
        "pushfd",
        "mov eax, dword ptr [esi + 0x20]",
        "cmp eax, 0x02000000",
        "jb 4f",
        "cmp eax, 0x7fff0000",
        "ja 4f",
        "test eax, 3",
        "jnz 4f",
        "cmp eax, 0x60000000",
        "jb 6f",
        "push eax",
        "push ecx",
        "push edx",
        "push eax",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "pop eax",
        "jnz 4f",
        "6:",
        "mov ecx, dword ptr [eax]",
        "test ecx, ecx",
        "jz 2f",
        "cmp ecx, 0x02000000",
        "jb 3f",
        "cmp ecx, 0x7fff0000",
        "ja 3f",
        "test ecx, 3",
        "jnz 3f",
        "cmp ecx, 0x60000000",
        "jb 7f",
        "push eax",
        "push ecx",
        "push edx",
        "push ecx",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "pop eax",
        "jnz 3f",
        "7:",
        "push edx",
        "mov edx, ecx",
        "xor edx, eax",
        "cmp edx, 0x00100000",
        "jbe 5f",
        "mov edx, ecx",
        "shr edx, 8",
        "and edx, 0xffff",
        "cmp edx, 0x0101",
        "pop edx",
        "je 3f",
        "jmp 2f",
        "5:",
        "pop edx",
        "2:",
        "popfd",
        "push {cont}",
        "ret",
        "3:",
        "xor ecx, ecx",
        "popfd",
        "push {cont}",
        "ret",
        "4:",
        "mov dword ptr [esi + 0x20], 0",
        "popfd",
        "push {refill}",
        "ret",
        bad = sym opfor_is_bad_pool_ptr,
        cont = const POOL_POP2_CONT,
        refill = const POOL_POP2_REFILL,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn pool_count_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x02000000",
        "jb 2f",
        "cmp eax, 0x7fff0000",
        "ja 2f",
        "test eax, 3",
        "jnz 2f",
        "cmp eax, 0x656a626f",
        "je 2f",
        "cmp eax, 0x61746164",
        "je 2f",
        "cmp eax, 0x60000000",
        "jae 4f",
        "push edx",
        "mov edx, eax",
        "shr edx, 8",
        "and edx, 0xffff",
        "cmp edx, 0x0101",
        "pop edx",
        "jne 3f",
        "4:",
        "push ecx",
        "push edx",
        "push eax",
        "push eax",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop eax",
        "pop edx",
        "pop ecx",
        "jnz 2f",
        "3:",
        "mov eax, dword ptr [eax]",
        "add ebp, 1",
        "push {cont}",
        "ret",
        "2:",
        "push {done}",
        "ret",
        bad = sym opfor_is_bad_pool_ptr,
        cont = const POOL_COUNT_CONT,
        done = const POOL_COUNT_DONE,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn pool_collect_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x02000000",
        "jb 2f",
        "cmp eax, 0x7fff0000",
        "ja 2f",
        "test eax, 3",
        "jnz 2f",
        "cmp eax, 0x656a626f",
        "je 2f",
        "cmp eax, 0x61746164",
        "je 2f",
        "cmp eax, 0x60000000",
        "jae 4f",
        "push edx",
        "mov edx, eax",
        "shr edx, 8",
        "and edx, 0xffff",
        "cmp edx, 0x0101",
        "pop edx",
        "jne 3f",
        "4:",
        "push ecx",
        "push edx",
        "push eax",
        "push eax",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop eax",
        "pop edx",
        "pop ecx",
        "jnz 2f",
        "3:",
        "mov dword ptr [ecx], eax",
        "mov eax, dword ptr [eax]",
        "add ecx, 4",
        "push {cont}",
        "ret",
        "2:",
        "push {done}",
        "ret",
        bad = sym opfor_is_bad_pool_ptr,
        cont = const POOL_COLLECT_CONT,
        done = const POOL_COLLECT_DONE,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn trie_unlink_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [eax + 4]",
        "test eax, eax",
        "jz 2f",
        "cmp dword ptr [eax + 4], esi",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const TRIE_UNLINK_CONT,
        skip = const TRIE_UNLINK_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_trie_find_guard_entry() {
    std::arch::naked_asm!(
        "cmp edi, 0x10000",
        "jb 2f",
        "mov edx, edi",
        "mov eax, ebp",
        "sub edx, ebp",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_TRIE_FIND_CONT,
        skip = const SP_TRIE_FIND_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_trie_ins_guard_entry() {
    std::arch::naked_asm!(
        "cmp ebx, 0x10000",
        "jb 2f",
        "cmp ebx, 0x7fff0000",
        "jae 2f",
        "mov ecx, ebx",
        "mov dword ptr [esp + 0x10], ebx",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_TRIE_INS_CONT,
        skip = const SP_TRIE_INS_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn trie_parent_cycle_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0xc]",
        "cmp ecx, esi",
        "jne 2f",
        "xor ecx, ecx",
        "2:",
        "test ecx, ecx",
        "push {cont}",
        "ret",
        cont = const TRIE_PARENT_CYCLE_CONT,
    );
}

#[cfg(target_arch = "x86")]


#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn trie_unlink2_guard_entry() {
    std::arch::naked_asm!(
        "test al, 3",
        "jnz 2f",
        "cmp eax, 0x02000000",
        "jb 2f",
        "cmp eax, 0x50000000",
        "jae 2f",
        "mov dword ptr [eax + 0x0c], edi",
        "mov eax, dword ptr [eax + 4]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const TRIE_UNLINK2_CONT,
        skip = const TRIE_UNLINK2_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn trie_find_ptr_guard_entry() {
    std::arch::naked_asm!(
        "cmp esi, ebx",
        "je 2f",
        "test esi, 3",
        "jnz 2f",
        "cmp esi, 0x01000000",
        "jb 2f",
        "cmp esi, 0x60000000",
        "jb 3f",
        "push eax",
        "push ecx",
        "push edx",
        "lea eax, [esi + 8]",
        "push eax",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "pop eax",
        "jnz 2f",
        "3:",
        "test byte ptr [esi + 8], 1",
        "push {cont}",
        "ret",
        "2:",
        "push {notfound}",
        "ret",
        bad = sym opfor_is_bad_write_ptr,
        cont = const TRIE_FIND_PTR_CONT,
        notfound = const TRIE_FIND_PTR_NOTFOUND,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn trie_leaf_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, ecx",
        "and eax, -4",
        "cmp eax, 0x01000000",
        "jb 2f",
        "cmp eax, 0x60000000",
        "jb 3f",
        "push eax",
        "push ecx",
        "push edx",
        "lea ecx, [eax + 8]",
        "push ecx",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "pop eax",
        "jnz 2f",
        "3:",
        "mov ecx, dword ptr [eax + 8]",
        "push {cont}",
        "ret",
        "2:",
        "xor eax, eax",
        "ret",
        bad = sym opfor_is_bad_write_ptr,
        cont = const TRIE_LEAF_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn trie_next_cycle_guard_entry() {
    std::arch::naked_asm!(
        "mov edx, dword ptr [ecx + 0xc]",
        "cmp edx, 0x10000",
        "jb 2f",
        "cmp edx, ecx",
        "je 2f",
        "cmp dword ptr [edx + 0xc], ecx",
        "je 2f",
        "test edx, edx",
        "push {cont}",
        "ret",
        "2:",
        "push {done}",
        "ret",
        cont = const TRIE_NEXT_CYCLE_CONT,
        done = const TRIE_NEXT_CYCLE_DONE,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn trie_ins_ptr_guard_entry() {
    std::arch::naked_asm!(
        "test ebp, 3",
        "jnz 2f",
        "cmp ebp, 0x01000000",
        "jb 2f",
        "cmp ebp, 0x7fff0000",
        "jae 2f",
        "cmp ebp, 0x60000000",
        "jb 4f",
        "push eax",
        "push ecx",
        "push edx",
        "lea eax, [ebp + 8]",
        "push eax",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "pop eax",
        "jnz 2f",
        "4:",
        "test byte ptr [ebp + 8], 1",
        "jnz 3f",
        "push {zero}",
        "ret",
        "3:",
        "push {one}",
        "ret",
        "2:",
        "xor ebp, ebp",
        "push {end}",
        "ret",
        bad = sym opfor_is_bad_write_ptr,
        zero = const TRIE_INS_PTR_ZERO,
        one = const TRIE_INS_PTR_ONE,
        end = const TRIE_INS_PTR_END,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn trie_key_4bcb5d_guard_entry() {
    std::arch::naked_asm!(
        "mov edx, dword ptr [ecx]",
        "cmp edx, 0x10000",
        "jb 2f",
        "cmp edx, 0x7fff0000",
        "jae 2f",
        "cmp edx, 0x60000000",
        "jb 3f",
        "push eax",
        "push ecx",
        "push edx",
        "push edx",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "pop eax",
        "jnz 2f",
        "3:",
        "mov eax, edx",
        "lea esi, [eax + 1]",
        "push {cont}",
        "ret",
        "2:",
        "mov eax, dword ptr [esp + 8]",
        "mov byte ptr [eax], 0",
        "pop esi",
        "ret 8",
        bad = sym opfor_is_bad_ptr,
        cont = const TRIE_KEY_4BCB5D_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_list_4968_guard_entry() {
    std::arch::naked_asm!(
        "test esi, 3",
        "jnz 2f",
        "cmp esi, 0x01000000",
        "jb 2f",
        "cmp esi, 0x60000000",
        "jb 3f",
        "push ecx",
        "push edx",
        "push esi",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "jnz 2f",
        "3:",
        "mov eax, dword ptr [esi + 0x08]",
        "test eax, 3",
        "jnz 2f",
        "cmp eax, 0x01000000",
        "jb 2f",
        "cmp eax, 0x60000000",
        "jb 4f",
        "push eax",
        "push ecx",
        "push edx",
        "lea ecx, [eax + 0x1c]",
        "push ecx",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "pop eax",
        "jnz 2f",
        "4:",
        "mov eax, dword ptr [eax + 0x1c]",
        "test eax, 3",
        "jnz 2f",
        "cmp eax, 0x01000000",
        "jb 2f",
        "cmp eax, 0x60000000",
        "jb 5f",
        "push ecx",
        "push edx",
        "push eax",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "jnz 2f",
        "5:",
        "mov eax, dword ptr [esi + 0x0c]",
        "test eax, eax",
        "push {cont}",
        "ret",
        "2:",
        "push {exit}",
        "ret",
        bad = sym opfor_is_bad_ptr,
        cont = const SP_LIST_4968_CONT,
        exit = const SP_LIST_4968_EXIT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_pool_4cf9_guard_entry() {
    std::arch::naked_asm!(
        "test eax, eax",
        "jz 2f",
        "test eax, 3",
        "jnz 4f",
        "cmp eax, 0x01000000",
        "jb 4f",
        "cmp eax, 0x60000000",
        "jb 3f",
        "push eax",
        "push ecx",
        "push edx",
        "push eax",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "pop eax",
        "jnz 4f",
        "3:",
        "mov ecx, dword ptr [eax - 8]",
        "mov dword ptr [eax], ecx",
        "mov ecx, dword ptr [eax - 4]",
        "mov dword ptr [eax + 4], ecx",
        "2:",
        "push {after}",
        "ret",
        "4:",
        "pop edi",
        "pop esi",
        "pop ebx",
        "ret 8",
        bad = sym opfor_is_bad_write_ptr,
        after = const SP_POOL_4CF9_AFTER,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_trie_4c8950_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [esp + 8]",
        "sub esp, 0x400",
        "push {cont}",
        "ret",
        "2:",
        "mov eax, dword ptr [esp + 4]",
        "cmp eax, 0x10000",
        "jb 3f",
        "mov dword ptr [eax], 0",
        "3:",
        "ret 8",
        cont = const SP_TRIE_4C8950_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_trie_4c89b0_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [esp + 4]",
        "sub esp, 0x400",
        "push {cont}",
        "ret",
        "2:",
        "xor eax, eax",
        "ret 4",
        cont = const SP_TRIE_4C89B0_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_trie_4c8a10_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [esp + 4]",
        "sub esp, 0x400",
        "push {cont}",
        "ret",
        "2:",
        "xor eax, eax",
        "ret 4",
        cont = const SP_TRIE_4C8A10_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_trie_4c8a70_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [esp + 4]",
        "sub esp, 0x400",
        "push {cont}",
        "ret",
        "2:",
        "xor eax, eax",
        "ret 4",
        cont = const SP_TRIE_4C8A70_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_trie_4c8ad0_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "sub esp, 0xc",
        "push ebx",
        "push esi",
        "mov esi, ecx",
        "push {cont}",
        "ret",
        "2:",
        "ret 8",
        cont = const SP_TRIE_4C8AD0_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_trie_4c8c80_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "sub esp, 0xf4",
        "push {cont}",
        "ret",
        "2:",
        "ret",
        cont = const SP_TRIE_4C8C80_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_4cf9a6_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x10000",
        "jb 2f",
        "cmp eax, 0x60000000",
        "jb 3f",
        "push eax",
        "push ecx",
        "push edx",
        "push eax",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "pop eax",
        "jnz 2f",
        "3:",
        "mov ecx, dword ptr [eax - 8]",
        "mov dword ptr [eax], ecx",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        bad = sym opfor_is_bad_write_ptr,
        cont = const SP_4CF9A6_CONT,
        skip = const SP_4CF9A6_SKIP,
    );
}

pub(super) unsafe fn install(mode: ModeId) -> usize {
    let mut installed = 0usize;
    let is_console_port = matches!(mode, ModeId::Bf1943 | ModeId::Bfbc1);
    let is_bfbc1 = mode == ModeId::Bfbc1;
    #[cfg(target_arch = "x86")]
    {
        if !is_console_port {
            return installed;
        }
        if unsafe {
            crate::hooks::install_detour(
                TRIE_PARENT_CYCLE,
                &TRIE_PARENT_CYCLE_PROLOGUE,
                trie_parent_cycle_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                TRIE_INSERT_LOCK,
                &TRIE_INSERT_LOCK_PROLOGUE,
                detour_trie_insert_lock as *const c_void,
            )
        } {
            let _ = ORIG_TRIE_INSERT_LOCK
                .set(unsafe { std::mem::transmute::<*const c_void, TrieInsertFn>(tramp) });
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                POOL_POP1_GUARD,
                &POOL_POP1_PROLOGUE,
                pool_pop1_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                POOL_POP2_GUARD,
                &POOL_POP2_PROLOGUE,
                pool_pop2_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                POOL_COUNT_GUARD,
                &POOL_COUNT_PROLOGUE,
                pool_count_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                POOL_COLLECT_GUARD,
                &POOL_COLLECT_PROLOGUE,
                pool_collect_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                TRIE_UNLINK_GUARD,
                &TRIE_UNLINK_PROLOGUE,
                trie_unlink_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_TRIE_FIND_GUARD,
                &SP_TRIE_FIND_PROLOGUE,
                sp_trie_find_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_TRIE_INS_GUARD,
                &SP_TRIE_INS_PROLOGUE,
                sp_trie_ins_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                SP_NODE_WALK_GUARD,
                &SP_NODE_WALK_PROLOGUE,
                detour_trie_build_path as *const c_void,
            )
        } {
            let _ = ORIG_NODE_WALK.set(unsafe {
                std::mem::transmute::<
                    *const c_void,
                    unsafe extern "fastcall" fn(usize, *mut c_void, usize, usize) -> usize,
                >(tramp)
            });
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                TRIE_UNLINK2_GUARD,
                &TRIE_UNLINK2_PROLOGUE,
                trie_unlink2_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                TRIE_FIND_PTR_GUARD,
                &TRIE_FIND_PTR_PROLOGUE,
                trie_find_ptr_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                TRIE_LEAF_GUARD,
                &TRIE_LEAF_PROLOGUE,
                trie_leaf_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                TRIE_NEXT_CYCLE_GUARD,
                &TRIE_NEXT_CYCLE_PROLOGUE,
                trie_next_cycle_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                TRIE_INS_PTR_GUARD,
                &TRIE_INS_PTR_PROLOGUE,
                trie_ins_ptr_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                TRIE_KEY_4BCB5D_GUARD,
                &TRIE_KEY_4BCB5D_PROLOGUE,
                trie_key_4bcb5d_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                TRIE_APPEND_KEY_GUARD,
                &TRIE_APPEND_KEY_PROLOGUE,
                detour_trie_append_key as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                TRIE_FIND_LOCK,
                &TRIE_FIND_LOCK_PROLOGUE,
                detour_trie_find_lock as *const c_void,
            )
        } {
            let _ = ORIG_TRIE_FIND_LOCK
                .set(unsafe { std::mem::transmute::<*const c_void, TrieFindFn>(tramp) });
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                TRIE_REMOVE_LOCK,
                &TRIE_REMOVE_LOCK_PROLOGUE,
                detour_trie_remove_lock as *const c_void,
            )
        } {
            let _ = ORIG_TRIE_REMOVE_LOCK
                .set(unsafe { std::mem::transmute::<*const c_void, TrieRemoveFn>(tramp) });
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_LIST_4968_GUARD,
                &SP_LIST_4968_PROLOGUE,
                sp_list_4968_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_POOL_4CF9_GUARD,
                &SP_POOL_4CF9_PROLOGUE,
                sp_pool_4cf9_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_TRIE_4C8950_GUARD,
                &SP_TRIE_4C8950_PROLOGUE,
                sp_trie_4c8950_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_TRIE_4C89B0_GUARD,
                &SP_TRIE_4C89B0_PROLOGUE,
                sp_trie_4c89b0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_TRIE_4C8A10_GUARD,
                &SP_TRIE_4C8A10_PROLOGUE,
                sp_trie_4c8a10_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_TRIE_4C8A70_GUARD,
                &SP_TRIE_4C8A70_PROLOGUE,
                sp_trie_4c8a70_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                SP_TRIE_4C8AD0_GUARD,
                &SP_TRIE_4C8AD0_PROLOGUE,
                detour_trie_clear_lock as *const c_void,
            )
        } {
            let _ = ORIG_TRIE_CLEAR_LOCK
                .set(unsafe { std::mem::transmute::<*const c_void, TrieClearFn>(tramp) });
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                SP_TRIE_4C8C80_GUARD,
                &SP_TRIE_4C8C80_PROLOGUE,
                detour_trie_compact_lock as *const c_void,
            )
        } {
            let _ = ORIG_TRIE_COMPACT_LOCK
                .set(unsafe { std::mem::transmute::<*const c_void, TrieCompactFn>(tramp) });
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_4CF9A6_GUARD,
                &SP_4CF9A6_PROLOGUE,
                sp_4cf9a6_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
    }
    let _ = (is_console_port, is_bfbc1);
    installed
}
