//! Option B Virtual Filesystem (VFS) path router.
//!
//! Translates `BFBC2Game.exe` file requests targeting:
//! - `engine\Dist\win32\package.mft` -> `cache\package.mft` (if present)
//! - `engine\Dist\win32\bundleManifest` -> `cache\bundleManifest` (if present)
//! - `engine\Package\<rel>` -> first enabled `modules\<era>\<rel>` that exists
//! - `engine\Output\win32\<rel>` -> first enabled `modules\<era>\<rel>` that exists
//!
//! Falls back transparently to the untouched `engine\` tree if no module overrides the path.

use std::path::{Path, PathBuf};

use crate::manifest::ModuleManifest;
use crate::modes::ModeId;

/// Outcome of routing an intercepted Win32 path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VfsResolution {
    /// Leave the original path untouched.
    PassThrough,
    /// Redirect the Win32 call to this resolved physical path.
    Redirect(PathBuf),
    /// Pretend the file does not exist (used to skip startup logo/splash movies).
    Hide,
}

const SKIPPED_INTRO_MOVIES: &[&str] = &[
    "output/win32/movies/ea_logo_hd.res",
    "output/win32/movies/dolbydigital.res",
    "output/win32/movies/esrb_rating_french_english.res",
    "output/win32/movies/attraction.res",
];

/// Pure, thread-safe VFS router loaded once at `dinput8.dll` / `opfor-host` init.
#[derive(Debug, Clone)]
pub struct VfsRouter {
    engine_dir_lower: String,
    cache_dir: PathBuf,
    active_mode_dir: Option<PathBuf>,
    module_dirs: Vec<PathBuf>,
    skip_intro_movies: bool,
    bf1943_native_menu: Option<PathBuf>,
}

impl VfsRouter {
    /// Build a router from the OPFOR root (`battlefield-opfor/`) and discovered modules.
    #[must_use]
    pub fn new(opfor_root: &Path, modules: &[ModuleManifest]) -> Self {
        Self::with_intro_skip(opfor_root, modules, false)
    }

    /// Build a router with explicit control over whether stock BFBC2 intro movies are hidden.
    #[must_use]
    pub fn with_intro_skip(
        opfor_root: &Path,
        modules: &[ModuleManifest],
        skip_intro_movies: bool,
    ) -> Self {
        Self::for_mode(opfor_root, modules, skip_intro_movies, None)
    }

    /// Build an era-aware router that isolates per-game HUD, menu, font, and minimap overlays
    /// (`menu/hud/*`, `menu/main/*`, `menu/shared/*`, `levels/*/ui/*`) strictly to `active_mode`.
    #[must_use]
    pub fn for_era(
        opfor_root: &Path,
        modules: &[ModuleManifest],
        active_mode: ModeId,
        skip_intro_movies: bool,
    ) -> Self {
        let active_slug = match active_mode {
            ModeId::Bfbc2 => "bfbc2",
            ModeId::Vietnam => "vietnam",
            ModeId::Bf1943 => "bf1943",
            ModeId::Bfbc1 => "bfbc1",
        };
        let active_mode_dir = modules
            .iter()
            .find(|m| m.enabled && m.id.eq_ignore_ascii_case(active_slug))
            .map(|m| m.root_dir.clone());
        let mut router = Self::for_mode(opfor_root, modules, skip_intro_movies, None);
        router.active_mode_dir = active_mode_dir;
        router
    }

    /// Build a router with explicit intro-skip and optional mode-specific native `mainmenu-00.fbrb`.
    #[must_use]
    pub fn for_mode(
        opfor_root: &Path,
        modules: &[ModuleManifest],
        skip_intro_movies: bool,
        bf1943_native_menu: Option<PathBuf>,
    ) -> Self {
        let engine_dir = opfor_root.join("engine");
        let cache_dir = opfor_root.join("cache");
        let module_dirs = modules
            .iter()
            .filter(|m| m.enabled)
            .map(|m| m.root_dir.clone())
            .collect();
        Self {
            engine_dir_lower: normalize_win_path(&engine_dir.to_string_lossy()),
            cache_dir,
            active_mode_dir: None,
            module_dirs,
            skip_intro_movies,
            bf1943_native_menu,
        }
    }

