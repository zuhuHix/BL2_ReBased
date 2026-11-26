//! Every native pack the slice gate wires, registered together on one VM, with no duplicate key (#744's landing check:
//! `WillowPawn.InitializeBehaviorProviders()` was bound by both ow-world and the kernel pack, which only an installed
//! run met, because no test registered both). AI-assisted (Claude Code).
//!
//! The package is invented (AGENTS.md rule 9). It declares, as the game's `WillowGame` does, the native a pack binds by
//! its declaration (`kernel::natives::bind` binds only declared functions), so a second binder is refused here as it
//! was in the installed run.

use std::sync::atomic::{AtomicU32, Ordering};

use ow_core::ParityMode;
use ow_package::testkit::script::{FunctionSpec, ScriptToy};
use ow_vm::api::NativePack;
use ow_vm::error::VmError;

use super::super::world;

const NATIVE: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

/// `WillowGame` with `WillowPawn` (no fields) and its native `InitializeBehaviorProviders()`.
fn willow_game() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function"]);
    let object = t.class("Object", 0);
    let pawn = t.class("WillowPawn", object);
    let spec = FunctionSpec::new(NATIVE, "InitializeBehaviorProviders");
    t.function(pawn, "InitializeBehaviorProviders", &[], None, &[], &spec, |_| None);
    t.build()
}

/// `Engine` with the classes the world's set-up looks up.
fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function"]);
    let object = t.class("Object", 0);
    t.class("Actor", object);
    t.build()
}

#[test]
fn the_slice_gate_s_packs_register_together_with_no_duplicate_key() {
    let dir = std::env::temp_dir().join(format!(
        "ow-cli-pack-wiring-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("WillowGame.upk"), willow_game()).unwrap();
    std::fs::write(dir.join("Engine.upk"), engine()).unwrap();
    // link 1's world (`--combat-run`): the world's own natives, then its packs, the kernel pack among them.
    let mut world = world(&dir, ParityMode::Off).expect("the combat-run world registers with no duplicate key");
    // The action skill pack (the equip link adds it) on the same VM. The inventory pack is not added here: it refuses
    // to register without the game's own declarations (`ApplyAttributeEffects` and the rest), so only an installed run
    // wires it.
    ow_skills::action_skill::Pack.register(&mut world.vm).expect("the action skill pack");
    // The key answers with the kernel pack's rule: its declaration check, not a placeholder's NotRead.
    let class = world.vm.find_class("WillowGame.WillowPawn").unwrap();
    let pawn = world.vm.instantiate(class, "Pawn").unwrap();
    let f = world.vm.find_function("WillowGame.WillowPawn.InitializeBehaviorProviders").unwrap();
    match world.vm.call(f, Some(pawn), vec![]) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("declares no ConsumerHandle"), "{message}"),
        other => panic!("expected the kernel rule's declaration check, got {other:?}"),
    }
    drop(world);
    let _ = std::fs::remove_dir_all(&dir);
}
