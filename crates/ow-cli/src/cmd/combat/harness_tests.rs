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
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend_from_slice(&t.none);
    t.p.add_export(thing, "ToyPawnArchetype", payload);
    t.p.build()
}

struct Fx {
    vm: Vm,
    controller: ObjRef,
    definition: ObjRef,
    /// What the class select saw: the log at that moment, and whether the archetype was in memory.
    seen: Rc<RefCell<Vec<(Vec<String>, bool)>>>,
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
    vm.register_native(
        "WillowPlayerController.ServerPlayerSelectClass(string)",
        Rc::new(move |c: &mut NativeCall<'_>| {
            let thing = c.vm.find_class("WillowGame.Thing")?;
            let loaded = c.vm.space.live(thing).next().is_some();
            record.borrow_mut().push((c.vm.log.clone(), loaded));
            Ok(Value::None)
        }),
    )
    .unwrap();
    let class = vm.find_class("WillowGame.WillowPlayerController").unwrap();
    let controller = ow_vm::interp::instantiate_from_defaults(&mut vm, class).unwrap();
    let dclass = vm.find_class("WillowGame.PlayerClassDefinition").unwrap();
    let definition = vm.space.alloc(dclass, "ToyClass", None, Origin::Runtime).unwrap();
    let p = vm.space.prop(dclass, "PawnArchetypePath").unwrap();
    vm.space.write(definition, PropAddr::scalar(p), Value::Str(archetype_path.into())).unwrap();
    Fx { vm, controller, definition, seen, dir }
}

fn harness_lines(vm: &Vm) -> Vec<String> {
    vm.log.iter().filter(|l| l.starts_with("harness input:")).cloned().collect()
}

#[test]
fn off_preloads_then_selects_the_class_and_logs_both() {
    let mut x = fx(ParityMode::Off, "WillowGame.ToyPawnArchetype");
    apply_class_inputs(&mut x.vm, x.controller, Some(x.definition)).unwrap();
    let lines = harness_lines(&x.vm);
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert!(lines[0].starts_with("harness input: preload WillowGame.ToyPawnArchetype (UNVERIFIED"), "{lines:?}");
    assert_eq!(lines[1], "harness input: preload WillowGame.ToyPawnArchetype: loaded", "{lines:?}");
    assert!(lines[2].starts_with("harness input: ServerPlayerSelectClass("), "{lines:?}");
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
}
