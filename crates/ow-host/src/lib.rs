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

/// The code packages every Borderlands 2 install holds in `WillowGame/CookedPCConsole`. Only their presence is
/// checked; nothing is read from them here.
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
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstallError::NotADirectory(p) => write!(f, "{} is not a folder", p.display()),
            InstallError::NoCookedFolder(p) => {
                write!(f, "{} has no WillowGame/CookedPCConsole folder", p.display())
            }
            InstallError::MissingPackage(p) => write!(f, "missing {}", p.display()),
        }
    }
}

impl std::error::Error for InstallError {}

/// Checks that `root` looks like a Borderlands 2 install: a folder with `WillowGame/CookedPCConsole` holding the
/// code packages in [`REQUIRED_PACKAGES`].
pub fn find_install(root: &Path) -> Result<Install, InstallError> {
    if !root.is_dir() {
        return Err(InstallError::NotADirectory(root.to_path_buf()));
    }
    let cooked = root.join("WillowGame").join("CookedPCConsole");
    if !cooked.is_dir() {
        return Err(InstallError::NoCookedFolder(root.to_path_buf()));
    }
    for name in REQUIRED_PACKAGES {
        let package = cooked.join(name);
        if !package.is_file() {
            return Err(InstallError::MissingPackage(package));
        }
    }
    Ok(Install { root: root.to_path_buf(), cooked })
}

pub mod frame;

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
}
