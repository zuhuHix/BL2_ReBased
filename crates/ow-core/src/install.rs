//! Owner: lane LINUX-PATHS. Finding files of the player's installed game whatever the case of their names.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-25-10-install-layout-and-path-case.md`.
//!
//! The Windows build of Borderlands 2 names its folders and packages in mixed case (`WillowGame/CookedPCConsole/
//! Startup.upk`), and the code names them that way. Aspyr's native Linux build has the same packages, every path in
//! lower case, under one more folder (`steamassets/willowgame/cookedpcconsole/startup.upk`). Windows file systems
//! ignore case, Linux ones do not, so a mixed-case join finds nothing there.
//!
//! **The rule** ([`resolve`]): a relative path is followed one component at a time. A component whose exact name
//! exists is taken (so a Windows install, and any exact match, behaves as before); otherwise the folder is listed and
//! the one entry whose name equals it ignoring ASCII case is taken. None is "not found"; two or more is an error
//! ([`PathCaseError::Ambiguous`]), never a guess. A folder that cannot be read for any reason but its absence is
//! [`PathCaseError::Io`], not "not found" (review r1 of #675). [`ends_with`] is the same comparison for a path the census already
//! holds: each trailing component equal exactly or ignoring ASCII case.
//!
//! **The install layouts** ([`cooked_folder`]): `<root>/WillowGame/CookedPCConsole` (Windows), then
//! `<root>/steamassets/WillowGame/CookedPCConsole` (Aspyr's Linux build), each by [`resolve`]. Both are the player's own
//! installed game (AGENTS.md rule 11); when neither exists there is no install.

use std::fmt;
use std::path::{Path, PathBuf};

/// The cooked folder of each install layout, relative to the install root, in the order [`cooked_folder`] tries them.
pub const COOKED_LAYOUTS: [&str; 2] = ["WillowGame/CookedPCConsole", "steamassets/WillowGame/CookedPCConsole"];

/// Why a path could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathCaseError {
    /// Several entries of `dir` equal `name` ignoring case (and none equals it exactly).
    Ambiguous { dir: PathBuf, name: String, matches: Vec<String> },
    /// `path` could not be checked or listed for a reason other than its absence.
    Io { path: PathBuf, message: String },
}

impl fmt::Display for PathCaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathCaseError::Ambiguous { dir, name, matches } => write!(
                f,
                "{} holds {} entries named {name} ignoring case ({}); refusing to pick one",
                dir.display(),
                matches.len(),
                matches.join(", ")
            ),
            PathCaseError::Io { path, message } => write!(f, "{}: {message}", path.display()),
        }
    }
}

impl std::error::Error for PathCaseError {}

/// The entry of `dir` named `name` (module documentation): the exact name if it exists, else the one entry equal to
/// it ignoring ASCII case; `Ok(None)` when there is none or `dir` does not exist. Any other failure to check or list
/// is [`PathCaseError::Io`].
pub fn resolve_child(dir: &Path, name: &str) -> Result<Option<PathBuf>, PathCaseError> {
    let io = |path: &Path, e: std::io::Error| PathCaseError::Io { path: path.to_path_buf(), message: e.to_string() };
    let exact = dir.join(name);
    if exact.try_exists().map_err(|e| io(&exact, e))? {
        return Ok(Some(exact));
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io(dir, e)),
    };
    let mut matches: Vec<String> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| io(dir, e))?;
        if let Some(n) = entry.file_name().to_str()
            && n.eq_ignore_ascii_case(name)
        {
            matches.push(n.to_string());
        }
    }
    matches.sort();
    match matches.len() {
        0 => Ok(None),
        1 => Ok(Some(dir.join(&matches[0]))),
        _ => Err(PathCaseError::Ambiguous { dir: dir.to_path_buf(), name: name.to_string(), matches }),
    }
}

/// `rel` (components separated by `/`) under `base`, each component by [`resolve_child`]; `Ok(None)` when one is
/// missing.
pub fn resolve(base: &Path, rel: &str) -> Result<Option<PathBuf>, PathCaseError> {
    let mut at = base.to_path_buf();
    for part in rel.split('/').filter(|p| !p.is_empty()) {
        match resolve_child(&at, part)? {
            Some(next) => at = next,
            None => return Ok(None),
        }
    }
    Ok(Some(at))
}

