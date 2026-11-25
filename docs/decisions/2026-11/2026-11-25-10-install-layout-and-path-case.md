## 2026-11-25: The install layout and file-name case: Aspyr's Linux build is an install, and names resolve ignoring case

AI-assisted (Claude Code)

Lane LINUX-PATHS, with Claude Opus 5.5; the hub approved cause B of the Linux install probe. No change to
`crates/ow-package` or `crates/ow-container`, no new dependency, and no bounds check changed.

- **Why:** the desktop now runs Linux, with Aspyr's native Linux build of Borderlands 2. Its packages carry the
  Windows build's 832/46 header, but every path is lower case, under one more folder:
  `steamassets/willowgame/cookedpcconsole/*.upk`. The code joins Windows-cased names
  (`Sanctuary_Dynamic.upk`, `WillowGame/CookedPCConsole/Startup.upk`), which a case-sensitive file system does not
  find. On main faa3bbc the installed Off `--slice-gate` gave 1 ok / 3 NotYet / 10 error there. Links 10 to 12
  stopped at "cannot open input package" for `Sanctuary_Dynamic.upk`, and `ow-host --game` refused the install.
- **The rule** (`ow_core::install`, a new module in the one crate every caller already depends on, so no new edge):
  - a relative path is followed one component at a time;
  - a component whose exact name exists is taken first, so a Windows install behaves as before;
  - otherwise the folder is listed and the one entry equal to the name ignoring ASCII case is taken;
  - none is "not found", and two or more is an error (`PathCaseError::Ambiguous`), never a guess;
  - for paths the census already holds, `install::ends_with` compares trailing components exactly or ignoring ASCII
    case.
- **The install layouts:** `<root>/WillowGame/CookedPCConsole` (Windows), then
  `<root>/steamassets/WillowGame/CookedPCConsole` (Aspyr), each by that rule (`install::cooked_folder`).
  `install::content_root` is the folder that holds `WillowGame` (for `WillowGame/Localization/...`). Both layouts are
  the player's own installed game (rule 11); `ow-host` still refuses a folder with neither, or with a missing code
  package, and also refuses an ambiguous name.
- **Where it applies:**
  - `ow-host`'s `find_install`;
  - the slice gate's Fire source;
  - the `Startup.upk` suffix filters of `--item-roll` / `--item-stats` and `PoolData::from_files`;
  - the vendor map view's level and globals files;
  - `installed_default_game`'s `DefaultGame.ini`;
  - the item cards' and the Phaselock run's localization files;
  - the installed-game tests that built `WillowGame/CookedPCConsole` by hand (`skeletal_source`,
    `after_login_census`, ow-ai's two);
  - (review r1 of #675) the `Object.Localize` roots of `slice::open` and `ow-host`'s `build_installed`
    (`install::localization_roots`: `Engine`, `Gearbox` and `WillowGame`'s `Localization`) and their `INT` folder
    (`localize.rs`, by `resolve_child`; two that differ only in case are an error);
  - (review r1 of #675) `resolve_map` (`ow-assets` `movement/sanctuary.rs`), used by `ow-host --map` and `--play`;
    an ambiguous name is now an error there;
  - (found while measuring review r1 of #675) the texture file caches: `TfcStore::read` (`ow-assets` `tfc.rs`)
    resolves `<cache name>.tfc` the same way, after its unchanged name checks; an ambiguous name is a failed open.
- **Errors, not guesses** (review r1 of #675): a folder that cannot be checked or listed for any reason but its
  absence is `PathCaseError::Io`. The vendor map view refuses two census files that both match its level ignoring
  case, instead of taking the first.
  Adding `pub mod install;` to `ow-core/src/lib.rs` is that file's first edit since the scaffold.
- **Measured** (zuhuPC, Linux, release build under the machine lock, main faa3bbc plus this lane): `ow-host --game
  <Aspyr install>` exit 0 (install found); `--game /tmp` exit 1. Installed Off strict `--slice-gate` on the Aspyr
  install: 1 ok / 3 NotYet / 10 error, the same counts as before. Links 10 to 12 now open their package and stop where
  links 1, 3 to 5, 13 and 14 do. All 10 errors now come from the other cause, which this lane leaves alone: a package
  is named after its file stem (`Package::load`), so Aspyr's class paths read `willowgame.MissionDefinition` and
  `gearboxframework.BehaviorProviderDefinition`, and exact compares fail. That change is in `crates/ow-package`, a
  sensitive file, and waits for the maintainer.
- **Not checked:** a Windows run of this branch (the exact-name path is the old join, and Windows CI runs the
  tests); macOS. Nothing here is a claim about the game's behaviour.
