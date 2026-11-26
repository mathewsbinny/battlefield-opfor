//! Manual 32-bit x86 PE loader for mapping `BFBC2Game.exe` (build 795745) at its
//! fixed native address (`0x00400000`) inside `OPFOREngine.exe` (`/BASE:0x10000000`).

use std::ffi::{CString, c_void};
use std::fs;
use std::path::Path;

use crate::imports;
use crate::win32::{
    self, DWORD, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READ,
    PAGE_EXECUTE_READWRITE, PAGE_NOACCESS, PAGE_READONLY, PAGE_READWRITE,
};

const IMAGE_SCN_MEM_EXECUTE: u32 = 0x2000_0000;
const IMAGE_SCN_MEM_READ: u32 = 0x4000_0000;
const IMAGE_SCN_MEM_WRITE: u32 = 0x8000_0000;

#[derive(Debug, Clone)]
pub struct MappedImage {
    pub image_base: usize,
    pub size_of_image: usize,
    pub entry_point: usize,
    pub section_count: usize,
    pub dll_count: usize,
    pub import_count: usize,
    pub override_count: usize,
}

fn read_u16(buf: &[u8], off: usize) -> Result<u16, String> {
    let b: [u8; 2] = buf
        .get(off..off + 2)
        .ok_or_else(|| format!("PE truncated at 0x{off:X}"))?
        .try_into()
        .unwrap();
    Ok(u16::from_le_bytes(b))
}

fn read_u32(buf: &[u8], off: usize) -> Result<u32, String> {
    let b: [u8; 4] = buf
        .get(off..off + 4)
        .ok_or_else(|| format!("PE truncated at 0x{off:X}"))?
        .try_into()
        .unwrap();
    Ok(u32::from_le_bytes(b))
}

fn section_protect(chars: u32) -> DWORD {
    let r = (chars & IMAGE_SCN_MEM_READ) != 0;
    let w = (chars & IMAGE_SCN_MEM_WRITE) != 0;
    let x = (chars & IMAGE_SCN_MEM_EXECUTE) != 0;
    match (r, w, x) {
        (_, true, true) => PAGE_EXECUTE_READWRITE,
        (_, false, true) => PAGE_EXECUTE_READ,
        (_, true, false) => PAGE_READWRITE,
        (true, false, false) => PAGE_READONLY,
        (false, false, false) => PAGE_NOACCESS,
    }
}

