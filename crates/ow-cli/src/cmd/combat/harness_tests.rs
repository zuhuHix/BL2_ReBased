//! The `maya` step's two Off harness inputs (decisions 2026-11-19-41 and 2026-11-19-46; review r1 of #431), on an
//! invented `WillowGame` package (AGENTS.md rule 9): the preload runs first and the class select second, each logged
//! with its outcome; Reference does neither.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_vm::api::{NativeCall, Vm};

use super::super::apply_class_inputs;

const NATIVE: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function", "ObjectProperty", "StrProperty"]);
    let object = t.class("Object", 0);
    let controller = t.class("WillowPlayerController", object);
    t.prop("Object", controller, "PlayerClass", 0, object);
    let spec = FunctionSpec::new(NATIVE, "ServerPlayerSelectClass");
    t.function(controller, "ServerPlayerSelectClass", &[Param::new("Str", "ClassName")], None, &[], &spec, |_| None);
    let definition = t.class("PlayerClassDefinition", object);
    t.prop("Str", definition, "PawnArchetypePath", 0, 0);
    let thing = t.class("Thing", object);
    t.prop("Object", thing, "Ref", 0, object);
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend_from_slice(&t.none);
    t.p.add_export(thing, "ToyPawnArchetype", payload);
    // Review r1 of #528: `ToyCycle`, whose export archetype is itself (the Off store's load refuses it, `NotYet {
    // NotRead }`), and `ToyLinkedArchetype`, whose same-package `Ref` names it.
    let cycle_ref = t.p.next_export();
    let mut cycle_payload = 0i32.to_le_bytes().to_vec();
    cycle_payload.extend_from_slice(&t.none);
    let mut cycle = ow_package::testkit::Export::new(thing, "ToyCycle", cycle_payload);
    cycle.archetype = cycle_ref;
    t.p.add_export_with(cycle);
    let mut linked = 0i32.to_le_bytes().to_vec();
    linked.extend(t.p.tag("Ref", "ObjectProperty", &cycle_ref.to_le_bytes()));
    linked.extend_from_slice(&t.none);
    t.p.add_export(thing, "ToyLinkedArchetype", linked);
    t.p.build()
}

struct Fx {
    vm: Vm,
    controller: ObjRef,
    definition: ObjRef,
    /// What the class select saw: the log at that moment, and whether the archetype was in memory.
    seen: Rc<RefCell<Vec<(Vec<String>, bool)>>>,
    /// When set, the class select refuses with `NotYet { NotRead }` (review r1 of #528).
    refuse: Rc<std::cell::Cell<bool>>,
    dir: std::path::PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx(mode: ParityMode, archetype_path: &str) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-cli-harness-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(&dir, &[("WillowGame.upk", &bytes)], IdentityPolicy::FirstInScanOrder, mode)
        .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let record = seen.clone();
    let refuse = Rc::new(std::cell::Cell::new(false));
    let refusing = refuse.clone();
    vm.register_native(
        "WillowPlayerController.ServerPlayerSelectClass(string)",
        Rc::new(move |c: &mut NativeCall<'_>| {
            let thing = c.vm.find_class("WillowGame.Thing")?;
            let loaded = c.vm.space.live(thing).next().is_some();
            record.borrow_mut().push((c.vm.log.clone(), loaded));
            if refusing.get() {
                return Err(ow_core::NotYet::not_read("a toy class select refusal", "toy record").into());
            }
            Ok(Value::None)
        }),
    )
    .unwrap();
    // The toy class select probes `WillowGame.Thing` (`find_class`), which would load that class's code inside the
    // step; warm it here, so the step's own writes are what the marking sees (lane RUNTIME-MARK-CODE-LOADS: a code
    // load marks a refusal).
    vm.find_class("WillowGame.Thing").unwrap();
    let class = vm.find_class("WillowGame.WillowPlayerController").unwrap();
    let controller = ow_vm::interp::instantiate_from_defaults(&mut vm, class).unwrap();
    let dclass = vm.find_class("WillowGame.PlayerClassDefinition").unwrap();
    let definition = vm.space.alloc(dclass, "ToyClass", None, Origin::Runtime).unwrap();
    let p = vm.space.prop(dclass, "PawnArchetypePath").unwrap();
    vm.space.write(definition, PropAddr::scalar(p), Value::Str(archetype_path.into())).unwrap();
    Fx { vm, controller, definition, seen, refuse, dir }
}

