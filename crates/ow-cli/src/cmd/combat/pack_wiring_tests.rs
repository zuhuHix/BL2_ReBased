//! Every native pack the slice gate wires, registered together on one VM, with no duplicate key and no silent shadow.
//! #744's landing check: `WillowPawn.InitializeBehaviorProviders()` was bound by both ow-world and the kernel pack,
//! which only an installed run met, because no test registered both. Lane PLACEHOLDER-SHADOWS: a key one pack binds
//! and another gives up (a keep-existing check, an `is_bound` guard, or a swallowed refusal) is caught too, through the
//! VM's shadow audit (`ow_vm::api::audit_new_vms`). AI-assisted (Claude Code).
//!
//! The packages are invented (AGENTS.md rule 9). They declare, as the game's do, the natives the packs bind by their
//! declaration (`kernel::natives::bind` and `ow_combat::hit::bind` bind only declared functions), and the two script
//! functions the inventory pack overrides, so every pack registers here as it does in the installed run.

use std::sync::atomic::{AtomicU32, Ordering};

use ow_core::ParityMode;
use ow_package::testkit::script::{Asm, FunctionSpec, ScriptToy};
use ow_vm::api::{NativePack, audit_new_vms};
use ow_vm::error::VmError;

use super::super::{shared_wrapper_shadow, world};

const NATIVE: u32 = 0x400 | 0x20000;
const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

fn script_function(t: &mut ScriptToy, owner: i32, name: &str) {
    t.function(owner, name, &[], None, &[], &FunctionSpec::new(SCRIPT, name), |_| {
        let mut a = Asm::new();
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
}

fn native_function(t: &mut ScriptToy, owner: i32, name: &str) {
    t.function(owner, name, &[], None, &[], &FunctionSpec::new(NATIVE, name), |_| None);
}

/// `WillowGame`: `WillowPawn` with its native `InitializeBehaviorProviders()`, `WillowPlayerController` with its native
/// `UpdateInputContext()`, and `WillowWeapon` with the script `CalculateWeaponBaseValues()`.
fn willow_game() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function"]);
    let object = t.class("Object", 0);
    let pawn = t.class("WillowPawn", object);
    native_function(&mut t, pawn, "InitializeBehaviorProviders");
    let controller = t.class("WillowPlayerController", object);
    native_function(&mut t, controller, "UpdateInputContext");
    let weapon = t.class("WillowWeapon", object);
    script_function(&mut t, weapon, "CalculateWeaponBaseValues");
    t.build()
}

/// `Engine`: the classes the world's set-up looks up, and `AttributeEffect` with the script `ApplyAttributeEffects()`.
fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function"]);
    let object = t.class("Object", 0);
    t.class("Actor", object);
    let effect = t.class("AttributeEffect", object);
    script_function(&mut t, effect, "ApplyAttributeEffects");
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
    audit_new_vms(true);
    // link 1's world (`--combat-run`): the world's own natives, then its packs, the kernel pack among them.
    let built = world(&dir, ParityMode::Off);
    audit_new_vms(false);
    let mut world = built.expect("the combat-run world registers with no duplicate key");
    // The packs the equip, pickup and cast links add, on the same VM.
    ow_inventory::manager::Pack.register(&mut world.vm).expect("the inventory pack");
    ow_skills::action_skill::Pack.register(&mut world.vm).expect("the action skill pack");
    let shadows: Vec<_> = world.vm.native_shadows().into_iter().filter(|s| !shared_wrapper_shadow(s)).collect();
    assert!(shadows.is_empty(), "keys one pack bound and another gave up: {shadows:#?}");
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
