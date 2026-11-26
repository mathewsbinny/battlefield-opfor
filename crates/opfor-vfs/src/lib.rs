//! Shared Option B Virtual Filesystem (VFS) router, 4-mode engine table,
//! module manifest discovery, and `package.mft` cache merger.
//!
//! Designed with zero external dependencies so it compiles identically for
//! both 32-bit (`i686-pc-windows-msvc` `dinput8.dll`) and 64-bit (`OPFOR.exe`
//! launcher and `opfor-cli`).

pub mod manifest;
pub mod modes;
pub mod router;

pub use manifest::{ModuleManifest, discover_modules, merge_package_mft};
pub use modes::{ModeId, ModeSpec, ModeTable};
pub use router::{VfsResolution, VfsRouter};