/// Map a 32-bit x86 PE executable from `payload_path` at its native `ImageBase` (`0x00400000`).
pub unsafe fn map_frostbite_payload(payload_path: &Path) -> Result<MappedImage, String> {
    let raw = fs::read(payload_path)
        .map_err(|e| format!("failed to read payload {}: {e}", payload_path.display()))?;
    if raw.len() < 0x1000 || raw[0] != b'M' || raw[1] != b'Z' {
        return Err("payload is not a valid MZ executable".into());
    }

    let lfanew = read_u32(&raw, 0x3C)? as usize;
    if read_u32(&raw, lfanew)? != 0x0000_4550 {
        return Err("missing PE\\0\\0 signature".into());
    }

    let machine = read_u16(&raw, lfanew + 4)?;
    if machine != 0x014C {
        return Err(format!("expected x86 PE32 (0x014C), got 0x{machine:04X}"));
    }
    let num_sections = read_u16(&raw, lfanew + 6)? as usize;
    let opt_size = read_u16(&raw, lfanew + 20)? as usize;
    let opt = lfanew + 24;

    let magic = read_u16(&raw, opt)?;
    if magic != 0x010B {
        return Err(format!("expected PE32 optional header (0x010B), got 0x{magic:04X}"));
    }

    let entry_rva = read_u32(&raw, opt + 16)? as usize;
    let image_base = read_u32(&raw, opt + 28)? as usize;
    let size_of_image = read_u32(&raw, opt + 56)? as usize;
    let size_of_headers = read_u32(&raw, opt + 60)? as usize;
    let import_rva = read_u32(&raw, opt + 96 + 8)? as usize;

    // 1. Make Frostbite's exact `0x00400000 .. 0x01D13000` span writable inside our
    //    pre-reserved BSS arena (host PE headers & code sit safely below 0x00400000).
    let base = image_base as *mut u8;
    let mut old_prot: DWORD = 0;
    if unsafe { win32::VirtualProtect(base.cast(), size_of_image, PAGE_READWRITE, &mut old_prot) } == 0 {
        let alloc = unsafe {
            win32::VirtualAlloc(
                image_base as *mut c_void,
                size_of_image,
                MEM_RESERVE | MEM_COMMIT,
                PAGE_READWRITE,
            )
        };
        if alloc as usize != image_base {
            return Err(format!(
                "failed to prepare 0x{image_base:08X}..0x{:08X} (VirtualAlloc returned {:p})",
                image_base + size_of_image,
                alloc
            ));
        }
    }

    // 2. Copy Frostbite's PE headers into `0x00400000` so Frostbite sees its own intact PE header.
    let hdr_len = size_of_headers.min(raw.len()).min(size_of_image);
    unsafe { std::ptr::copy_nonoverlapping(raw.as_ptr(), base, hdr_len) };

    // 3. Copy all sections (VirtualAlloc memory is already zeroed for BSS tails)
    struct SecProt {
        va: usize,
        size: usize,
        prot: DWORD,
    }
    let mut prots = Vec::with_capacity(num_sections);
    let sec_table = opt + opt_size;
    for i in 0..num_sections {
        let s = sec_table + i * 40;
        let vsize = read_u32(&raw, s + 8)? as usize;
        let vaddr = read_u32(&raw, s + 12)? as usize;
        let rsize = read_u32(&raw, s + 16)? as usize;
        let rptr = read_u32(&raw, s + 20)? as usize;
        let chars = read_u32(&raw, s + 36)?;

        if rsize > 0 && rptr + rsize <= raw.len() && vaddr + rsize <= size_of_image {
            unsafe {
                std::ptr::copy_nonoverlapping(raw.as_ptr().add(rptr), base.add(vaddr), rsize);
            }
        }
        let span = vsize.max(rsize);
        if span > 0 {
            prots.push(SecProt {
                va: image_base + vaddr,
                size: span,
                prot: section_protect(chars),
            });
        }
    }

    // 4. Resolve Import Directory with OPFOR's native overrides
    #[repr(C)]
    struct ImportDescriptor {
        orig_first_thunk: u32,
        time_date_stamp: u32,
        forwarder_chain: u32,
        name_rva: u32,
        first_thunk: u32,
    }

    let mut dll_count = 0usize;
    let mut import_count = 0usize;
    let mut override_count = 0usize;

    if import_rva != 0 {
        let mut desc = unsafe { base.add(import_rva) as *const ImportDescriptor };
        unsafe {
            while (*desc).name_rva != 0 {
                let dll_name_ptr = base.add((*desc).name_rva as usize);
                let dll_name = win32::from_ansi_ptr(dll_name_ptr).unwrap_or_default();
                let c_dll = CString::new(dll_name.as_str())
                    .map_err(|_| format!("invalid DLL name '{dll_name}'"))?;
                let hmod = win32::LoadLibraryA(c_dll.as_ptr().cast());
                if hmod.is_null() {
                    return Err(format!("LoadLibraryA('{dll_name}') failed"));
                }
                dll_count += 1;

                let orig_rva = if (*desc).orig_first_thunk != 0 {
                    (*desc).orig_first_thunk
                } else {
                    (*desc).first_thunk
                } as usize;
                let mut orig_thunk = base.add(orig_rva) as *const u32;
                let mut iat_thunk = base.add((*desc).first_thunk as usize) as *mut *const c_void;

                while *orig_thunk != 0 {
                    let val = *orig_thunk;
                    let (real_proc, fname_opt) = if (val & 0x8000_0000) != 0 {
                        let ordinal = (val & 0xFFFF) as usize as *const u8;
                        (win32::GetProcAddress(hmod, ordinal), None)
                    } else {
                        let name_ptr = base.add(val as usize + 2);
                        let fname = win32::from_ansi_ptr(name_ptr).unwrap_or_default();
                        (win32::GetProcAddress(hmod, name_ptr), Some(fname))
                    };
                    if real_proc.is_null() {
                        return Err(format!(
                            "GetProcAddress failed for {dll_name}!{:?}",
                            fname_opt.unwrap_or_else(|| format!("#{}", val & 0xFFFF))
                        ));
                    }

                    let final_proc = if let Some(ref fname) = fname_opt {
                        let p = imports::resolve_override(&dll_name, fname, real_proc);
                        if p != real_proc {
                            override_count += 1;
                        }
                        p
                    } else {
                        real_proc
                    };

                    *iat_thunk = final_proc;
                    import_count += 1;
                    orig_thunk = orig_thunk.add(1);
                    iat_thunk = iat_thunk.add(1);
                }
                desc = desc.add(1);
            }
        }
    }

    // 5. Apply final page protections (code RX, rdata RO, data RW)
    unsafe {
        for sp in &prots {
            win32::VirtualProtect(sp.va as *mut c_void, sp.size, sp.prot, &mut old_prot);
        }
        win32::FlushInstructionCache(win32::GetCurrentProcess(), base.cast(), size_of_image);
    }

    Ok(MappedImage {
        image_base,
        size_of_image,
        entry_point: image_base + entry_rva,
        section_count: num_sections,
        dll_count,
        import_count,
        override_count,
    })
}