fn harness_lines(vm: &Vm) -> Vec<String> {
    vm.log.iter().filter(|l| l.starts_with("harness input:")).cloned().collect()
}

#[test]
fn off_preloads_then_selects_the_class_and_logs_both() {
    let mut x = fx(ParityMode::Off, "WillowGame.ToyPawnArchetype");
    apply_class_inputs(&mut x.vm, x.controller, Some(x.definition)).unwrap();
    let lines = harness_lines(&x.vm);
    assert_eq!(lines.len(), 12, "{lines:?}");
    assert!(lines[0].starts_with("harness input: preload WillowGame.ToyPawnArchetype (UNVERIFIED"), "{lines:?}");
    assert_eq!(lines[1], "harness input: preload WillowGame.ToyPawnArchetype: loaded", "{lines:?}");
    // Maya's tree (lane IMPL-MESH-REST): this toy package set has none, so the attempt is logged and finds nothing.
    assert!(lines[2].starts_with("harness input: preload GD_PlayerShared.Character.AnimTree_Player (UNVERIFIED"), "{lines:?}");
    assert_eq!(lines[3], "harness input: preload GD_PlayerShared.Character.AnimTree_Player: found nothing", "{lines:?}");
    // Maya's mesh data (lane SLICE-HARNESS-MAYA-MESH): the skeleton source is installed; the toy package holds no
    // mesh, so the preload is logged and skipped.
    assert!(lines[4].starts_with("harness input: the skeleton source over UModel .psk exports in "), "{lines:?}");
    assert!(x.vm.service::<ow_world::world::skeletal::SkeletalMeshes>().is_some(), "the source is installed");
    let mesh = "harness input: preload Char_Siren.Mesh.Skel_SirenBody (Maya's mesh; UNVERIFIED";
    assert!(lines[5].starts_with(mesh), "{lines:?}");
    assert_eq!(lines[6], "harness input: preload Char_Siren.Mesh.Skel_SirenBody: found nothing", "{lines:?}");
    // The mesh's sockets (lane IMPL-BODYCOMP-SOCKET-BONE-TABLES): no mesh here either, so logged and skipped.
    assert!(lines[7].starts_with("harness input: preload the sockets of Char_Siren.Mesh.Skel_SirenBody (UNVERIFIED"), "{lines:?}");
    assert_eq!(lines[8], "harness input: preload the sockets of Char_Siren.Mesh.Skel_SirenBody: found nothing", "{lines:?}");
    // The body composition's hook 1 set-up (lane HARNESS-GEARLIKENESS-SETUP): the toy packages hold neither class.
    let gear = "harness input: set-up class WillowGame.IGearLikenessConsumer: not in the packages, nothing loaded";
    assert_eq!(lines[9], gear, "{lines:?}");
    assert!(lines[10].starts_with("harness input: set-up interface check Engine.SkeletalMeshComponent implements"), "{lines:?}");
    assert!(lines[10].ends_with("not in the packages, nothing resolved"), "{lines:?}");
    assert!(lines[11].starts_with("harness input: ServerPlayerSelectClass("), "{lines:?}");
    let seen = x.seen.borrow();
    assert_eq!(seen.len(), 1, "the class select ran once");
    let (log_then, loaded_then) = &seen[0];
    assert!(*loaded_then, "the archetype was in memory when the class select ran");
    assert_eq!(harness_lines_of(log_then), lines, "both lines were written before the call");
}

