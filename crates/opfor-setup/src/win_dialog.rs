//! Pure-Rust native Win32 File & Folder Picker dialogs (`comdlg32.dll` / `shell32.dll`).
//!
//! Replaces PowerShell `System.Windows.Forms` subprocess invocations with direct Win32 API calls
//! for instant response and zero external script dependencies.
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::ptr;

const OFN_PATHMUSTEXIST: u32 = 0x0000_0800;
const OFN_FILEMUSTEXIST: u32 = 0x0000_1000;
const OFN_NOCHANGEDIR: u32 = 0x0000_0008;

const BIF_RETURNONLYFSDIRS: u32 = 0x0000_0001;
const BIF_NEWDIALOGSTYLE: u32 = 0x0000_0040;
const COINIT_APARTMENTTHREADED: u32 = 0x2;

#[repr(C)]
struct OpenFileNameW {
    l_struct_size: u32,
    hwnd_owner: *mut c_void,
    h_instance: *mut c_void,
    lpstr_filter: *const u16,
    lpstr_custom_filter: *mut u16,
    n_max_cust_filter: u32,
    n_filter_index: u32,
    lpstr_file: *mut u16,
    n_max_file: u32,
    lpstr_file_title: *mut u16,
    n_max_file_title: u32,
    lpstr_initial_dir: *const u16,
    lpstr_title: *const u16,
    flags: u32,
    n_file_offset: u16,
    n_file_extension: u16,
    lpstr_def_ext: *const u16,
    l_cust_data: isize,
    lpfn_hook: *mut c_void,
    lp_template_name: *const u16,
    pv_reserved: *mut c_void,
    dw_reserved: u32,
    flags_ex: u32,
}

#[repr(C)]
struct BrowseInfoW {
    hwnd_owner: *mut c_void,
    pidl_root: *const c_void,
    psz_display_name: *mut u16,
    lpsz_title: *const u16,
    ul_flags: u32,
    lpfn: *const c_void,
    l_param: isize,
    i_image: i32,
}

#[link(name = "comdlg32")]
unsafe extern "system" {
    fn GetOpenFileNameW(ofn: *mut OpenFileNameW) -> i32;
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn SHBrowseForFolderW(bi: *mut BrowseInfoW) -> *mut c_void;
    fn SHGetPathFromIDListW(pidl: *const c_void, psz_path: *mut u16) -> i32;
}

#[link(name = "ole32")]
unsafe extern "system" {
    fn CoInitializeEx(pv_reserved: *mut c_void, dw_co_init: u32) -> i32;
    fn CoTaskMemFree(pv: *mut c_void);
}

fn to_wide_nul(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Opens a native Win32 folder selection dialog (`SHBrowseForFolderW`).
pub fn pick_folder(title: &str) -> Option<String> {
    let title_w = to_wide_nul(title);
    let mut display_buf = [0u16; 260];
    let mut bi = BrowseInfoW {
        hwnd_owner: ptr::null_mut(),
        pidl_root: ptr::null(),
        psz_display_name: display_buf.as_mut_ptr(),
        lpsz_title: title_w.as_ptr(),
        ul_flags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
        lpfn: ptr::null(),
        l_param: 0,
        i_image: 0,
    };

    // SAFETY: Win32 COM initialization and shell folder browser with valid stack-allocated buffers.
    unsafe {
        let _ = CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED);
        let pidl = SHBrowseForFolderW(&mut bi);
        if pidl.is_null() {
            return None;
        }
        let mut path_buf = [0u16; 32_768];
        let ok = SHGetPathFromIDListW(pidl, path_buf.as_mut_ptr()) != 0;
        CoTaskMemFree(pidl);
        if !ok {
            return None;
        }
        let len = path_buf.iter().position(|&c| c == 0).unwrap_or(0);
        if len == 0 {
            None
        } else {
            Some(String::from_utf16_lossy(&path_buf[..len]))
        }
    }
}

/// Opens a native Win32 file selection dialog (`GetOpenFileNameW`).
///
/// `filter` accepts `|`-delimited pairs (e.g. `"Xbox 360 ISO (*.iso)|*.iso|All Files (*.*)|*.*"`).
pub fn pick_file(title: &str, filter: &str) -> Option<String> {
    let title_w = to_wide_nul(title);
    let mut filter_w: Vec<u16> = filter
        .split('|')
        .flat_map(|part| part.encode_utf16().chain(std::iter::once(0)))
        .collect();
    filter_w.push(0);

    let mut file_buf = [0u16; 32_768];
    let mut ofn = OpenFileNameW {
        l_struct_size: std::mem::size_of::<OpenFileNameW>() as u32,
        hwnd_owner: ptr::null_mut(),
        h_instance: ptr::null_mut(),
        lpstr_filter: filter_w.as_ptr(),
        lpstr_custom_filter: ptr::null_mut(),
        n_max_cust_filter: 0,
        n_filter_index: 1,
        lpstr_file: file_buf.as_mut_ptr(),
        n_max_file: file_buf.len() as u32,
        lpstr_file_title: ptr::null_mut(),
        n_max_file_title: 0,
        lpstr_initial_dir: ptr::null(),
        lpstr_title: title_w.as_ptr(),
        flags: OFN_PATHMUSTEXIST | OFN_FILEMUSTEXIST | OFN_NOCHANGEDIR,
        n_file_offset: 0,
        n_file_extension: 0,
        lpstr_def_ext: ptr::null(),
        l_cust_data: 0,
        lpfn_hook: ptr::null_mut(),
        lp_template_name: ptr::null(),
        pv_reserved: ptr::null_mut(),
        dw_reserved: 0,
        flags_ex: 0,
    };

    // SAFETY: `ofn` holds valid pointers to stack/heap buffers for the duration of the modal call.
    let ok = unsafe { GetOpenFileNameW(&mut ofn) != 0 };
    if !ok {
        return None;
    }
    let len = file_buf.iter().position(|&c| c == 0).unwrap_or(0);
    if len == 0 {
        None
    } else {
        Some(String::from_utf16_lossy(&file_buf[..len]))
    }
}
