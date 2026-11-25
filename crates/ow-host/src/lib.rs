//! The Rust host, on the engine route the maintainer picked from the Phase 0R benchmark: our own engine on wgpu +
//! winit (`docs/verification/ENGINE_BENCHMARK.md`, decision 2026-11-15-01). Lane H1 added the two engine crates
//! (`THIRD_PARTY.md`, "Engine crates: wgpu and winit") and the skeleton: a window, a wgpu device and surface, and a
//! frame loop that clears and presents ([`app`], [`gpu`]), stepping a stand-in world through the host boundary
//! ([`sim`]). It draws nothing from the game yet; scene loading is H2.
//!
//! The one rule every route keeps (AGENTS.md rule 11) stays first: the host reads the player's own installed game and
//! refuses to start without it. [`find_install`] checks that the install's folders and code packages exist; it reads
//! nothing from them. Only `--headless-probe`, a GPU diagnostic that opens no window and runs nothing, skips it.

pub mod app;
pub mod args;
pub mod executor;
pub mod gpu;
pub mod sim;
pub mod stats;

use std::fmt;
use std::path::{Path, PathBuf};
pub mod draw;
pub mod render;
pub mod camera;
pub mod material;
pub mod lighting;
pub mod lightmap;

/// The code packages every Borderlands 2 install holds in its cooked folder. Only their presence is checked; nothing
/// is read from them here.
pub const REQUIRED_PACKAGES: [&str; 3] = ["Core.upk", "Engine.upk", "WillowGame.upk"];

/// The folders of an install that the host reads from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Install {
    pub root: PathBuf,
    pub cooked: PathBuf,
}

/// Why a folder is not a usable install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    NotADirectory(PathBuf),
    NoCookedFolder(PathBuf),
    MissingPackage(PathBuf),
    /// Two entries whose names differ only in case stand where one install folder or package is looked for
    /// (`ow_core::install`): refused, not guessed.
    AmbiguousName(String),
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstallError::NotADirectory(p) => write!(f, "{} is not a folder", p.display()),
            InstallError::NoCookedFolder(p) => {
                write!(f, "{} has no WillowGame/CookedPCConsole (or steamassets/WillowGame/CookedPCConsole) folder", p.display())
            }
            InstallError::MissingPackage(p) => write!(f, "missing {}", p.display()),
            InstallError::AmbiguousName(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for InstallError {}

/// Checks that `root` looks like a Borderlands 2 install: a folder whose cooked folder holds the code packages in
/// [`REQUIRED_PACKAGES`]. The cooked folder is `WillowGame/CookedPCConsole` (the Windows build) or
/// `steamassets/WillowGame/CookedPCConsole` (Aspyr's Linux build, every name in lower case), each name matched
/// exactly first and then ignoring case (`ow_core::install::cooked_folder`). Both are the player's own installed
/// game; without either the host refuses to start (AGENTS.md rule 11).
pub fn find_install(root: &Path) -> Result<Install, InstallError> {
    if !root.is_dir() {
        return Err(InstallError::NotADirectory(root.to_path_buf()));
    }
    let ambiguous = |e: ow_core::install::PathCaseError| InstallError::AmbiguousName(e.to_string());
    let Some(cooked) = ow_core::install::cooked_folder(root).map_err(ambiguous)? else {
        return Err(InstallError::NoCookedFolder(root.to_path_buf()));
    };
    for name in REQUIRED_PACKAGES {
        let package = ow_core::install::resolve_or_join(&cooked, name).map_err(ambiguous)?;
        if !package.is_file() {
            return Err(InstallError::MissingPackage(package));
        }
    }
    Ok(Install { root: root.to_path_buf(), cooked })
}

pub mod anim;
pub mod frame;
pub mod movement;
pub mod nav;
pub mod overlay;
pub mod play;
pub mod weapon;
pub mod world;

/// The scene loader moved to `ow_assets::scene` (lane LINK-2-HEADLESS-MOVER, decision 2026-11-19-66), so the
/// headless mover in `ow-cli` loads maps the same way; re-exported so `crate::scene` paths are unchanged.
pub use ow_assets::scene;

#[cfg(test)]
mod scene_install_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // Synthetic folders only (AGENTS.md rule 9): empty files stand in for the packages.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ow-host-test-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn refuses_a_missing_folder() {
        let dir = scratch("missing").join("nope");
        assert_eq!(find_install(&dir), Err(InstallError::NotADirectory(dir.clone())));
    }

    #[test]
    fn refuses_a_folder_without_cooked_packages() {
        let dir = scratch("nocooked");
        assert_eq!(find_install(&dir), Err(InstallError::NoCookedFolder(dir.clone())));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refuses_an_install_missing_a_code_package() {
        let dir = scratch("partial");
        let cooked = dir.join("WillowGame").join("CookedPCConsole");
        fs::create_dir_all(&cooked).unwrap();
        fs::write(cooked.join("Core.upk"), b"").unwrap();
        fs::write(cooked.join("Engine.upk"), b"").unwrap();
        assert_eq!(
            find_install(&dir),
            Err(InstallError::MissingPackage(cooked.join("WillowGame.upk")))
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn accepts_a_complete_install() {
        let dir = scratch("complete");
        let cooked = dir.join("WillowGame").join("CookedPCConsole");
        fs::create_dir_all(&cooked).unwrap();
        for name in REQUIRED_PACKAGES {
            fs::write(cooked.join(name), b"").unwrap();
        }
        assert_eq!(find_install(&dir), Ok(Install { root: dir.clone(), cooked }));
        fs::remove_dir_all(&dir).unwrap();
    }

    /// Lane LINUX-PATHS: Aspyr's layout, every name in lower case, is an install too; its packages are the files
    /// found, in their own case.
    #[test]
    fn accepts_aspyr_s_lower_case_layout() {
        let dir = scratch("aspyr");
        let cooked = dir.join("steamassets").join("willowgame").join("cookedpcconsole");
        fs::create_dir_all(&cooked).unwrap();
        for name in REQUIRED_PACKAGES {
            fs::write(cooked.join(name.to_ascii_lowercase()), b"").unwrap();
        }
        let found = find_install(&dir).unwrap();
        // Ignoring case: a case-insensitive volume (Windows CI) answers with the requested spelling (review r2 of #675).
        assert!(ow_core::install::ends_with(&found.cooked, "steamassets/willowgame/cookedpcconsole"), "{:?}", found.cooked);
        assert!(found.cooked.is_dir());
        assert_eq!(found.root, dir);
        fs::remove_file(cooked.join("core.upk")).unwrap();
        assert!(matches!(find_install(&dir), Err(InstallError::MissingPackage(_))), "a missing package still refuses");
        fs::remove_dir_all(&dir).unwrap();
    }
}
