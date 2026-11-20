## 2026-11-19: UpdateURL, SaveConfig and UpdateInputContext after the login check in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-LINK1-AFTER-LOGIN, implementer. The decision number is pending from the
hub (three digits); NNN is a placeholder.

- **Sources:** `docs/verification/NATIVE_LOGIN_URL_CONFIG_INPUT.md` (#494 at 2923f6f4; analyst lane
  ANALYST-LINK1-AFTER-LOGIN, not this thread) and the hub's steer for its review. No native code or analysis output was
  opened.
- **No real-game session:** every rule is **UNVERIFIED** in the running game.
- **Changed guards:** #479's three named `NotYet { NotRead }` placeholders (`UpdateInputContext`, `UpdateURL`,
  `SaveConfig`) are replaced under the note: `UpdateInputContext` by its rule, and the other two by "nothing" with a
  read-back check each. No dependency was added and no bounds check was loosened.
- **The base:** `fix/rust-engine-object-offline` (#501, aca14fa0), H6's engine object with the empty `OnlineSubsystem`
  slot, on #479 (afb32d86). First written on #479 at b6d59ee4.

**What changed** (`crates/ow-world/src/world/after_login.rs`, bound by key from `online::register`, Off only;
Reference keeps the stub policy):
- **`PlayerController.UpdateURL(string,string,bool)`:** nothing, `None` returned. The port never writes the player's
  config, and the option add was not read.
- **`Object.SaveConfig()`:** nothing.
- **Both, under the note's read-back check** (sections 1.3 and 2.3), kept as tests:
  - `off_url_and_config_have_no_readers` (`ow-cli`, installed data, run with `--ignored`): the slice drivers' Off worlds
    bind no key naming a URL or a config except these two. That excludes `GetDefaultURL`, `GetLocalURL`,
    `GetAddressURL`, `GetURLMap`, `BrowseToURL`, `GetPerObjectConfigSections` and any config load or reload. A failure
    says the answers must become the note's `NotYet { LayerAbsent }`;
  - `off_url_and_config_have_no_readers_in_source` (always run): no crate names a config file or the player's config
    folder;
  - `the_url_and_config_natives_answer_nothing` (`ow-world`): both calls leave the whole heap unchanged.
- **`WillowPlayerController.UpdateInputContext()`:**
  - with a `WillowPlayerInput` (or a subclass) as `PlayerInput`: `bUpdateInputContext` true;
  - with none, or another class: nothing.
  - A `WillowPlayerInput` without a bool flag is a data error.
  - The slice test `the_slice_calls_update_input_context_without_a_player_input` checks that link 1's `maya:8` calls it
    with no `PlayerInput`. It runs on installed data, on #501's engine object.
  - **Accepted divergence:** a world that sets a `PlayerInput` (links 13 and 14 with one) gets the flag raised, and it
    stays raised, since the port runs no input frame to clear it.
- **No preflight:** none of the three refuses. A `NotYet` they would become is plain at entry; the caller that wrote
  adds the prefix, as `SetupInputDevices` does.
- **`online::UNREAD`** keeps only `WillowPlayerInput.SetupInputDevices`. #479's tests for the three now expect these
  answers.
- **`ow-vm`:** a read-only `Vm::native_keys()` accessor, used only by the census.

**Installed Off `--slice-gate`** (this branch, on #501): links 1, 3, 4, 5, 13 and 14 get past `GetOnlineSubsystem`,
`UpdateURL`, `SaveConfig` and `UpdateInputContext` (one call, no `PlayerInput`) and stop in `maya:8` at the unbound
native `WillowGameInfo.IsEditor()`, graded an error (link 1: 0 of 1 driver step). 0 ok / 5 NotYet / 9 errors overall.

**Review r1 of #499, folded in.**
1. The census also takes the keys of the fully built link-1 world (the mission bridge, `Run::with_bridge`, after
   `maya:8`, on #501's engine object), so natives the driver binds later count.
2. `SetupInputDevices`' marking is tested again, with a stand-in `NotYet` for `UpdateInputContext`: the exact prefix
   once, kind and record kept, and the clear kept. `online::register` is split into `register_core` and
   `after_login::register` for it, with no change in what it binds.
3. The cast class `WillowPlayerInput` and the flag `bUpdateInputContext` are labelled UNVERIFIED (fitted).
4. The source scan is a heuristic, case-folded with backslash runs collapsed to one `/`.
5. The stale `UNREAD` comment is cut to `SetupInputDevices`.

**Last tidy round (after review r2 of #499):** rebased onto #501 aca14fa0. NATIVE_LOGIN_STATUS.md's two rows for these
natives point here as superseded. `EngineObject`'s doc comment says the slice worlds register it, and the SetupInputDevices
test is renamed `setup_input_devices_clears_then_completes`. `cargo test --workspace` is recorded in GATES.md.