fn harness_lines_of(log: &[String]) -> Vec<String> {
    log.iter().filter(|l| l.starts_with("harness input:")).cloned().collect()
}

#[test]
fn a_preload_that_finds_nothing_or_cannot_parse_is_logged_and_the_select_still_runs() {
    let mut x = fx(ParityMode::Off, "WillowGame.Missing");
    apply_class_inputs(&mut x.vm, x.controller, Some(x.definition)).unwrap();
    let lines = harness_lines(&x.vm);
    assert!(lines[1].contains("found nothing"), "{lines:?}");
    assert_eq!(x.seen.borrow().len(), 1);
    let mut x = fx(ParityMode::Off, "");
    apply_class_inputs(&mut x.vm, x.controller, Some(x.definition)).unwrap();
    let lines = harness_lines(&x.vm);
    assert!(lines[0].starts_with("harness input: preload skipped"), "{lines:?}");
    assert_eq!(x.seen.borrow().len(), 1);
}

#[test]
fn reference_does_neither_input() {
    let mut x = fx(ParityMode::Reference, "WillowGame.ToyPawnArchetype");
    apply_class_inputs(&mut x.vm, x.controller, Some(x.definition)).unwrap();
    assert!(harness_lines(&x.vm).is_empty());
    assert!(x.seen.borrow().is_empty(), "no class select");
    let thing = x.vm.find_class("WillowGame.Thing").unwrap();
    assert!(x.vm.space.live(thing).next().is_none(), "no preload");
    assert!(x.vm.service::<ow_world::world::skeletal::SkeletalMeshes>().is_none(), "no skeleton source");
}

/// Review r1 of #528, item 1, and r2: a same-package reference whose load itself fails stops the step with the store's
/// `NotYet`, its kind kept, and marked (the preload and the failed load are writes). A retry meets the cached failure:
/// the same typed refusal, unmarked (nothing was written). The class select runs on neither attempt.
#[test]
fn a_failed_same_package_load_keeps_its_kind_and_is_marked() {
    let mut x = fx(ParityMode::Off, "WillowGame.ToyLinkedArchetype");
    let mut first = String::new();
    for attempt in 0..2 {
        match apply_class_inputs(&mut x.vm, x.controller, Some(x.definition)) {
            Err(ow_vm::error::VmError::NotYet(n)) => {
                assert_eq!(n.missing, ow_core::notyet::Missing::NotRead, "attempt {attempt}");
                assert!(n.what.contains("archetype chain is a cycle"), "{n}");
                let marked = n.what.starts_with("stopped after partial writes: ");
                assert_eq!(marked, attempt == 0, "attempt {attempt}: {n}");
                if attempt == 0 {
                    first = n.what.to_string();
                } else {
                    assert_eq!(format!("stopped after partial writes: {}", n.what), first, "the same error");
                }
            }
            other => panic!("attempt {attempt}: expected the load's NotYet, got {other:?}"),
        }
    }
    assert!(x.seen.borrow().is_empty(), "no class select on either attempt");
}

/// Review r1 of #528, item 1: the write state is carried through the class select, so its refusal after a preload
/// that loaded is marked; with no preload (and so no write), the same refusal is returned as it is.
#[test]
fn a_class_select_refusal_is_marked_only_after_a_write() {
    for (path, marked) in [("WillowGame.ToyPawnArchetype", true), ("", false)] {
        let mut x = fx(ParityMode::Off, path);
        x.refuse.set(true);
        match apply_class_inputs(&mut x.vm, x.controller, Some(x.definition)) {
            Err(ow_vm::error::VmError::NotYet(n)) => {
                assert!(n.what.ends_with("a toy class select refusal"), "{n}");
                assert_eq!(n.what.starts_with("stopped after partial writes: "), marked, "{path:?}: {n}");
            }
            other => panic!("expected the select's NotYet, got {other:?}"),
        }
        assert_eq!(x.seen.borrow().len(), 1, "the class select ran");
    }
}