    /// Resolve a Win32 path (absolute or relative to `engine\`) using a custom existence probe.
    #[must_use]
    pub fn resolve_with<F>(&self, raw_path: &str, mut exists: F) -> VfsResolution
    where
        F: FnMut(&Path) -> bool,
    {
        let norm = normalize_win_path(raw_path);
        let rel = self.strip_engine_prefix(&norm);

        // 0. Skip stock BFBC2 intro movies only for eras that suppress the BFBC2 title screen (WW2 / BFBC1)
        if self.skip_intro_movies && SKIPPED_INTRO_MOVIES.contains(&rel) {
            if let Some(sub) = rel.strip_prefix("output/win32/") {
                let sub_win = sub.replace('/', "\\");
                for mdir in &self.module_dirs {
                    let cand = mdir.join(&sub_win);
                    if exists(&cand) {
                        return VfsResolution::Redirect(cand);
                    }
                }
            }
            return VfsResolution::Hide;
        }

        // 1. Merged cache manifests (keeps engine/Dist/win32 100% pure stock)
        if rel == "dist/win32/package.mft" {
            let cand = self.cache_dir.join("package.mft");
            if exists(&cand) {
                return VfsResolution::Redirect(cand);
            }
        } else if rel == "dist/win32/bundlemanifest" {
            let cand = self.cache_dir.join("bundleManifest");
            if exists(&cand) {
                return VfsResolution::Redirect(cand);
            }
        } else if rel == "dist/win32/overlay-00.fbrb" {
            let cand = self.cache_dir.join("overlay-00.fbrb");
            if exists(&cand) {
                return VfsResolution::Redirect(cand);
            }
        } else if rel == "package/async/startup-00.fbrb" {
            // IMPORTANT: Only override the Package/ overlay startup bundle (~636 KB).
            // Never override Dist/win32/async/startup-00.fbrb (~1.36 MB base bundle)!
            let cand = self.cache_dir.join("startup-00.fbrb");
            if exists(&cand) {
                return VfsResolution::Redirect(cand);
            }
        }

        // 1b. Era-isolated in-game HUD, minimap, frames, icons, and shared UI overlays
        // (strictly active era only — never leaks BF1943 or BFBC1 HUD into BFBC2 or Vietnam!).
        // Only redirect `Dist/win32/` for full-replacement `Dist`-only bundles (`ingame-00`, `loader-00`).
        // Bundles that exist in BOTH `Dist/win32/` (base) and `Package/` (patch overlay) — such as
        // `menu/shared/fonts_us-00.fbrb`, `menu/main/mainmenu-00.fbrb`, and `streaming_sounds-00.fbrb` —
        // must let `Dist/win32/` pass through to stock base so `Package/` cleanly layers the era delta.
        if let Some(sub) = rel.strip_prefix("dist/win32/")
            && is_era_isolated_dist_rel(sub)
        {
            if let Some(ref active_dir) = self.active_mode_dir {
                let cand = active_dir.join(sub.replace('/', "\\"));
                if exists(&cand) {
                    return VfsResolution::Redirect(cand);
                }
            }
            return VfsResolution::PassThrough;
        }
        if let Some(sub) = rel.strip_prefix("package/") {
            if is_era_isolated_rel(sub) {
                if let Some(ref active_dir) = self.active_mode_dir {
                    let cand = active_dir.join(sub.replace('/', "\\"));
                    if exists(&cand) {
                        return VfsResolution::Redirect(cand);
                    }
                }
                return VfsResolution::PassThrough;
            }
            if rel == "package/menu/main/mainmenu-00.fbrb" {
                if let Some(ref native_menu) = self.bf1943_native_menu
                    && exists(native_menu)
                {
                    return VfsResolution::Redirect(native_menu.clone());
                }
                return VfsResolution::PassThrough;
            }

            // 2. Package/<rel> -> active era first, then other enabled modules
            let sub_win = sub.replace('/', "\\");
            if let Some(ref active_dir) = self.active_mode_dir {
                let cand = active_dir.join(&sub_win);
                if exists(&cand) {
                    return VfsResolution::Redirect(cand);
                }
            }
            for mdir in &self.module_dirs {
                let cand = mdir.join(&sub_win);
                if exists(&cand) {
                    return VfsResolution::Redirect(cand);
                }
            }
        }

        // 3. Output/win32/<rel> -> active era first, then other enabled modules (ai/..., movies/...)
        if let Some(sub) = rel.strip_prefix("output/win32/") {
            let sub_win = sub.replace('/', "\\");
            if let Some(ref active_dir) = self.active_mode_dir {
                let cand = active_dir.join(&sub_win);
                if exists(&cand) {
                    return VfsResolution::Redirect(cand);
                }
            }
            for mdir in &self.module_dirs {
                let cand = mdir.join(&sub_win);
                if exists(&cand) {
                    return VfsResolution::Redirect(cand);
                }
            }
        }

        VfsResolution::PassThrough
    }

