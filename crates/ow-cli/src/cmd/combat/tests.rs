//! The `--combat-run` parity with the C++ binary on invented packages (AGENTS.md rule 9): the packages of
//! `ow-combat`'s fixture (`crates/ow-combat/src/run/fixture.rs`, included below), sixteen step lists of
//! `tests/combat_test.py`'s scenarios plus the armored pawn's `AdjustDamage` ones.
//!
//! **Where the expected digests come from.** Each is the FNV-1a 64 of the C++ binary's stdout line (its newline read as `\n`) for that step list on
//! the same package bytes (`cargo test -p ow-combat -- --ignored write_fixture` wrote them; `build/Release/
//! ow-package.exe <dir>/ToyPawns.upk --combat-run --cooked <dir> <steps>` ran on them, 2026-11-16, pc-hummer; re-run on the r2 bytes at 881ea3c, unchanged), after
//! [`normalize`]: the two timing fields removed (`milliseconds`, `elapsed_ms`: each binary's own clock) and every
//! `"SetHealth <value>"` event line removed. Those lines are the C++ pool half's log, which the Rust `SetHealth`
//! (W5-POOLS) does not write (`ow_combat::run`, module documentation). Nothing else is changed: the rest of each
//! report, rows, events, call order, state, stubs, notes and errors, is byte-identical.
//!
//! Every scenario's pawn report reaches the Willow hold query. Off refuses that unread virtual; Reference keeps
//! the C++ stand-in. The armored scenarios also reach attributes not declared by these toy classes in Reference.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_core::ParityMode;

use super::world;

#[allow(dead_code)]
#[path = "../../../../ow-combat/src/run/fixture.rs"]
mod fixture;

#[path = "harness_tests.rs"]
mod harness_tests;
#[path = "mesh_harness_tests.rs"]
mod mesh_harness_tests;
// Every slice-gate pack registered together, with no duplicate key (#744's landing check).
#[path = "pack_wiring_tests.rs"]
mod pack_wiring_tests;

/// The scenarios: name, steps, the C++ digest, and whether Off mode equals it (else only Reference does).
const CASES: [(&str, &str, u64, bool); 16] = [
    (
        "pools",
        "spawn:ToyPawns/Pawn_Toy:4 sethealth:0:0.5 sethealth:0:-3 sethealth:0:1.5 sethealth:0:500 setmax:0:99 pool:0:Pool_Health_Copy pool:0:Pool_Ammo pool:0:Pool_Ammo_Copy copyref:0:1 valid:0:3 unpool:0:1 valid:0:3 pool:0:Pool_Ammo valid:0:3 valid:0:4 destroyhealth:0 sethealth:0:7 pool:0:Pool_Health",
        0x7d03_6655_4536_18f2,
        false,
    ),
    (
        "hit",
        "spawn:ToyPawns/Pawn_Toy:4 player:8 hit:0:10::Bullet sethealth:0:40.5 hold:0:Safe hit:0:10::Bullet sethealth:0:40.5 hold:0:Other release:0:Safe hit:0:10::Bullet release:0:Other sethealth:0:40.5 hit:0:10::Bullet",
        0xe5f4_8be8_9c0b_a963,
        // Off refuses the unread Willow override (pinned below; decision 2026-11-16-83).
        false,
    ),
    (
        "death",
        "spawn:ToyPawns/Pawn_Doomed:4 player:8 hit:0:10::Bullet destroyhealth:0",
        0x86ec_ba2d_c2ea_3bf0,
        false,
    ),
    (
        "xp1",
        "spawn:ToyPawns/Pawn_Doomed:5 player:4 players:1 hit:0:10::Bullet",
        0xc207_d47b_66fa_077e,
        false,
    ),
    (
        "xp2",
        "spawn:ToyPawns/Pawn_Doomed:5 player:4 player:7 players:2 hit:0:10::Bullet",
        0xf106_ebb9_b6f1_aa37,
        false,
    ),
    (
        "xp3",
        "spawn:ToyPawns/Pawn_Doomed:5 player:1 players:1 hit:0:10::Bullet",
        0xb4ad_0f14_88e9_6ec6,
        false,
    ),
    (
        "xp4",
        "spawn:ToyPawns/Pawn_Doomed:5 player:5 players:1 hit:0:10::Bullet",
        0x4365_a8f4_6ee9_eed7,
        false,
    ),
    (
        "xp5",
        "spawn:ToyPawns/Pawn_Doomed:5 player:4 hit:0:10::Bullet",
        0x2dac_9831_a0f8_c17c,
        false,
    ),
    (
        "swing",
        "spawn:ToyPawns/Pawn_Toy:4 player:8 swing:0:Melee_Toy swing:0:Throw_Toy:Cause_Toy swing:0:Throw_Toy swing:0:Pawn_Toy swing:0:NoSuchObject",
        0xeb15_1147_8b46_72da,
        false,
    ),
    (
        "swing2",
        "spawn:ToyPawns/Pawn_Toy:4 swing:0:Melee_Toy",
        0x5ed0_b100_bb5e_fd73,
        false,
    ),
    (
        "down",
        "spawn:ToyPawns/Pawn_Down:4 player:8 hit:0:10::Bullet",
        0x991a_9371_6809_5c9f,
        false,
    ),
    (
        "down2",
        "spawn:ToyPawns/Pawn_Down:4 bleed:1",
        0xa387_af0e_fc77_419f,
        false,
    ),
    (
        "down3",
        "spawn:ToyPawns/Pawn_Down:4 cash:100",
        0xad02_e542_28e8_011d,
        false,
    ),
    (
        "armor",
        "spawn:ToyPawns/Pawn_Armored:4 player:8 attr:0:InstigatedBulletDamageModifier:1.2 hit:0:100:DamageType_Toy:Bullet hit:0:100::Bullet hit:0:100:DamageType_Toy:Melee",
        0x1e8e_9241_fc06_215e,
        false,
    ),
    (
        "armor2",
        "spawn:ToyPawns/Pawn_Armored:8 player:8 hit:0:50:DamageType_Toy:Grenade",
        0xa1fa_abcd_e08d_0894,
        false,
    ),
    (
        "bodied",
        "spawn:ToyPawns/Pawn_Bodied:4 player:8 attr:0:InstigatedBulletDamageModifier:1.2 hit:0:100:DamageType_Toy:Bullet hit:0:100:DamageType_Resist:Bullet",
        0x069b_83b6_24fe_fc8a,
        false,
    ),
];

