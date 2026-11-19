//! The read-back census of NATIVE_LOGIN_URL_CONFIG_INPUT.md sections 1.3 and 2.3 (lane IMPL-LINK1-AFTER-LOGIN). The
//! "nothing" answers of `PlayerController.UpdateURL` and `Object.SaveConfig` (`ow_world::world::after_login`) hold only
//! while nothing the port binds reads URL options or saved config back, and no crate reads a config file.
//! AI-assisted (Claude Code). Test only.

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::rc::Rc;

    use ow_attr::AttributeSystem;
    use ow_balance::types::*;
    use ow_core::ParityMode;
    use ow_defs::DefStore;
    use ow_object::heap::ObjectSpace;
    use ow_vm::api::{NativePack, StubPolicy, Vm};
    use ow_world::world::after_login::{SAVE_CONFIG_KEY, UPDATE_URL_KEY};
    use ow_world::world::{BalanceState, World};

    const MUST_BECOME: &str = "the \"nothing\" answers of UpdateURL and SaveConfig must become their NotYet { LayerAbsent } \
                               (NATIVE_LOGIN_URL_CONFIG_INPUT.md sections 1.3 and 2.3)";

    fn cooked() -> std::path::PathBuf {
        std::env::var_os("OW_GAME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2"))
            .join("WillowGame")
            .join("CookedPCConsole")
    }

    /// The keys an Off world over the installed packages binds with `packs` on top of the world's own pack.
    fn keys(packs: &[&dyn NativePack]) -> Vec<String> {
        let defs = DefStore::open(cooked(), ow_defs::identity::IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
        let attrs = AttributeSystem::standard().unwrap();
        let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(attrs), ParityMode::Off).with_stub_policy(StubPolicy::refuse_all());
        ow_vm::natives_core::register(&mut vm).unwrap();
        let snapshot = BalanceSnapshot::new(
            Playthrough::NORMAL,
            OpLevel::new(0).unwrap(),
            PlayerCount::SINGLE,
            ExpLevel::new(1),
            GameStage(1),
            AwesomeLevel::new(0),
            LevelCapUpgrades::ALL,
        )
        .unwrap();
        let world = World::new(vm, BalanceState::new(snapshot), packs).unwrap();
        world.vm.native_keys().map(str::to_string).collect()
    }

    /// (c): the crate sources name no config file and no player config folder. A heuristic over the source text: the
    /// text is folded to lower case and every run of backslashes to one `/`, then searched for the config extension, the
    /// documents folder of the player's settings and the game's config folder, each as a whole word at its end. This
    /// file, which names them, is skipped.
    fn config_readers(dir: &Path, hits: &mut Vec<String>) {
        let needles = [format!("{}{}", ".i", "ni"), format!("{} {}", "my", "games"), format!("{}{}", "willowgame/", "config")];
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                config_readers(&path, hits);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let raw = std::fs::read_to_string(&path).unwrap().to_ascii_lowercase();
                let mut text = String::with_capacity(raw.len());
                for ch in raw.chars() {
                    let ch = if ch == '\\' { '/' } else { ch };
                    if !(ch == '/' && text.ends_with('/')) {
                        text.push(ch);
                    }
                }
                // This file names the needles; it reads nothing.
                if path.file_name().is_some_and(|n| n == "after_login_census.rs") {
                    continue;
                }
                for n in &needles {
                    // A needle counts only as a whole word at its end (`.ini`, not `.init`).
                    let whole = text.match_indices(n.as_str()).any(|(at, _)| {
                        text[at + n.len()..].chars().next().is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'))
                    });
                    if whole {
                        hits.push(format!("{}: {n}", path.display()));
                    }
                }
            }
        }
    }

    /// Sections 1.3 and 2.3, as the hub's steer for #494 frames them. (a): the Off worlds of the slice's drivers, built
    /// over the installed packages with each driver's native packs, bind no reader of URL options (`GetDefaultURL`,
    /// `GetLocalURL`, `GetAddressURL`, `GetURLMap`, `BrowseToURL`), no `GetPerObjectConfigSections` and no config load
    /// or reload: no bound key but `UpdateURL` names a URL, and none but `SaveConfig` names a config. (c) is
    /// [`off_url_and_config_have_no_readers_in_source`], which runs without the game. (b), that both natives write
    /// nothing, is `ow_world`'s `the_url_and_config_natives_answer_nothing` (a full heap comparison).
    ///
    /// Run: `cargo test -p ow-cli off_url_and_config_have_no_readers -- --ignored` (`OW_GAME` names the game folder).
    #[test]
    #[ignore = "needs the installed game"]
    fn off_url_and_config_have_no_readers() {
        let combat = ow_combat::hit::Pack;
        let manager = ow_skills::action_skill::manager::Pack;
        let pawn_data = ow_world::world::pawn_data::Pack;
        let tree_init = ow_skills::tree::init::Pack;
        let inventory = ow_inventory::manager::Pack;
        let tree = ow_skills::tree::Pack;
        let action = ow_skills::action_skill::Pack;
        let behavior = ow_behavior::handlers::Pack;
        let sets: [&[&dyn NativePack]; 4] = [
            &[&combat, &manager, &pawn_data, &tree_init],
            &[&combat, &inventory],
            &[&tree, &action],
            &[&behavior],
        ];
        let mut bad = Vec::new();
        for set in sets {
            for k in keys(set) {
                if reader(&k) {
                    bad.push(k);
                }
            }
        }
        bad.sort();
        bad.dedup();
        assert!(bad.is_empty(), "bound readers {bad:?}: {MUST_BECOME}");
        let combat_keys = keys(&[&combat, &manager, &pawn_data, &tree_init]);
        assert!(combat_keys.iter().any(|k| k == UPDATE_URL_KEY), "the census sees the world's own bindings");
        // Review r1 of #499, item 1: the natives the link-1 driver binds later (the mission bridge's, the mover's)
        // count too, so the keys are also taken from the fully built world after `maya:8`.
        let late = built_link1_keys();
        let late_bad: Vec<String> = late.iter().filter(|k| reader(k)).cloned().collect();
        assert!(late_bad.is_empty(), "bound readers after the link-1 set-up {late_bad:?}: {MUST_BECOME}");
        assert!(late.len() > combat_keys.len(), "the built world binds more than the bare one");
    }

    /// Whether a bound key names a URL or a config reader (anything but the two "nothing" natives).
    fn reader(k: &str) -> bool {
        let lower = k.to_ascii_lowercase();
        (lower.contains("url") && k != UPDATE_URL_KEY) || (lower.contains("config") && k != SAVE_CONFIG_KEY)
    }

    /// The link-1 world as the combat driver builds it (`combat::world`, the mission bridge, `Run::with_bridge`),
    /// after `maya:8` (the world's offline engine object, #501, lets the step reach past the login check).
    fn built_link1_keys() -> Vec<String> {
        use ow_combat::run::{PlayerBridge, Run};
        let mut world = crate::cmd::combat::world(&cooked(), ParityMode::Off).unwrap();
        let bridge: Rc<dyn PlayerBridge> = Rc::new(crate::cmd::combat::MissionBridge::new(&mut world).unwrap());
        let mut run = Run::with_bridge(world, Some(bridge)).unwrap();
        run.apply("maya:8");
        run.world().vm.native_keys().map(str::to_string).collect()
    }

    /// The hub's steer: on link 1's path (the combat driver's `maya:8`), every `UpdateInputContext` call meets a
    /// controller with no `PlayerInput`, so the native writes nothing there. A call with one would raise the flag and
    /// leave it raised (no input frame runs): the accepted divergence `ow_world::world::after_login` names.
    #[test]
    #[ignore = "needs the installed game"]
    fn the_slice_calls_update_input_context_without_a_player_input() {
        use ow_combat::run::{PlayerBridge, Run};
        use ow_world::world::after_login::InputContextCalls;
        let mut world = crate::cmd::combat::world(&cooked(), ParityMode::Off).unwrap();
        world.vm.add_service(InputContextCalls::default()).unwrap();
        let bridge: Rc<dyn PlayerBridge> = Rc::new(crate::cmd::combat::MissionBridge::new(&mut world).unwrap());
        let mut run = Run::with_bridge(world, Some(bridge)).unwrap();
        run.apply("maya:8");
        let calls = run.world().vm.service::<InputContextCalls>().unwrap().borrow().0.clone();
        eprintln!("UpdateInputContext calls: {calls:?}; errors: {:?}", run.errors());
        assert!(!calls.is_empty(), "maya:8 reaches UpdateInputContext");
        assert!(calls.iter().all(|had| !had), "a call with a PlayerInput: {calls:?}");
    }

    /// (c) of the census: no crate names a config file or the player's config folder (so no crate reads one).
    #[test]
    fn off_url_and_config_have_no_readers_in_source() {
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let mut hits = Vec::new();
        config_readers(crates, &mut hits);
        assert!(hits.is_empty(), "config file reads {hits:?}: {MUST_BECOME}");
    }
}