    /// Resolve using real filesystem `Path::exists`.
    #[must_use]
    pub fn resolve(&self, raw_path: &str) -> VfsResolution {
        self.resolve_with(raw_path, |p| p.exists())
    }

    fn strip_engine_prefix<'a>(&self, norm: &'a str) -> &'a str {
        let stripped = norm.strip_prefix("./").unwrap_or(norm);
        if let Some(rest) = stripped.strip_prefix(&self.engine_dir_lower) {
            rest.trim_start_matches('/')
        } else {
            stripped
        }
    }
}

/// Returns `true` for `Dist/win32`-only full-replacement bundles that do not have a `Package/`
/// overlay counterpart and must be isolated to the active era.
fn is_era_isolated_dist_rel(sub: &str) -> bool {
    sub == "levels/sp_common/ingame-00.fbrb"
        || sub == "levels/mp_common/ingame-00.fbrb"
        || sub == "levels/sp_common/loader-00.fbrb"
        || sub == "levels/mp_common/loader-00.fbrb"
}

/// Returns `true` for shared HUD/UI/font/ingame bundle paths that must be strictly isolated to the
/// currently active game era (`BFBC2`, `Vietnam`, `BF1943`, or `BFBC1`).
fn is_era_isolated_rel(sub: &str) -> bool {
    is_era_isolated_dist_rel(sub)
        || sub.starts_with("menu/hud/")
        || sub.starts_with("menu/shared/")
        || sub == "streaming_sounds-00.fbrb"
        || sub == "menu/main/mainmenu-00.fbrb"
}