/// [`resolve`], or `base` joined with `rel` as written when it finds nothing, so a caller's own "cannot open" error
/// names the path it looked for.
pub fn resolve_or_join(base: &Path, rel: &str) -> Result<PathBuf, PathCaseError> {
    Ok(resolve(base, rel)?.unwrap_or_else(|| rel.split('/').fold(base.to_path_buf(), |p, c| p.join(c))))
}

/// The install's cooked package folder (module documentation): the first of [`COOKED_LAYOUTS`] that resolves to a
/// folder under `root`, or `Ok(None)`.
pub fn cooked_folder(root: &Path) -> Result<Option<PathBuf>, PathCaseError> {
    for layout in COOKED_LAYOUTS {
        if let Some(p) = resolve(root, layout)?
            && p.is_dir()
        {
            return Ok(Some(p));
        }
    }
    Ok(None)
}

/// The folder that holds `WillowGame` in the install at `root`: the cooked folder's grandparent when
/// [`cooked_folder`] finds one (`root` itself for the Windows layout, `root/steamassets` for Aspyr's), else `root`.
/// Paths such as `WillowGame/Localization/INT/<package>.int` are resolved from it.
pub fn content_root(root: &Path) -> Result<PathBuf, PathCaseError> {
    Ok(cooked_folder(root)?
        .and_then(|c| c.parent().and_then(Path::parent).map(Path::to_path_buf))
        .unwrap_or_else(|| root.to_path_buf()))
}

/// The install's cooked folder for a caller that reports a missing one itself: [`cooked_folder`], or
/// `root/WillowGame/CookedPCConsole` as written when there is none.
pub fn cooked_or_join(root: &Path) -> Result<PathBuf, PathCaseError> {
    Ok(cooked_folder(root)?.unwrap_or_else(|| root.join("WillowGame").join("CookedPCConsole")))
}

/// The install's localization roots for the cooked folder `cooked`, in priority order: `Engine`, `Gearbox` and
/// `WillowGame`'s `Localization` folders under the folder two above `cooked`, each by [`resolve_or_join`] (review r1
/// of #675).
pub fn localization_roots(cooked: &Path) -> Result<Vec<PathBuf>, PathCaseError> {
    let game = cooked.parent().and_then(Path::parent).ok_or_else(|| PathCaseError::Io {
        path: cooked.to_path_buf(),
        message: "the cooked folder has no install parent".to_string(),
    })?;
    ["Engine", "Gearbox", "WillowGame"].iter().map(|n| resolve_or_join(game, &format!("{n}/Localization"))).collect()
}

