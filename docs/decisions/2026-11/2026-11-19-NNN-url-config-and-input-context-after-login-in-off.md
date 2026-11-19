## 2026-11-19: UpdateURL, SaveConfig and UpdateInputContext after the login check in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-LINK1-AFTER-LOGIN, implementer. The decision number is pending from the
hub (three digits); NNN is a placeholder.

- **Sources:** `docs/verification/NATIVE_LOGIN_URL_CONFIG_INPUT.md` (#494 at 2923f6f4; analyst lane
  ANALYST-LINK1-AFTER-LOGIN, not this thread) and the hub's steer for its review. No native code or analysis output was
  opened.
- **No real-game session:** every rule is **UNVERIFIED** in the running game.
- **Unchanged rules:** no dependency was added, no bounds check was loosened and no existing `NotYet` guard was
  relaxed.
- **The base:** `fix/rust-login-status` (#479, b6d59ee4). H6's engine object, which gives the empty `OnlineSubsystem`
  slot, comes later; this branch is rebased onto it when it is published.

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
    with no `PlayerInput`. It runs on installed data, with a stand-in for H6's engine object.
  - **Accepted divergence:** a world that sets a `PlayerInput` (links 13 and 14 with one) gets the flag raised, and it
    stays raised, since the port runs no input frame to clear it.
- **No preflight:** none of the three refuses. A `NotYet` they would become is plain at entry; the caller that wrote
  adds the prefix, as `SetupInputDevices` does.
- **`online::UNREAD`** keeps only `WillowPlayerInput.SetupInputDevices`. #479's tests for the three now expect these
  answers.
- **`ow-vm`:** a read-only `Vm::native_keys()` accessor, used only by the census.

**Installed Off `--slice-gate`** (this branch): links 1, 3, 4, 5, 13 and 14 still stop in `maya:8` at
`GameEngine.GetOnlineSubsystem`'s no-engine `NotYet` (#479), before these natives. With the test's stand-in engine
object, `maya:8` gets past all three and next stops at the unbound `WillowGameInfo.IsEditor()`.