/// Normalize a Windows path for case-insensitive matching (`/` separators, lowercase, no `\\?\`).
#[must_use]
pub fn normalize_win_path(p: &str) -> String {
    let s = p.strip_prefix(r"\\?\").unwrap_or(p);
    let mut out = String::with_capacity(s.len());
    let mut prev_slash = false;
    for ch in s.chars() {
        if ch == '\\' || ch == '/' {
            if !prev_slash {
                out.push('/');
                prev_slash = true;
            }
        } else {
            out.push(ch.to_ascii_lowercase());
            prev_slash = false;
        }
    }
    out.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn dummy_module(id: &str, root: &str) -> ModuleManifest {
        ModuleManifest::parse_str(
            &format!("id = \"{id}\"\nname = \"{id}\"\nenabled = true\n"),
            PathBuf::from(root),
        )
        .unwrap()
    }

    #[test]
    fn routes_package_and_output_and_cache() {
        let root = Path::new(r"C:\Programming\Projects\battlefield-opfor");
        let mods = vec![
            dummy_module("bf1943", r"C:\Programming\Projects\battlefield-opfor\modules\bf1943"),
            dummy_module("vietnam", r"C:\Programming\Projects\battlefield-opfor\modules\vietnam"),
            dummy_module("bfbc1", r"C:\Programming\Projects\battlefield-opfor\modules\bfbc1"),
        ];
        let router = VfsRouter::new(root, &mods);

        let existing: HashSet<PathBuf> = [
            PathBuf::from(r"C:\Programming\Projects\battlefield-opfor\cache\package.mft"),
            PathBuf::from(r"C:\Programming\Projects\battlefield-opfor\modules\vietnam\levels\nam_mp_002cq\level-00.fbrb"),
            PathBuf::from(r"C:\Programming\Projects\battlefield-opfor\modules\bfbc1\ai\levels\mp_beachhead_day1\infantry\pathdata.hpd"),
        ]
        .into_iter()
        .collect();

        // 1. Merged package.mft
        assert_eq!(
            router.resolve_with(r"Dist\win32\package.mft", |p| existing.contains(p)),
            VfsResolution::Redirect(PathBuf::from(
                r"C:\Programming\Projects\battlefield-opfor\cache\package.mft"
            ))
        );

        // 2. Vietnam level routed out of Package/levels into modules/vietnam/levels
        assert_eq!(
            router.resolve_with(
                r"C:\Programming\Projects\battlefield-opfor\engine\Package\levels\nam_mp_002cq\level-00.fbrb",
                |p| existing.contains(p)
            ),
            VfsResolution::Redirect(PathBuf::from(
                r"C:\Programming\Projects\battlefield-opfor\modules\vietnam\levels\nam_mp_002cq\level-00.fbrb"
            ))
        );

        // 3. BFBC1 pathdata.hpd routed out of Output/win32 into modules/bfbc1/ai
        assert_eq!(
            router.resolve_with(
                r"Output\win32\ai\levels\mp_beachhead_day1\infantry\pathdata.hpd",
                |p| existing.contains(p)
            ),
            VfsResolution::Redirect(PathBuf::from(
                r"C:\Programming\Projects\battlefield-opfor\modules\bfbc1\ai\levels\mp_beachhead_day1\infantry\pathdata.hpd"
            ))
        );

        // 4. Stock BFBC2 level passes through untouched
        assert_eq!(
            router.resolve_with(r"Package\levels\mp_001\level-00.fbrb", |p| existing.contains(p)),
            VfsResolution::PassThrough
        );
    }

    #[test]
    fn isolates_era_ingame_apt_menus() {
        let root = Path::new(r"C:\Programming\Projects\battlefield-opfor");
        let mods = vec![
            dummy_module("bf1943", r"C:\Programming\Projects\battlefield-opfor\modules\bf1943"),
            dummy_module("bfbc1", r"C:\Programming\Projects\battlefield-opfor\modules\bfbc1"),
        ];
        let existing: HashSet<PathBuf> = [
            PathBuf::from(r"C:\Programming\Projects\battlefield-opfor\modules\bfbc1\levels\sp_common\ingame-00.fbrb"),
            PathBuf::from(r"C:\Programming\Projects\battlefield-opfor\modules\bfbc1\levels\mp_common\ingame-00.fbrb"),
            PathBuf::from(r"C:\Programming\Projects\battlefield-opfor\modules\bf1943\levels\mp_common\ingame-00.fbrb"),
        ]
        .into_iter()
        .collect();

        let r_bc1 = VfsRouter::for_era(root, &mods, ModeId::Bfbc1, true);
        let r_43 = VfsRouter::for_era(root, &mods, ModeId::Bf1943, true);
        let r_bc2 = VfsRouter::for_era(root, &mods, ModeId::Bfbc2, false);

        assert_eq!(
            r_bc1.resolve_with(r"Dist\win32\levels\sp_common\ingame-00.fbrb", |p| existing.contains(p)),
            VfsResolution::Redirect(PathBuf::from(
                r"C:\Programming\Projects\battlefield-opfor\modules\bfbc1\levels\sp_common\ingame-00.fbrb"
            ))
        );
        assert_eq!(
            r_43.resolve_with(r"Dist\win32\levels\mp_common\ingame-00.fbrb", |p| existing.contains(p)),
            VfsResolution::Redirect(PathBuf::from(
                r"C:\Programming\Projects\battlefield-opfor\modules\bf1943\levels\mp_common\ingame-00.fbrb"
            ))
        );
        // BFBC2 must NEVER see BFBC1 or BF1943 ingame menus!
        assert_eq!(
            r_bc2.resolve_with(r"Dist\win32\levels\sp_common\ingame-00.fbrb", |p| existing.contains(p)),
            VfsResolution::PassThrough
        );
        assert_eq!(
            r_bc2.resolve_with(r"Package\levels\mp_common\ingame-00.fbrb", |p| existing.contains(p)),
            VfsResolution::PassThrough
        );
    }

    #[test]
    fn preserves_base_dist_for_menu_and_font_overlays() {
        let root = Path::new(r"C:\Programming\Projects\battlefield-opfor");
        let mods = vec![
            dummy_module("bf1943", r"C:\Programming\Projects\battlefield-opfor\modules\bf1943"),
            dummy_module("bfbc1", r"C:\Programming\Projects\battlefield-opfor\modules\bfbc1"),
        ];
        let existing: HashSet<PathBuf> = [
            PathBuf::from(r"C:\Programming\Projects\battlefield-opfor\modules\bf1943\menu\main\mainmenu-00.fbrb"),
            PathBuf::from(r"C:\Programming\Projects\battlefield-opfor\modules\bf1943\menu\shared\fonts_us-00.fbrb"),
            PathBuf::from(r"C:\Programming\Projects\battlefield-opfor\modules\bfbc1\menu\main\mainmenu-00.fbrb"),
        ]
        .into_iter()
        .collect();

        let r_43 = VfsRouter::for_era(root, &mods, ModeId::Bf1943, true);
        let r_bc1 = VfsRouter::for_era(root, &mods, ModeId::Bfbc1, true);

        // Dist/win32 base bundles must pass through so base textures & SWFs remain mounted
        assert_eq!(
            r_43.resolve_with(r"Dist\win32\menu\main\mainmenu-00.fbrb", |p| existing.contains(p)),
            VfsResolution::PassThrough
        );
        assert_eq!(
            r_43.resolve_with(r"Dist\win32\menu\shared\fonts_us-00.fbrb", |p| existing.contains(p)),
            VfsResolution::PassThrough
        );
        assert_eq!(
            r_bc1.resolve_with(r"Dist\win32\menu\main\mainmenu-00.fbrb", |p| existing.contains(p)),
            VfsResolution::PassThrough
        );
        // Package overlays still redirect strictly to the active era
        assert_eq!(
            r_43.resolve_with(r"Package\menu\main\mainmenu-00.fbrb", |p| existing.contains(p)),
            VfsResolution::Redirect(PathBuf::from(
                r"C:\Programming\Projects\battlefield-opfor\modules\bf1943\menu\main\mainmenu-00.fbrb"
            ))
        );
        assert_eq!(
            r_43.resolve_with(r"Package\menu\shared\fonts_us-00.fbrb", |p| existing.contains(p)),
            VfsResolution::Redirect(PathBuf::from(
                r"C:\Programming\Projects\battlefield-opfor\modules\bf1943\menu\shared\fonts_us-00.fbrb"
            ))
        );
        assert_eq!(
            r_bc1.resolve_with(r"Package\menu\main\mainmenu-00.fbrb", |p| existing.contains(p)),
            VfsResolution::Redirect(PathBuf::from(
                r"C:\Programming\Projects\battlefield-opfor\modules\bfbc1\menu\main\mainmenu-00.fbrb"
            ))
        );
    }
}