/// Whether `path` ends with `rel` (components separated by `/`), each component equal exactly or ignoring ASCII
/// case (module documentation).
pub fn ends_with(path: &Path, rel: &str) -> bool {
    let want: Vec<&str> = rel.split('/').filter(|p| !p.is_empty()).collect();
    let have: Vec<String> = path.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    if want.is_empty() || have.len() < want.len() {
        return false;
    }
    have[have.len() - want.len()..].iter().zip(&want).all(|(h, w)| h == w || h.eq_ignore_ascii_case(w))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A temporary folder with the given relative files (created empty), removed on drop.
    struct Tree(PathBuf);

    impl Tree {
        fn new(tag: &str, files: &[&str]) -> Tree {
            let dir = std::env::temp_dir().join(format!("ow-core-install-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            for f in files {
                let p = dir.join(f);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(&p, b"").unwrap();
            }
            std::fs::create_dir_all(&dir).unwrap();
            Tree(dir)
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Whether this file system tells `a` and `A` apart (Linux does; Windows and a default macOS volume do not).
    fn case_sensitive(dir: &Path) -> bool {
        let probe = dir.join("case_probe");
        std::fs::write(&probe, b"").unwrap();
        let sensitive = !dir.join("CASE_PROBE").exists();
        std::fs::remove_file(&probe).unwrap();
        sensitive
    }

    #[test]
    fn a_lower_case_tree_resolves_from_the_windows_names() {
        let t = Tree::new("lower", &["steamassets/willowgame/cookedpcconsole/startup.upk"]);
        let got = resolve(&t.0, "steamassets/WillowGame/CookedPCConsole/Startup.upk").unwrap().unwrap();
        assert!(got.ends_with("steamassets/willowgame/cookedpcconsole/startup.upk"), "{got:?}");
        assert_eq!(resolve(&t.0, "steamassets/WillowGame/NoSuch.upk").unwrap(), None);
        let cooked = cooked_folder(&t.0).unwrap().unwrap();
        assert!(cooked.ends_with("steamassets/willowgame/cookedpcconsole"), "{cooked:?}");
        assert_eq!(content_root(&t.0).unwrap(), t.0.join("steamassets"));
        assert_eq!(cooked_or_join(&t.0).unwrap(), cooked);
    }

    #[test]
    fn an_exact_name_is_taken_first_and_the_windows_layout_before_aspyr_s() {
        let t = Tree::new("exact", &["WillowGame/CookedPCConsole/Core.upk", "steamassets/willowgame/cookedpcconsole/core.upk"]);
        let cooked = cooked_folder(&t.0).unwrap().unwrap();
        assert!(cooked.ends_with("WillowGame/CookedPCConsole"), "{cooked:?}");
        let core = resolve(&cooked, "Core.upk").unwrap().unwrap();
        assert!(core.ends_with("CookedPCConsole/Core.upk"), "{core:?}");
    }

    #[test]
    fn two_names_equal_ignoring_case_are_an_error_not_a_guess() {
        let t = Tree::new("ambiguous", &[]);
        if !case_sensitive(&t.0) {
            return; // two such names cannot exist side by side here
        }
        std::fs::write(t.0.join("startup.upk"), b"").unwrap();
        std::fs::write(t.0.join("STARTUP.upk"), b"").unwrap();
        match resolve(&t.0, "Startup.upk") {
            Err(PathCaseError::Ambiguous { matches, .. }) => assert_eq!(matches, ["STARTUP.upk", "startup.upk"]),
            other => panic!("{other:?}"),
        }
        // An exact name among them is no ambiguity.
        assert!(resolve(&t.0, "startup.upk").unwrap().unwrap().ends_with("startup.upk"));
    }

    #[test]
    fn the_localization_roots_resolve_in_aspyr_s_layout() {
        let t = Tree::new("loc", &["steamassets/engine/localization/int/core.int", "steamassets/willowgame/cookedpcconsole/core.upk"]);
        let cooked = cooked_folder(&t.0).unwrap().unwrap();
        let roots = localization_roots(&cooked).unwrap();
        assert!(roots[0].ends_with("steamassets/engine/localization"), "{roots:?}");
        // A root that is not there is joined as written; its reader finds no file there.
        assert!(roots[1].ends_with("steamassets/Gearbox/Localization"), "{roots:?}");
        assert_eq!(resolve_child(&roots[0], "INT").unwrap(), Some(roots[0].join("int")));
    }

    #[test]
    fn no_install_layout_is_no_install() {
        let t = Tree::new("none", &["Binaries/x.txt"]);
        assert_eq!(cooked_folder(&t.0).unwrap(), None);
        assert_eq!(content_root(&t.0).unwrap(), t.0);
        assert_eq!(cooked_or_join(&t.0).unwrap(), t.0.join("WillowGame").join("CookedPCConsole"));
        assert_eq!(resolve_or_join(&t.0, "WillowGame/X.upk").unwrap(), t.0.join("WillowGame").join("X.upk"));
    }

    #[test]
    fn ends_with_compares_trailing_components_ignoring_case() {
        let p = Path::new("/g/steamassets/willowgame/cookedpcconsole/startup.upk");
        assert!(ends_with(p, "WillowGame/CookedPCConsole/Startup.upk"));
        assert!(!ends_with(p, "WillowGame/CookedPCConsole/Other.upk"));
        assert!(!ends_with(Path::new("startup.upk"), "CookedPCConsole/Startup.upk"));
        assert!(ends_with(Path::new("/a/WillowGame/CookedPCConsole/Startup.upk"), "WillowGame/CookedPCConsole/Startup.upk"));
    }
}