/// FNV-1a 64.
fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Removes `,"<key>":<number>` (a number as the reports print it).
fn drop_number_field(text: &str, key: &str) -> String {
    let pattern = format!(",\"{key}\":");
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(&pattern) {
        out.push_str(&rest[..at]);
        let after = &rest[at + pattern.len()..];
        let end = after
            .find(|c: char| !(c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e')))
            .unwrap_or(after.len());
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

/// The normalization of the module documentation.
fn normalize(text: &str) -> String {
    let text = drop_number_field(text, "milliseconds");
    let text = drop_number_field(&text, "elapsed_ms");
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(at) = rest.find("\"SetHealth ") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let close = after.find('"').map_or(after.len(), |i| i + 1);
        let after = &after[close..];
        rest = after.strip_prefix(',').unwrap_or(after);
    }
    out.push_str(rest);
    out
}

static NEXT: AtomicU32 = AtomicU32::new(0);

/// The fixture written to a fresh folder (removed on drop).
struct Install(PathBuf);

impl Install {
    fn new() -> Install {
        let dir = std::env::temp_dir().join(format!(
            "ow-cli-combat-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, bytes) in fixture::files() {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
        Install(dir)
    }
}

impl Drop for Install {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The line the command prints for `steps` in `mode` (its newline included, as the digests hash it).
fn report(install: &Install, steps: &str, mode: ParityMode) -> (String, bool) {
    let w = world(&install.0, mode).unwrap();
    let steps: Vec<String> = steps.split(' ').map(str::to_string).collect();
    let (text, failed) = ow_combat::run::combat_run(w, &steps).unwrap();
    (
        format!(
            "{text}
"
        ),
        failed,
    )
}

#[test]
fn the_normalization_removes_only_timing_fields_and_set_health_lines() {
    let text = r#"{"a":1,"milliseconds":0.25,"events":["TakeDamage x","SetHealth -5.000000","NotifyTakeHit"],"elapsed_ms":3e-05}"#;
    assert_eq!(
        normalize(text),
        r#"{"a":1,"events":["TakeDamage x","NotifyTakeHit"]}"#
    );
}

#[test]
fn combat_run_equals_the_cxx_binary_on_the_invented_packages() {
    let install = Install::new();
    for (name, steps, cxx, off_equal) in CASES {
        let (reference, _) = report(&install, steps, ParityMode::Reference);
        assert_eq!(
            fnv1a64(normalize(&reference).as_bytes()),
            cxx,
            "{name} (Reference): {reference}"
        );
        let (off, _) = report(&install, steps, ParityMode::Off);
        let same = fnv1a64(normalize(&off).as_bytes()) == cxx;
        assert_eq!(same, off_equal, "{name} (Off): {off}");
    }
}

/// Replay determinism (lane W6-COMBAT-RUN2): each list run twice in the same process, in each mode, prints the same
/// line once the two timing fields are removed.
#[test]
fn every_list_replays_to_the_same_report_in_the_same_process() {
    let install = Install::new();
    for (name, steps, _, _) in CASES {
        for mode in [ParityMode::Reference, ParityMode::Off] {
            let (first, a) = report(&install, steps, mode);
            let (second, b) = report(&install, steps, mode);
            assert_eq!(a, b, "{name} ({mode:?})");
            assert_eq!(normalize(&first), normalize(&second), "{name} ({mode:?})");
        }
    }
}

/// Off stops before the attributes at the Willow component's demigod list: since decision 2026-11-19-43 Off reads it
/// (`WillowAIComponent` keeps the base routine), and the pinned toy `AIComponent` declares no `DemigodHolds`.
#[test]
fn off_mode_stops_before_undeclared_attributes_at_the_willow_hold() {
    let install = Install::new();
    let (off, failed) = report(&install, CASES[13].1, ParityMode::Off);
    assert!(failed);
    assert!(off.contains("AIComponent has no DemigodHolds array"), "{off}");
    let (reference, failed) = report(&install, CASES[13].1, ParityMode::Reference);
    assert!(!failed, "{reference}");
    assert!(
        reference.contains("\"hit.Armored.L4.1.applied\":60"),
        "{reference}"
    );
}

/// Off reads the Willow component's demigod list (decision 2026-11-19-43; the pinned toy `AIComponent` declares no
/// `DemigodHolds`, a typed error); Reference retains the C++ stand-in.
#[test]
fn off_mode_reads_the_willow_demigod_list() {
    let install = Install::new();
    let (off, failed) = report(&install, CASES[1].1, ParityMode::Off);
    assert!(failed, "{off}");
    assert!(off.contains("AIComponent has no DemigodHolds array"), "{off}");
    let (reference, failed) = report(&install, CASES[1].1, ParityMode::Reference);
    assert!(!failed, "{reference}");
    assert!(reference.contains("\"hit.Toy.L4.4.health_after\":0"), "{reference}");
}

#[test]
fn a_run_with_a_maya_step_is_built_over_the_mission_bridge() {
    let args: Vec<String> = ["ToyPawns.upk", "--combat-run", "--cooked", "x", "spawn:dummy:1", "maya:8"]
        .map(String::from)
        .to_vec();
    assert!(ow_combat::run::wants_bridge(&args[4..]));
    assert!(!ow_combat::run::wants_bridge(&args[4..5]));
}

/// Hub ruling (a), lane SPAWN-COMPLETE: the streaming load's stand-in loads the preloaded archetype's references that
/// live in the archetype's own package, and only those. Invented packages: `GD_ToyHome` holds the archetype and
/// `HomeTarget`; `GD_ToyOther` holds `OtherTarget`. The archetype's `Same` names `HomeTarget` and its `Other` names
/// `OtherTarget`; both are unloaded handles after the archetype loads.
#[test]
fn the_stand_in_loads_only_the_archetype_s_same_package_references() {
    use std::rc::Rc;

    use ow_attr::AttributeSystem;
    use ow_core::ObjectPath;
    use ow_defs::DefStore;
    use ow_defs::identity::IdentityPolicy;
    use ow_object::heap::ObjectSpace;
    use ow_object::value::Value;
    use ow_package::testkit::script::ScriptToy;
    use ow_vm::api::Vm;

    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!("ow-cli-standin-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let core = {
        let mut t = ScriptToy::new(&[]);
        t.class("Object", 0);
        t.build()
    };
    let willow = {
        let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        let arch = t.class("ToyArch", object);
        t.prop("Object", arch, "Same", 0, object);
        t.prop("Object", arch, "Other", 0, object);
        t.build()
    };
    let none_only = |t: &ScriptToy| {
        let mut p = ow_package::testkit::ints(&[0]);
        p.extend_from_slice(&t.none);
        p
    };
    let other = {
        let mut t = ScriptToy::with_imports(&["Class"]);
        let willow = t.p.add_import("Core", "Package", 0, "WillowGame");
        let class = t.p.add_import("Core", "Class", willow, "ToyArch");
        let payload = none_only(&t);
        t.p.add_export(class, "OtherTarget", payload);
        t.build()
    };
    let home = {
        let mut t = ScriptToy::with_imports(&["Class"]);
        let willow = t.p.add_import("Core", "Package", 0, "WillowGame");
        let class = t.p.add_import("Core", "Class", willow, "ToyArch");
        let other_package = t.p.add_import("Core", "Package", 0, "GD_ToyOther");
        let other_target = t.p.add_import("WillowGame", "ToyArch", other_package, "OtherTarget");
        let payload = none_only(&t);
        let home_target = t.p.add_export(class, "HomeTarget", payload);
        let mut arch = ow_package::testkit::ints(&[0]);
        arch.extend(t.p.tag("Same", "ObjectProperty", &ow_package::testkit::ints(&[home_target])));
        arch.extend(t.p.tag("Other", "ObjectProperty", &ow_package::testkit::ints(&[other_target])));
        arch.extend_from_slice(&t.none);
        t.p.add_export(class, "Pawn_Toy", arch);
        t.build()
    };
    let files: Vec<(&str, &[u8])> = vec![
        ("Core.upk", core.as_slice()),
        ("WillowGame.upk", willow.as_slice()),
        ("GD_ToyHome.upk", home.as_slice()),
        ("GD_ToyOther.upk", other.as_slice()),
    ];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).expect("store");
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let path = ObjectPath::parse("GD_ToyHome.Pawn_Toy").unwrap();
    let arch = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
    let class = vm.space.heap.get(arch).unwrap().class;
    let read = |vm: &Vm, name: &str| {
        let p = vm.space.prop(class, name).unwrap();
        match vm.space.read(arch, ow_object::ids::PropAddr::scalar(p)).unwrap() {
            Value::Object(Some(o)) => *o,
            other => panic!("{name}: {other:?}"),
        }
    };
    let (same, other) = (read(&vm, "Same"), read(&vm, "Other"));
    assert!(vm.defs.is_unloaded(&vm.space, same) && vm.defs.is_unloaded(&vm.space, other), "both start unloaded");
    let loaded = super::load_same_package_references(&mut vm, arch).unwrap();
    assert_eq!(loaded, vec!["GD_ToyHome.HomeTarget".to_string()]);
    assert!(!vm.defs.is_unloaded(&vm.space, same), "the same-package reference is loaded");
    assert!(vm.defs.is_unloaded(&vm.space, other), "the other package's reference is not");
    assert!(vm.log.iter().any(|l| l.contains("GD_ToyHome.HomeTarget") && l.contains("UNVERIFIED")), "logged");
    drop(vm);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The body-composition walk (lane LINK1-HEAD-MESH, the hub's ruling): over the templates, each loaded only if it is an
/// export of the home package file, then its `SkeletalMesh` by the same rule. Invented packages: `GD_ToyHome` holds the
/// holder, `TemplateA` and `TemplateB` (each naming its own mesh, `MeshA` and `MeshB`, both home exports); `GD_ToyOther`
/// holds `TemplateC`. All start as unloaded handles. A, B and their meshes load; C is skipped with a line naming it.
/// Then the cap: a walk already at it visits nothing more.
#[test]
fn the_body_walk_loads_home_templates_and_meshes_and_skips_another_package() {
    use std::rc::Rc;

    use ow_attr::AttributeSystem;
    use ow_core::ObjectPath;
    use ow_defs::DefStore;
    use ow_defs::identity::IdentityPolicy;
    use ow_object::heap::ObjectSpace;
    use ow_object::value::Value;
    use ow_package::testkit::script::ScriptToy;
    use ow_vm::api::Vm;

    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!("ow-cli-bodywalk-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let core = {
        let mut t = ScriptToy::new(&[]);
        t.class("Object", 0);
        t.build()
    };
    let willow = {
        let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        let holder = t.class("ToyHolder", object);
        for n in ["A", "B", "C"] {
            t.prop("Object", holder, n, 0, object);
        }
        let comp = t.class("ToyComp", object);
        t.prop("Object", comp, "SkeletalMesh", 0, object);
        t.class("ToyMesh", object);
        t.build()
    };
    let none_only = |t: &ScriptToy| {
        let mut p = ow_package::testkit::ints(&[0]);
        p.extend_from_slice(&t.none);
        p
    };
    let other = {
        let mut t = ScriptToy::with_imports(&["Class"]);
        let willow = t.p.add_import("Core", "Package", 0, "WillowGame");
        let comp = t.p.add_import("Core", "Class", willow, "ToyComp");
        let payload = none_only(&t);
        t.p.add_export(comp, "TemplateC", payload);
        t.build()
    };
    let home = {
        let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
        let willow = t.p.add_import("Core", "Package", 0, "WillowGame");
        let holder = t.p.add_import("Core", "Class", willow, "ToyHolder");
        let comp = t.p.add_import("Core", "Class", willow, "ToyComp");
        let mesh = t.p.add_import("Core", "Class", willow, "ToyMesh");
        let other_package = t.p.add_import("Core", "Package", 0, "GD_ToyOther");
        let c = t.p.add_import("WillowGame", "ToyComp", other_package, "TemplateC");
        let payload = none_only(&t);
        let mesh_a = t.p.add_export(mesh, "MeshA", payload.clone());
        let mesh_b = t.p.add_export(mesh, "MeshB", payload);
        let template = |t: &mut ScriptToy, name: &str, m: i32| {
            let mut p = ow_package::testkit::ints(&[0]);
            p.extend(t.p.tag("SkeletalMesh", "ObjectProperty", &ow_package::testkit::ints(&[m])));
            p.extend_from_slice(&t.none);
            t.p.add_export(comp, name, p)
        };
        let a = template(&mut t, "TemplateA", mesh_a);
        let b = template(&mut t, "TemplateB", mesh_b);
        let mut h = ow_package::testkit::ints(&[0]);
        for (n, r) in [("A", a), ("B", b), ("C", c)] {
            h.extend(t.p.tag(n, "ObjectProperty", &ow_package::testkit::ints(&[r])));
        }
        h.extend_from_slice(&t.none);
        t.p.add_export(holder, "Holder", h);
        t.build()
    };
    let files: Vec<(&str, &[u8])> = vec![
        ("Core.upk", core.as_slice()),
        ("WillowGame.upk", willow.as_slice()),
        ("GD_ToyHome.upk", home.as_slice()),
        ("GD_ToyOther.upk", other.as_slice()),
    ];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).expect("store");
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let holder = vm.defs.find(&mut vm.space, &ObjectPath::parse("GD_ToyHome.Holder").unwrap()).unwrap().unwrap();
    let read = |vm: &Vm, o: ow_object::ids::ObjRef, name: &str| {
        let c = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(c, name).unwrap();
        match vm.space.read(o, ow_object::ids::PropAddr::scalar(p)).unwrap() {
            Value::Object(Some(x)) => *x,
            other => panic!("{name}: {other:?}"),
        }
    };
    let (a, b, c) = (read(&vm, holder, "A"), read(&vm, holder, "B"), read(&vm, holder, "C"));
    for t in [a, b, c] {
        assert!(vm.defs.is_unloaded(&vm.space, t), "every template starts unloaded");
    }
    let home_id = super::file_package(&vm, holder).unwrap();
    let mut walk = super::BodyWalk { home: home_id, package: "GD_ToyHome".into(), visited: Vec::new(), loaded: 0 };
    super::walk_templates(&mut vm, &mut walk, &[a, b, c]).unwrap();
    for t in [a, b] {
        assert!(!vm.defs.is_unloaded(&vm.space, t), "a home template is loaded");
        let m = read(&vm, t, "SkeletalMesh");
        assert!(!vm.defs.is_unloaded(&vm.space, m), "and its home mesh");
    }
    assert!(vm.defs.is_unloaded(&vm.space, c), "the other package's template is not loaded");
    assert_eq!(walk.loaded, 4, "two templates and two meshes");
    assert!(
        vm.log.iter().any(|l| l.contains("GD_ToyOther.TemplateC") && l.contains("is not an export of GD_ToyHome") && l.contains("its branch stops")),
        "the skipped template is logged: {:?}",
        vm.log
    );
    assert!(vm.log.iter().any(|l| l.contains("GD_ToyHome.TemplateA") && l.contains("UNVERIFIED")), "each load is labelled");

    // The cap: a walk that has visited the cap's worth of objects visits nothing more, and says so.
    let mut full = super::BodyWalk { home: home_id, package: "GD_ToyHome".into(), visited: vec![holder; super::BODY_WALK_CAP], loaded: 0 };
    super::walk_templates(&mut vm, &mut full, &[c]).unwrap();
    assert_eq!(full.visited.len(), super::BODY_WALK_CAP);
    assert!(vm.log.last().unwrap().contains("stopped at the cap of 64 objects"), "{:?}", vm.log.last());
    drop(vm);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review of #727, LOW: the walk over a duplicated path. `GD_AOther.upk` and `GD_ToyHome.upk` each hold
/// `GD_Shared.Cube`; `GD_AOther` comes first in the scan, so its copy is the policy's. Home's `TemplateA` names home's
/// own cube. The walk loads home's copy into the cube's handle and logs it; with the other copy already resident, the
/// store's conflict is the walk's error.
#[test]
fn the_body_walk_loads_the_home_file_s_own_copy_of_a_duplicated_path() {
    use std::rc::Rc;

    use ow_attr::AttributeSystem;
    use ow_core::ObjectPath;
    use ow_defs::DefStore;
    use ow_defs::identity::IdentityPolicy;
    use ow_object::heap::{ObjectSpace, Origin};
    use ow_object::value::Value;
    use ow_package::testkit::script::ScriptToy;
    use ow_vm::api::Vm;

    static NEXT: AtomicU32 = AtomicU32::new(0);
    let core = {
        let mut t = ScriptToy::new(&[]);
        t.class("Object", 0);
        t.build()
    };
    let willow = {
        let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        let holder = t.class("ToyHolder", object);
        t.prop("Object", holder, "A", 0, object);
        let comp = t.class("ToyComp", object);
        t.prop("Object", comp, "SkeletalMesh", 0, object);
        t.class("ToyMesh", object);
        t.build()
    };
    // A file holding `GD_Shared.Cube` under a package export, then `extra` exports: (exports, the cube's index).
    let with_cube = |extra: bool| {
        let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let package = t.p.add_import("Core", "Class", core, "Package");
        let willow = t.p.add_import("Core", "Package", 0, "WillowGame");
        let mesh = t.p.add_import("Core", "Class", willow, "ToyMesh");
        let shared = t.p.add_export(package, "GD_Shared", ow_package::testkit::ints(&[0]));
        let mut payload = ow_package::testkit::ints(&[0]);
        payload.extend_from_slice(&t.none);
        let mut cube = ow_package::testkit::Export::new(mesh, "Cube", payload);
        cube.outer = shared;
        let cube = t.p.add_export_with(cube);
        if extra {
            let comp = t.p.add_import("Core", "Class", willow, "ToyComp");
            let holder = t.p.add_import("Core", "Class", willow, "ToyHolder");
            let mut p = ow_package::testkit::ints(&[0]);
            p.extend(t.p.tag("SkeletalMesh", "ObjectProperty", &ow_package::testkit::ints(&[cube])));
            p.extend_from_slice(&t.none);
            let a = t.p.add_export(comp, "TemplateA", p);
            let mut h = ow_package::testkit::ints(&[0]);
            h.extend(t.p.tag("A", "ObjectProperty", &ow_package::testkit::ints(&[a])));
            h.extend_from_slice(&t.none);
            t.p.add_export(holder, "Holder", h);
        }
        t.build()
    };
    let (other, home) = (with_cube(false), with_cube(true));
    let vm_of = |tag: &str| {
        let dir = std::env::temp_dir().join(format!("ow-cli-bodywalk-dup-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let _ = std::fs::remove_dir_all(&dir);
        let files: Vec<(&str, &[u8])> = vec![
            ("Core.upk", core.as_slice()),
            ("WillowGame.upk", willow.as_slice()),
            ("GD_AOther.upk", other.as_slice()),
            ("GD_ToyHome.upk", home.as_slice()),
        ];
        let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).expect("store");
        (Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off), dir)
    };
    let read = |vm: &Vm, o: ow_object::ids::ObjRef, name: &str| {
        let c = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(c, name).unwrap();
        match vm.space.read(o, ow_object::ids::PropAddr::scalar(p)).unwrap() {
            Value::Object(Some(x)) => *x,
            other => panic!("{name}: {other:?}"),
        }
    };
    let start = |vm: &mut Vm| {
        let holder = vm.defs.find(&mut vm.space, &ObjectPath::parse("GD_ToyHome.Holder").unwrap()).unwrap().unwrap();
        let a = read(vm, holder, "A");
        let home = super::file_package(vm, holder).unwrap();
        (a, home)
    };

    let (mut vm, dir) = vm_of("own");
    let (a, home_id) = start(&mut vm);
    let mut walk = super::BodyWalk { home: home_id, package: "GD_ToyHome".into(), visited: Vec::new(), loaded: 0 };
    super::walk_templates(&mut vm, &mut walk, &[a]).unwrap();
    let cube = read(&vm, a, "SkeletalMesh");
    assert!(matches!(vm.space.heap.get(cube).unwrap().origin, Origin::Export { package, .. } if package == home_id), "home's own copy");
    assert_eq!(walk.loaded, 2, "the template and the cube");
    assert!(vm.log.iter().any(|l| l.contains("GD_Shared.Cube: loaded GD_ToyHome's own copy")), "{:?}", vm.log);
    drop(vm);
    let _ = std::fs::remove_dir_all(&dir);

    let (mut vm, dir) = vm_of("conflict");
    let (a, home_id) = start(&mut vm);
    vm.defs.ensure_loaded(&mut vm.space, a).unwrap();
    let cube = read(&vm, a, "SkeletalMesh");
    vm.defs.ensure_loaded(&mut vm.space, cube).unwrap();
    let mut walk = super::BodyWalk { home: home_id, package: "GD_ToyHome".into(), visited: Vec::new(), loaded: 0 };
    let err = super::walk_templates(&mut vm, &mut walk, &[a]).unwrap_err();
    assert!(err.to_string().contains("identity conflict"), "{err}");
    assert!(vm.log.iter().any(|l| l.contains("GD_Shared.Cube: failed")), "{:?}", vm.log);
    drop(vm);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The walk's weapon-hold part (lane IMPL-GETWEAPONHOLDDEF, the hub's ruling): from the body class along
/// `WeaponActionParentClass`, each class's `WeaponHoldDefs` entries and `DefaultWeaponHoldDef`, loaded only when they
/// are exports of the home package file. Invented packages: `GD_ToyHome` holds `Body` (holds `H1` and
/// `GD_ToyOther.X`, default `D`, parent `Parent`) and `Parent` (default `PD`); `GD_ToyOther` holds `X`. H1, D, Parent
/// and PD load; X is skipped with a line naming it.
#[test]
fn the_body_walk_loads_home_weapon_holds_and_the_parent_chain() {
    use std::rc::Rc;

    use ow_attr::AttributeSystem;
    use ow_core::ObjectPath;
    use ow_defs::DefStore;
    use ow_defs::identity::IdentityPolicy;
    use ow_object::heap::ObjectSpace;
    use ow_object::value::Value;
    use ow_package::testkit::ints;
    use ow_package::testkit::script::ScriptToy;
    use ow_vm::api::Vm;

    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!("ow-cli-holdwalk-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let core = {
        let mut t = ScriptToy::new(&[]);
        t.class("Object", 0);
        t.build()
    };
    let willow = {
        let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty", "ArrayProperty"]);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        let body = t.class("ToyBody", object);
        let array = t.p.next_export();
        t.prop("Array", body, "WeaponHoldDefs", 0, array + 1);
        t.prop("Object", array, "WeaponHoldDefs", 0, object);
        t.prop("Object", body, "DefaultWeaponHoldDef", 0, object);
        t.prop("Object", body, "WeaponActionParentClass", 0, object);
        t.class("ToyHold", object);
        t.build()
    };
    let none_only = |t: &ScriptToy| {
        let mut p = ints(&[0]);
        p.extend_from_slice(&t.none);
        p
    };
    let other = {
        let mut t = ScriptToy::with_imports(&["Class"]);
        let willow = t.p.add_import("Core", "Package", 0, "WillowGame");
        let hold = t.p.add_import("Core", "Class", willow, "ToyHold");
        let payload = none_only(&t);
        t.p.add_export(hold, "X", payload);
        t.build()
    };
    let home = {
        let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty", "ArrayProperty"]);
        let willow = t.p.add_import("Core", "Package", 0, "WillowGame");
        let body = t.p.add_import("Core", "Class", willow, "ToyBody");
        let hold = t.p.add_import("Core", "Class", willow, "ToyHold");
        let other_package = t.p.add_import("Core", "Package", 0, "GD_ToyOther");
        let x = t.p.add_import("WillowGame", "ToyHold", other_package, "X");
        let payload = none_only(&t);
        let h1 = t.p.add_export(hold, "H1", payload.clone());
        let d = t.p.add_export(hold, "D", payload.clone());
        let pd = t.p.add_export(hold, "PD", payload);
        let mut p = ints(&[0]);
        p.extend(t.p.tag("DefaultWeaponHoldDef", "ObjectProperty", &ints(&[pd])));
        p.extend_from_slice(&t.none);
        let parent = t.p.add_export(body, "Parent", p);
        let mut p = ints(&[0]);
        p.extend(t.p.array_tag("WeaponHoldDefs", 2, &ints(&[h1, x])));
        p.extend(t.p.tag("DefaultWeaponHoldDef", "ObjectProperty", &ints(&[d])));
        p.extend(t.p.tag("WeaponActionParentClass", "ObjectProperty", &ints(&[parent])));
        p.extend_from_slice(&t.none);
        t.p.add_export(body, "Body", p);
        t.build()
    };
    let files: Vec<(&str, &[u8])> = vec![
        ("Core.upk", core.as_slice()),
        ("WillowGame.upk", willow.as_slice()),
        ("GD_ToyHome.upk", home.as_slice()),
        ("GD_ToyOther.upk", other.as_slice()),
    ];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).expect("store");
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let body = vm.defs.find(&mut vm.space, &ObjectPath::parse("GD_ToyHome.Body").unwrap()).unwrap().unwrap();
    let read = |vm: &Vm, o: ow_object::ids::ObjRef, name: &str| {
        let c = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(c, name).unwrap();
        vm.space.read(o, ow_object::ids::PropAddr::scalar(p)).unwrap().clone()
    };
    let one = |v: Value| match v {
        Value::Object(Some(x)) => x,
        other => panic!("{other:?}"),
    };
    let holds: Vec<_> = read(&vm, body, "WeaponHoldDefs").elements().unwrap().iter().cloned().map(one).collect();
    let (h1, x) = (holds[0], holds[1]);
    let (d, parent) = (one(read(&vm, body, "DefaultWeaponHoldDef")), one(read(&vm, body, "WeaponActionParentClass")));
    for o in [h1, x, d, parent] {
        assert!(vm.defs.is_unloaded(&vm.space, o), "every reference starts unloaded");
    }
    let home_id = super::file_package(&vm, body).unwrap();
    let mut walk = super::BodyWalk { home: home_id, package: "GD_ToyHome".into(), visited: Vec::new(), loaded: 0 };
    super::walk_weapon_holds(&mut vm, &mut walk, body).unwrap();
    for o in [h1, d, parent] {
        assert!(!vm.defs.is_unloaded(&vm.space, o), "a home hold, the default and the parent load");
    }
    let pd = one(read(&vm, parent, "DefaultWeaponHoldDef"));
    assert!(!vm.defs.is_unloaded(&vm.space, pd), "and the parent's default");
    assert!(vm.defs.is_unloaded(&vm.space, x), "the other package's hold is not loaded");
    assert_eq!(walk.loaded, 4, "H1, D, Parent and PD");
    assert!(
        vm.log.iter().any(|l| l.contains("GD_ToyOther.X") && l.contains("is not an export of GD_ToyHome")),
        "the skipped hold is logged: {:?}",
        vm.log
    );
    assert!(vm.log.iter().any(|l| l.contains("GD_ToyHome.PD") && l.contains("UNVERIFIED")), "each load is labelled");
    drop(vm);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `--map` parsing (review r1 of #556, item 5; lane MAP-ROUTE-FOLLOWUP): the name and the steps after it, none
/// without the option, the usage message without a name, `NotYet` outside Off.
#[test]
fn the_map_option_is_parsed_after_cooked() {
    use super::{Failure, map_option};
    let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let with = args(&["P.upk", "--combat-run", "--cooked", "dir", "--map", "Sanctuary_P", "maya:8", "spawn:dummy:1"]);
    match map_option(&with, ParityMode::Off) {
        Ok(Some((map, steps))) => {
            assert_eq!(map, "Sanctuary_P");
            assert_eq!(steps, &with[6..]);
        }
        _ => panic!("expected the map and its steps"),
    }
    let without = args(&["P.upk", "--combat-run", "--cooked", "dir", "maya:8"]);
    assert!(matches!(map_option(&without, ParityMode::Off), Ok(None)));
    let nameless = args(&["P.upk", "--combat-run", "--cooked", "dir", "--map"]);
    assert!(matches!(map_option(&nameless, ParityMode::Off), Err(Failure::Message(_))));
    assert!(matches!(map_option(&with, ParityMode::Reference), Err(Failure::NotYet(_))));
    let bare = args(&["P.upk", "--combat-run", "--cooked", "dir", "--map", "Sanctuary_P"]);
    assert!(matches!(map_option(&bare, ParityMode::Off), Ok(Some(("Sanctuary_P", [])))));
}
