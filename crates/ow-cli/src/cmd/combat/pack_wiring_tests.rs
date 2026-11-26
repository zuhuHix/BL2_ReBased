//! Every native pack the slice gate wires, registered together on one VM, with no duplicate key and no silent shadow.
//! #744's landing check: `WillowPawn.InitializeBehaviorProviders()` was bound by both ow-world and the kernel pack,
//! which only an installed run met, because no test registered both. Lane PLACEHOLDER-SHADOWS: a key one pack binds
//! and another gives up (a keep-existing check, an `is_bound` guard, or a swallowed refusal) is caught too, through the
//! VM's shadow audit (`ow_vm::api::audit_new_vms`). AI-assisted (Claude Code).
//!
//! The packages are invented (AGENTS.md rule 9). They declare, as the game's do, the natives the packs bind by their
//! declaration (`kernel::natives::bind` and `ow_combat::hit::bind` bind only declared functions), and the two script
//! functions the inventory pack overrides, so every pack registers here as it does in the installed run. They also
//! declare `ResolveContext`, `AddAttributeModifier` and `Actor.Trace` with their game parameters (lane
//! SHADOW-AUDIT-FOLLOWUPS-2), so the test meets the three allowed shadows itself, and fails if `ow_combat::hit::bind`
//! loses `#[track_caller]`.

use std::sync::atomic::{AtomicU32, Ordering};

use ow_core::ParityMode;
use ow_package::testkit::reflect::toy_struct;
use ow_package::testkit::script::{Asm, CPF_OPT, CPF_OUT, FunctionSpec, Param, ScriptToy};
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

/// A native with these declared parameters (each `(kind, name, extra flags, type reference)`).
fn native_with(t: &mut ScriptToy, owner: i32, name: &str, params: &[(&str, &str, u64, i32)]) {
    let declared: Vec<Param> =
        params.iter().map(|&(kind, n, flags, type_ref)| Param { flags, type_ref, ..Param::new(kind, n) }).collect();
    t.function(owner, name, &declared, None, &[], &FunctionSpec::new(NATIVE, name), |_| None);
}

/// `Engine`: the classes the world's set-up looks up, and `AttributeEffect` with the script `ApplyAttributeEffects()`.
/// Also the three keys two packs bind to one implementation each (`combat::shared_wrapper_shadow`):
/// `AttributeDefinitionBase.ResolveContext(object)`, `AttributeDefinitionBase.AddAttributeModifier(object,object,bool)`
/// and `Actor.Trace` with the game's ten parameters (the `Vector` and `TraceHitInfo` structs invented here).
fn engine() -> Vec<u8> {
    let mut t = ScriptToy::new(&["Object", "Bool", "Int", "Float", "Struct"]);
    let object = t.class("Object", 0);
    let vector = toy_struct(&mut t, object, "Vector", None);
    for f in ["X", "Y", "Z"] {
        t.prop("Float", vector, f, 0, 0);
    }
    let info = toy_struct(&mut t, object, "TraceHitInfo", None);
    t.prop("Float", info, "ToyField", 0, 0);
    let actor = t.class("Actor", object);
    native_with(&mut t, actor, "Trace", &[
        ("Struct", "HitLocation", CPF_OUT, vector),
        ("Struct", "HitNormal", CPF_OUT, vector),
        ("Struct", "TraceEnd", 0, vector),
        ("Struct", "TraceStart", CPF_OPT, vector),
        ("Bool", "bTraceActors", CPF_OPT, 0),
        ("Struct", "Extent", CPF_OPT, vector),
        ("Struct", "HitInfo", CPF_OPT | CPF_OUT, info),
        ("Int", "ExtraTraceFlags", CPF_OPT, 0),
        ("Bool", "bTraceBulletListeners", CPF_OPT, 0),
        ("Object", "BulletListenerSource", CPF_OPT, object),
    ]);
    // Lane FIX-IS-PLAYER-PAWN-AND-UNBOUND-KEYS: the arms' attach, which the combat world now binds itself.
    native_with(&mut t, actor, "AttachComponent", &[("Object", "NewComponent", 0, object)]);
    let effect = t.class("AttributeEffect", object);
    script_function(&mut t, effect, "ApplyAttributeEffects");
    let attribute = t.class("AttributeDefinitionBase", object);
    native_with(&mut t, attribute, "ResolveContext", &[("Object", "ContextSource", 0, object)]);
    native_with(&mut t, attribute, "AddAttributeModifier", &[
        ("Object", "Context", 0, object),
        ("Object", "Modifier", 0, object),
        ("Bool", "bAllowDuplicate", 0, 0),
    ]);
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
    let all = world.vm.native_shadows();
    let shadows: Vec<_> = all.iter().filter(|s| !shared_wrapper_shadow(s)).collect();
    assert!(shadows.is_empty(), "keys one pack bound and another gave up: {shadows:#?}");
    // Both allowance arms are met here, each key once: the helper's #[track_caller] names the action skill file.
    let mut allowed: Vec<&str> = all.iter().filter(|s| shared_wrapper_shadow(s)).map(|s| s.key.as_str()).collect();
    allowed.sort_unstable();
    let mut want = vec![
        ow_inventory::equip::natives::ADD_MODIFIER,
        super::super::ACTOR_TRACE_KEY,
        ow_inventory::equip::natives::RESOLVE_CONTEXT,
    ];
    want.sort_unstable();
    assert_eq!(allowed, want, "the three allowed shadows, met by this test's packs");
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

/// Lane FIX-IS-PLAYER-PAWN-AND-UNBOUND-KEYS: link 1's combat world binds `Actor.AttachComponent` (ow-inventory's
/// rule, through `bind_engine_attach`), `Pawn.IsPlayerPawn()` and the two `MaterialInstanceConstant` keys in Off, and
/// none of them in Reference. The inventory pack registered after it reuses the binding (no shadow: the test above).
#[test]
fn the_combat_world_binds_the_three_once_unbound_keys_in_off_only() {
    use ow_inventory::equip::material_instances::{MIC_SET_PARENT, MIC_SET_VECTOR_PARAMETER_VALUE};
    use ow_world::world::render_hooks::IS_PLAYER_PAWN;
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let dir = std::env::temp_dir().join(format!(
            "ow-cli-pack-wiring-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("WillowGame.upk"), willow_game()).unwrap();
        std::fs::write(dir.join("Engine.upk"), engine()).unwrap();
        let mut world = world(&dir, mode).expect("the combat-run world");
        let off = mode == ParityMode::Off;
        let attach = world.vm.find_function("Engine.Actor.AttachComponent").unwrap();
        assert_eq!(world.vm.is_bound(attach), off, "Actor.AttachComponent in {mode:?}");
        for key in [IS_PLAYER_PAWN, MIC_SET_PARENT, MIC_SET_VECTOR_PARAMETER_VALUE] {
            assert_eq!(world.vm.has_native(key), off, "{key} in {mode:?}");
        }
        drop(world);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
