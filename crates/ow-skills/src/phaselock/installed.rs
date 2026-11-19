//! The installed-game run for Maya (local only, ignored): `cargo test -p ow-skills -- --ignored installed_game`
//! with `OW_BL2_ROOT` set to the game's install folder. It reads the installed packages through the definition store
//! and runs the installed script on the Rust VM, then writes what it saw to `local/p2/w6-skills/maya_installed.txt`
//! (ignored: names and numbers from the install stay under `local/`). Nothing from it is copied into a tracked file
//! except counts and the record's own constants.
//!
//! What it runs, in order:
//! 1. Maya's skill tree (`SkillTree_Siren`) through [`crate::tree`]: its branches, tiers and cells, and a spend from
//!    the action skill through Motion's first tier to Suspension grade 5.
//! 2. Every skill of the tree through [`crate::skills::skill_entry`]: the counts NATIVE_SKILLS.md 4.8 recorded for the
//!    2026-11-03 install (31 skills, 50 effect rows, 32 on an attribute and a target, 34 constraints).
//! 3. Suspension's info-box lines through [`crate::skills::skill_lines`].
//! 4. The lock duration: the installed `InstancedDesignerAttribute.SetBaseValue` on `Att_Phaselock_Duration`'s base,
//!    Suspension's modifier from [`crate::action_skill::calculate_modifier_value`] (called directly, not through the
//!    VM) on the installed effect data, on the attribute's `Value` stack (`ow-attr`), for grades 0 to 5, against the
//!    record's `5 + 0.5 g`. What this checks is the installed data (base 5, Suspension 0.5 g) through that path; the
//!    stack rule is `ow-attr`'s on both sides. The game's own chain (the designer context resolver and the pawn's
//!    instanced attribute) is tried first and where it stops is written.
//! 5. The schedule from the installed archetype's numbers (the [`super::Schedule`] oracle).
//! 6. The lift and the bob: the installed `LiftActionSkill.GetLiftLocation` and `GetBobLocation` on the installed
//!    archetype, frame by frame, with `ow-vm`'s `VInterpTo`. Independent of `VInterpTo`: the script's bob target
//!    (before the smoothing) against the record's formula ([`super::oracle::bob_target_z`]). The smoothed bob is
//!    compared with [`super::oracle::vinterp_to`], the record's rule written apart from `ow-vm`'s binding; the rule was
//!    matched live to RMS 0.001 uu (NATIVE_PHASELOCK_TARGETING.md section 5), and `VInterpTo`'s precision is
//!    UNVERIFIED.
//! 7. `LiftActionSkill.OnActionSkillStarted`, to write where the whole cast stops.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_attr::api::EvalCx;
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount,
    Playthrough,
};
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::{StructValue, Value};
use ow_vm::api::{NativePack, Vm};
use ow_vm::natives_core;
use ow_world::world::{BalanceState, World};

use crate::skills::{EffectTarget, Reader, read_skill, skill_entry, skill_lines};
use crate::tree::{SkillTree, read_tree};

fn snapshot() -> BalanceSnapshot {
    BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::NONE,
        PlayerCount::SINGLE,
        ExpLevel::new(8),
        GameStage(8),
        AwesomeLevel::new(0),
        LevelCapUpgrades::NONE,
    )
    .unwrap()
}

fn find(vm: &mut Vm, path: &str) -> Result<ObjRef, String> {
    let p = ObjectPath::parse(path).map_err(|e| format!("{path}: {e:?}"))?;
    vm.defs
        .find(&mut vm.space, &p)
        .map_err(|e| format!("{path}: {e}"))?
        .ok_or_else(|| format!("{path}: not found"))
}

fn set(vm: &mut Vm, o: ObjRef, name: &str, v: Value) -> Result<(), String> {
    let c = vm.space.heap.get(o).map_err(|e| e.to_string())?.class;
    let p = vm
        .space
        .prop(c, name)
        .ok_or_else(|| format!("no property {name}"))?;
    vm.space
        .write(o, PropAddr::scalar(p), v)
        .map_err(|e| e.to_string())
}

fn get(vm: &mut Vm, o: ObjRef, name: &str) -> Result<Value, String> {
    crate::action_skill::need(vm, o, name).map_err(|e| e.to_string())
}

fn float(vm: &mut Vm, o: ObjRef, name: &str) -> Result<f32, String> {
    match get(vm, o, name)? {
        Value::Float(x) => Ok(x),
        other => Err(format!("{name}: {}", other.kind_name())),
    }
}

fn vector(vm: &Vm, [x, y, z]: [f32; 3]) -> Result<Value, String> {
    let classes = &vm.space.classes;
    let s = classes
        .struct_by_path("Core.Object.Vector")
        .ok_or("no Vector struct")?;
    let layout = classes.struct_layout(s).ok_or("no Vector layout")?;
    let mut v = match layout.zero() {
        Value::Struct(s) => StructValue::clone(s),
        _ => return Err("no Vector zero".to_string()),
    };
    for (n, val) in [("X", x), ("Y", y), ("Z", z)] {
        let (e, _) = layout
            .field(vm.space.names.find(n).ok_or("no axis name")?)
            .ok_or("no axis")?;
        v.fields[usize::try_from(e).map_err(|_| "entry")?] = Value::Float(val);
    }
    Ok(Value::Struct(Rc::new(v)))
}

fn axes(vm: &Vm, v: &Value) -> Result<[f32; 3], String> {
    let s = v.as_struct().ok_or("not a vector")?;
    let layout = vm.space.classes.struct_layout(s.ty).ok_or("no layout")?;
    let mut out = [0.0f32; 3];
    for (i, n) in ["X", "Y", "Z"].iter().enumerate() {
        let (e, _) = layout
            .field(vm.space.names.find(n).ok_or("no axis name")?)
            .ok_or("no axis")?;
        out[i] = match s.fields.get(usize::try_from(e).map_err(|_| "entry")?) {
            Some(Value::Float(x)) => *x,
            _ => return Err("an axis that is not a float".to_string()),
        };
    }
    Ok(out)
}

fn call(vm: &mut Vm, path: &str, this: Option<ObjRef>, args: Vec<Value>) -> Result<Value, String> {
    let f = vm.find_function(path).map_err(|e| format!("{path}: {e}"))?;
    vm.call(f, this, args)
        .map(|r| r.ret)
        .map_err(|e| format!("{path}: {e}"))
}

/// An actor of `class` as the world's spawn makes it in Off (decision 2026-11-19-44): from the class defaults, with
/// its own copy of each component they own. Under Reference the C++ stand-in shares them, as `new_of` does.
fn spawned_of(vm: &mut Vm, class: &str) -> Result<ObjRef, String> {
    let made = new_of(vm, class)?;
    if vm.parity == ParityMode::Off {
        let c = vm.space.heap.get(made).map_err(|e| e.to_string())?.class;
        let path = vm.space.classes.get_layout(c).ok_or("no layout")?.path().as_str().to_string();
        let defaults = vm.defs.class_default(&mut vm.space, &path).map_err(|e| format!("{class}: {e}"))?;
        ow_world::actors::instance_components(vm, defaults, made).map_err(|e| format!("{class}: {e}"))?;
    }
    Ok(made)
}

fn new_of(vm: &mut Vm, class: &str) -> Result<ObjRef, String> {
    let c = vm.find_class(class).map_err(|e| format!("{class}: {e}"))?;
    ow_vm::interp::instantiate_from_defaults(vm, c).map_err(|e| format!("{class}: {e}"))
}

/// The record's lock duration at Suspension grade `g` (PHASELOCK_STOCK_DATA.md: base 5, +0.5 per grade from grade 1),
/// the number the installed data are checked against.
fn expected(g: i32) -> f32 {
    // An int grade 0..=5 to float is exact.
    5.0 + 0.5 * g as f32
}

fn g9(x: f32) -> String {
    ow_core::fmt::fmt_g(f64::from(x), 9)
}

/// Runs the parts in order; a part that cannot run writes why and the next part runs.
fn run_all(root: PathBuf, mode: ParityMode) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# AI-assisted (Claude Code). W6-SKILLS installed-game run for Maya, local only. {mode:?}."
    );
    let cooked = root.join("WillowGame").join("CookedPCConsole");
    let defs = match DefStore::open(cooked, IdentityPolicy::FirstInScanOrder, mode) {
        Ok(d) => d,
        Err(e) => {
            let _ = writeln!(out, "store: {e}");
            return out;
        }
    };
    let vm = Vm::new(
        ObjectSpace::new(),
        defs,
        Rc::new(AttributeSystem::standard().unwrap()),
        mode,
    );
    let packs: [&dyn NativePack; 3] = [
        &natives_core::Pack,
        &crate::tree::Pack,
        &crate::action_skill::Pack,
    ];
    let mut w = match World::new(vm, BalanceState::new(snapshot()), &packs) {
        Ok(w) => w,
        Err(e) => {
            let _ = writeln!(out, "world: {e:?}");
            return out;
        }
    };
    // The allegiance natives only (NATIVE_COMBAT_GROUNDWORK.md section 6), not the combat pack's stand-ins: the cast's
    // `GetPlayerAnimation` asks `Pawn.IsFriendly` on its target path (review of #437).
    if let Err(e) = ow_combat::hit::register_allegiance(&mut w.vm) {
        let _ = writeln!(out, "allegiance: {e}");
        return out;
    }
    let wi = match new_of(&mut w.vm, "Engine.WorldInfo") {
        Ok(wi) => wi,
        Err(e) => {
            let _ = writeln!(out, "worldinfo: {e}");
            return out;
        }
    };
    if let Err(e) = w.actors().borrow_mut().place_world_info(wi) {
        let _ = writeln!(out, "worldinfo: {e}");
        return out;
    }
    let vm = &mut w.vm;
    for (name, part) in [
        (
            "tree",
            part_tree as fn(&mut Vm, &mut String) -> Result<(), String>,
        ),
        ("skills", part_skills),
        ("infobox", part_infobox),
        ("duration", part_duration),
        ("chains", part_chains),
        ("schedule", part_schedule),
        ("lift", part_lift),
        ("cast", part_cast),
    ] {
        let _ = writeln!(out, "## {name}");
        if let Err(e) = part(vm, &mut out) {
            let _ = writeln!(out, "{name}: stopped: {e}");
        }
    }
    out
}

const TREE: &str = "GD_Siren_Streaming.SkillTree.SkillTree_Siren";

fn maya_tree(vm: &mut Vm) -> Result<SkillTree, String> {
    let t = find(vm, TREE)?;
    let mut cx = EvalCx::new(vm);
    let def = read_tree(&mut cx, t).map_err(|e| e.to_string())?;
    SkillTree::initialize(&mut cx, def, None).map_err(|e| e.to_string())
}

fn name_of(vm: &Vm, o: ObjRef) -> String {
    ow_behavior::provider::stock_path(vm, o).unwrap_or_default()
}

fn part_tree(vm: &mut Vm, out: &mut String) -> Result<(), String> {
    let mut tree = maya_tree(vm)?;
    let def = tree.def().clone();
    let mut cells_match = 0;
    let mut tiers = 0;
    for (i, b) in def.branches.iter().enumerate() {
        let cum: Vec<String> = b.tiers.iter().map(|t| t.cumulative.to_string()).collect();
        let shape: Vec<String> = b
            .tiers
            .iter()
            .map(|t| format!("{}/{}", t.skills.len(), t.cells))
            .collect();
        let _ = writeln!(
            out,
            "branch {i}: tiers {} cumulative [{}] skills/cells [{}] unlock_total {} children {:?}",
            b.tiers.len(),
            cum.join(" "),
            shape.join(" "),
            b.unlock_total,
            b.children
        );
        for t in &b.tiers {
            tiers += 1;
            cells_match += usize::from(t.skills.len() <= t.cells);
        }
    }
    let _ = writeln!(
        out,
        "tiers whose skills all fit their cells (so every skill counts): {cells_match}/{tiers}"
    );
    let _ = writeln!(
        out,
        "skills in the tree: {}; action skill set: {}",
        tree.skills().len(),
        tree.action_skill().is_some()
    );
    let motion = def
        .branches
        .iter()
        .position(|b| name_of(vm, b.object).ends_with("Branch_Motion"))
        .ok_or("no Motion branch")?;
    let action = tree.action_skill().ok_or("no action skill")?;
    let before = tree
        .upgrade(
            &mut EvalCx::new(vm),
            def.branches[motion].tiers[0].skills[0],
        )
        .map_err(|e| e.to_string())?;
    let up = tree
        .upgrade(&mut EvalCx::new(vm), action)
        .map_err(|e| e.to_string())?;
    let opened = (1..def.branches.len())
        .filter(|&b| tree.unlocked(b, 0).unwrap_or(false))
        .count();
    let _ = writeln!(
        out,
        "spend: a Motion skill before the action skill: {before}; the action skill: {up}; child branches opened: {opened}/{}",
        def.branches.len() - 1
    );
    let first = def.branches[motion].tiers[0].skills[0];
    let mut spent = 0;
    for _ in 0..5 {
        spent += i32::from(
            tree.upgrade(&mut EvalCx::new(vm), first)
                .map_err(|e| e.to_string())?,
        );
    }
    let suspension = def.branches[motion]
        .tiers
        .iter()
        .flat_map(|t| t.skills.iter())
        .copied()
        .find(|s| name_of(vm, *s).ends_with("Motion.Suspension"))
        .ok_or("no Suspension")?;
    let mut grades = Vec::new();
    for _ in 0..6 {
        let ok = tree
            .upgrade(&mut EvalCx::new(vm), suspension)
            .map_err(|e| e.to_string())?;
        grades.push(format!(
            "{ok}:{}",
            tree.grade(&mut EvalCx::new(vm), suspension)
                .map_err(|e| e.to_string())?
                .unwrap_or(-1)
        ));
    }
    let _ = writeln!(
        out,
        "spend: tier-1 skill x5 {spent}/5; Motion tier 2 open: {}; Suspension upgrades {}",
        tree.unlocked(motion, 1).unwrap_or(false),
        grades.join(" ")
    );
    let _ = writeln!(
        out,
        "points spent: {} (expected 1 + 5 + 5 = 11)",
        tree.points_spent().map_err(|e| e.to_string())?
    );
    Ok(())
}

fn tree_skills(tree: &SkillTree) -> Vec<ObjRef> {
    // The action skill first, then every branch's tiers in tree order (prepare_skill_effects.main's order).
    tree.skills().iter().map(|s| s.definition).collect()
}

fn part_skills(vm: &mut Vm, out: &mut String) -> Result<(), String> {
    let tree = maya_tree(vm)?;
    let sys = vm.attrs.clone();
    let mut reader = Reader::new(&sys);
    let (mut skills, mut effects, mut applied, mut conditions, mut failed) =
        (0, 0, 0, 0, Vec::new());
    for s in tree_skills(&tree) {
        let mut cx = EvalCx::new(vm);
        match skill_entry(&mut cx, &mut reader, s) {
            Ok(e) => {
                skills += 1;
                effects += e.effects.len();
                applied += e
                    .effects
                    .iter()
                    .filter(|x| x.attribute.is_some() && x.target != EffectTarget::None)
                    .count();
                conditions += e.conditions.len();
            }
            Err(e) => failed.push(format!("{}: {e}", name_of(vm, s))),
        }
    }
    let _ = writeln!(
        out,
        "skills {skills}, effect rows {effects}, rows on an attribute and a target {applied}, constraint rows {conditions}, unresolved values {}",
        reader.unresolved.len()
    );
    let _ = writeln!(
        out,
        "record (2026-11-03 install, NATIVE_SKILLS.md 4.8): 31 skills, 50 effect rows, 32, 34 constraints"
    );
    for f in &failed {
        let _ = writeln!(out, "skill failed: {f}");
    }
    Ok(())
}

fn suspension(vm: &mut Vm) -> Result<ObjRef, String> {
    find(vm, "GD_Siren_Skills.Motion.Suspension")
}

fn part_infobox(vm: &mut Vm, out: &mut String) -> Result<(), String> {
    let s = suspension(vm)?;
    let root = std::env::var_os("OW_BL2_ROOT")
        .map(PathBuf::from)
        .ok_or("OW_BL2_ROOT")?;
    let mut loc = ow_core::DetMap::new();
    for pkg in ["GD_Siren_Streaming_SF", "GD_Siren_Skills"] {
        let path = root
            .join("WillowGame")
            .join("Localization")
            .join("INT")
            .join(format!("{pkg}.int"));
        let m = crate::skills::read_localization(&path).map_err(|e| e.to_string())?;
        let _ = writeln!(out, "localization {pkg}.int: {} entries", m.len());
        for (k, v) in m.iter() {
            loc.insert(k.clone(), v.clone());
        }
    }
    let sys = vm.attrs.clone();
    let mut cx = EvalCx::new(vm);
    let def = read_skill(&mut cx, s).map_err(|e| e.to_string())?;
    let (lines, notes) =
        skill_lines(&mut cx, &mut Reader::new(&sys), &def, &loc, 5).map_err(|e| e.to_string())?;
    let mut agree = 0;
    for (g, row) in lines.iter().enumerate().skip(1) {
        let numbers: Vec<&str> = row.iter().map(|(_, n, _)| n.as_str()).collect();
        let want = crate::skills::format_number(0.5 * g as f64, NumberFlagsFloat::get());
        agree += usize::from(numbers.iter().any(|n| *n == want));
        let _ = writeln!(
            out,
            "grade {g}: numbers {numbers:?} (the duration change {want})"
        );
    }
    let _ = writeln!(
        out,
        "info-box number equals the lock duration change at {agree}/5 grades; notes {}",
        notes.len()
    );
    Ok(())
}

/// The number flags of a value shown as seconds with one decimal (the expected text for the info-box check).
struct NumberFlagsFloat;
impl NumberFlagsFloat {
    fn get() -> crate::skills::NumberFlags {
        crate::skills::NumberFlags {
            as_percentage: false,
            rounding_float: true,
            ..Default::default()
        }
    }
}

/// The duration and cooldown chains through `ow-attr` (lane FIX-DESIGNER-RESOLVER, decision 2026-11-16-62): the
/// designer attribute `Att_Phaselock_Duration`, the archetype's `LockDurationFormula` and the cooldown pool's
/// `BaseMaxValue`, from the player's pawn and from its controller. Each answer, or where it stops, is written.
fn part_chains(vm: &mut Vm, out: &mut String) -> Result<(), String> {
    let attr = find(vm, "GD_Siren_Skills.Misc.Att_Phaselock_Duration")?;
    let archetype = find(vm, "GD_Siren_Skills.Phaselock.ActionSkill_Phaselock")?;
    let pool = find(vm, "D_Resourcepools.PlayerPools.ActiveSkillCooldownPool_Siren")?;
    let controller = new_of(vm, "WillowGame.WillowPlayerController")?;
    let pawn = new_of(vm, "WillowGame.WillowPlayerPawn")?;
    set(vm, pawn, "Controller", Value::Object(Some(controller)))?;
    set(vm, controller, "Pawn", Value::Object(Some(pawn)))?;
    let formula = get(vm, archetype, "LockDurationFormula")?;
    let cooldown = get(vm, pool, "BaseMaxValue")?;
    let attrs = vm.attrs.clone();
    for (label, source) in [("pawn", pawn), ("controller", controller)] {
        let value = attrs.value_of(attr, Some(source), None, &mut EvalCx::new(vm));
        let _ = writeln!(
            out,
            "{label}: Att_Phaselock_Duration = {}",
            match value {
                Ok(Some(v)) => g9(v.value),
                Ok(None) => "no context (None)".to_string(),
                Err(e) => format!("stops: {e}"),
            }
        );
        for (name, data) in [("LockDurationFormula", &formula), ("cooldown BaseMaxValue", &cooldown)] {
            let read = ow_attr::eval::init_data(&vm.space, data)
                .and_then(|d| attrs.evaluate_init(&d, Some(source), None, &mut EvalCx::new(vm)));
            let _ = writeln!(
                out,
                "{label}: {name} = {}",
                match read {
                    Ok(v) => g9(v),
                    Err(e) => format!("stops: {e}"),
                }
            );
        }
    }
    Ok(())
}

fn part_duration(vm: &mut Vm, out: &mut String) -> Result<(), String> {
    let attr = find(vm, "GD_Siren_Skills.Misc.Att_Phaselock_Duration")?;
    let archetype = find(vm, "GD_Siren_Skills.Phaselock.ActionSkill_Phaselock")?;
    let controller = new_of(vm, "WillowGame.WillowPlayerController")?;
    let pawn = new_of(vm, "WillowGame.WillowPlayerPawn")?;
    set(vm, pawn, "Controller", Value::Object(Some(controller)))?;
    set(vm, controller, "Pawn", Value::Object(Some(pawn)))?;
    // The game's chain first: the archetype's LockDurationFormula through the pawn's installed
    // EvaluateAttributeInitialization (the designer context resolver, then the pawn's instanced attribute).
    let formula = get(vm, archetype, "LockDurationFormula")?;
    match call(
        vm,
        "Engine.Pawn.EvaluateAttributeInitialization",
        Some(pawn),
        vec![formula],
    ) {
        Ok(v) => {
            let _ = writeln!(
                out,
                "the game's chain: EvaluateAttributeInitialization(LockDurationFormula) = {v:?}"
            );
        }
        Err(e) => {
            let _ = writeln!(out, "the game's chain stops: {e}");
        }
    }
    match call(
        vm,
        "WillowGame.WillowPawn.GetInstancedDesignerAttribute",
        Some(pawn),
        vec![Value::Object(Some(attr))],
    ) {
        Ok(v) => {
            let _ = writeln!(out, "GetInstancedDesignerAttribute = {v:?}");
        }
        Err(e) => {
            let _ = writeln!(out, "the pawn's instanced attribute stops: {e}");
        }
    }
    // The part that runs: SetBaseValue on a fresh instanced attribute, then Suspension's modifier on its Value.
    let instanced = new_of(vm, "WillowGame.InstancedDesignerAttribute")?;
    let base = get(vm, attr, "BaseValue")?;
    call(
        vm,
        "WillowGame.InstancedDesignerAttribute.SetBaseValue",
        Some(instanced),
        vec![base, Value::Object(Some(controller))],
    )?;
    let _ = writeln!(
        out,
        "installed SetBaseValue: Value {} ValueBaseValue {}",
        g9(float(vm, instanced, "Value")?),
        g9(float(vm, instanced, "ValueBaseValue")?)
    );
    let s = suspension(vm)?;
    let effect = get(vm, s, "SkillEffectDefinitions")?
        .elements()
        .and_then(|e| e.first().cloned())
        .ok_or("Suspension has no effect")?;
    let name = vm.space.names.intern("Value");
    let sys = vm.attrs.clone();
    let mut equal = 0;
    for g in 0..=5 {
        let mut modifier = None;
        if g >= 1 {
            let amount =
                crate::action_skill::calculate_modifier_value(vm, &effect, g, Some(controller))
                    .map_err(|e| e.to_string())?;
            let m = new_of(vm, "Core.AttributeModifier")?;
            set(vm, m, "Type", Value::Byte(2))?;
            set(vm, m, "Value", Value::Float(amount))?;
            let mut cx = EvalCx::new(vm);
            let r = sys
                .add_modifier_by_name(instanced, m, name, false, &mut cx)
                .map_err(|e| e.to_string())?;
            let _ = writeln!(
                out,
                "grade {g}: Suspension modifier {} added {r:?}",
                g9(amount)
            );
            modifier = Some(m);
        }
        let value = float(vm, instanced, "Value")?;
        let same = value.to_bits() == expected(g).to_bits();
        equal += usize::from(same);
        let _ = writeln!(
            out,
            "grade {g}: lock duration {} (5 + 0.5 g: {same})",
            g9(value)
        );
        if let Some(m) = modifier {
            let mut cx = EvalCx::new(vm);
            sys.remove_modifier_by_name(instanced, m, name, false, &mut cx)
                .map_err(|e| e.to_string())?;
        }
    }
    let _ = writeln!(
        out,
        "lock duration bit-equal to 5 + 0.5 g at {equal}/6 grades (what this checks: the installed data give base 5 and Suspension 0.5 g, through the installed SetBaseValue and the stack; the stack rule is not checked against anything but itself)"
    );
    Ok(())
}

fn part_schedule(vm: &mut Vm, out: &mut String) -> Result<(), String> {
    let a = find(vm, "GD_Siren_Skills.Phaselock.ActionSkill_Phaselock")?;
    let (lift, fade, buffer) = (
        float(vm, a, "LiftDuration")?,
        float(vm, a, "LockFadeOutTime")?,
        float(vm, a, "ReleaseBufferTime")?,
    );
    let _ = writeln!(
        out,
        "archetype: LiftDuration {} LockFadeOutTime {} ReleaseBufferTime {}",
        g9(lift),
        g9(fade),
        g9(buffer)
    );
    for g in 0..=5 {
        let s = super::Schedule::new(lift, expected(g), 1.0, fade, buffer);
        let _ = writeln!(
            out,
            "grade {g}: lock at {} outro {} release {} end {}",
            g9(s.lock_at),
            g9(s.outro_at),
            g9(s.release_at),
            g9(s.end_at)
        );
    }
    Ok(())
}

fn part_lift(vm: &mut Vm, out: &mut String) -> Result<(), String> {
    let a = find(vm, "GD_Siren_Skills.Phaselock.ActionSkill_Phaselock")?;
    let wi = {
        let actors = ow_world::actors::actors_of(vm).map_err(|e| e.to_string())?;

        actors.borrow().world_info().ok_or("no world info")?
    };
    set(vm, a, "WorldInfo", Value::Object(Some(wi)))?;
    let (start, end) = ([0.0f32, 0.0, 0.0], [0.0f32, 0.0, 200.0]);
    let sv = vector(vm, start)?;
    let ev = vector(vm, end)?;
    set(vm, a, "LiftStartLocation", sv)?;
    set(vm, a, "LiftEndLocation", ev.clone())?;
    set(vm, a, "LiftStartTime", Value::Float(0.0))?;
    set(vm, a, "SkillStartTime", Value::Float(0.0))?;
    let lift = float(vm, a, "LiftDuration")?;
    let (amp, freq) = (
        float(vm, a, "LiftBobAmplitude")?,
        float(vm, a, "LiftBobFrequency")?,
    );
    let _ = writeln!(
        out,
        "archetype: LiftBobAmplitude {} LiftBobFrequency {}",
        g9(amp),
        g9(freq)
    );
    // The lift: GetLiftLocation(t) at 60 frames per second up to the lift's end (where it stops is written, and the
    // bob still runs).
    let mut zs = Vec::new();
    let mut t = 0.0f32;
    let mut lift_stop = None;
    while t <= lift {
        // The script's first part reads its LiftTime argument, the second part the world's TimeSeconds (lane
        // FIX-PHASELOCK-CAST part A): both advance together here, as they do from a lift started at time 0.
        set(vm, wi, "TimeSeconds", Value::Float(t))?;
        match call(
            vm,
            "WillowGame.LiftActionSkill.GetLiftLocation",
            Some(a),
            vec![Value::Float(t)],
        ) {
            Ok(v) => zs.push((t, axes(vm, &v)?[2])),
            Err(e) => {
                lift_stop = Some(e);
                break;
            }
        }
        t += 1.0 / 60.0;
    }
    match lift_stop {
        Some(e) => {
            let _ = writeln!(out, "lift: stops after {} frames: {e}", zs.len());
        }
        None => {
            let monotonic = zs.windows(2).all(|w| w[1].1 >= w[0].1);
            let _ = writeln!(
                out,
                "lift: {} frames, Z from {} to {}, never down: {monotonic}",
                zs.len(),
                g9(zs[0].1),
                g9(zs[zs.len() - 1].1)
            );
        }
    }
    // The bob, seeded at the lift's end: GetBobLocation(dt) with WorldInfo.TimeSeconds advanced, against the oracle.
    set(vm, a, "PrevBobLocation", ev)?;
    let mut prev = [f64::from(end[0]), f64::from(end[1]), f64::from(end[2])];
    let mut now = lift;
    let (mut frames, mut equal, mut worst) = (0, 0, 0.0f64);
    let mut peak = f32::MIN;
    // An uneven frame sequence (invented), 4.5 s of it.
    let steps = [1.0f32 / 60.0, 1.0 / 30.0, 1.0 / 144.0, 1.0 / 45.0];
    let mut i = 0usize;
    while now < lift + 4.5 {
        let dt = steps[i % steps.len()];
        i += 1;
        now += dt;
        set(vm, wi, "TimeSeconds", Value::Float(now))?;
        let v = call(
            vm,
            "WillowGame.LiftActionSkill.GetBobLocation",
            Some(a),
            vec![Value::Float(dt)],
        )?;
        let got = axes(vm, &v)?;
        let target = [
            f64::from(end[0]),
            f64::from(end[1]),
            f64::from(super::oracle::bob_target_z(end[2], amp, freq, now - 0.0)),
        ];
        prev = super::oracle::vinterp_to(prev, target, f64::from(dt), 1.0);
        frames += 1;
        let same = (0..3).all(|k| f64::from(got[k]).to_bits() == prev[k].to_bits());

        equal += usize::from(same);
        worst = worst.max((f64::from(got[2]) - prev[2]).abs());
        peak = peak.max(got[2] - end[2]);
    }
    // The script's own target: with DeltaTime x speed >= 1, VInterpTo returns its target exactly.
    let (mut targets, mut targets_equal) = (0, 0);
    let mut t = lift;
    while t < lift + 4.0 {
        t += 0.037;
        set(vm, wi, "TimeSeconds", Value::Float(t))?;
        let v = call(
            vm,
            "WillowGame.LiftActionSkill.GetBobLocation",
            Some(a),
            vec![Value::Float(2.0)],
        )?;
        let z = axes(vm, &v)?[2];
        targets += 1;
        let rule = super::oracle::bob_target_z(end[2], amp, freq, t);
        targets_equal += usize::from(z.to_bits() == rule.to_bits());
    }
    let _ = writeln!(
        out,
        "bob target before the smoothing (independent of VInterpTo): the script equals the record's formula at {targets_equal}/{targets} times"
    );
    let _ = writeln!(
        out,
        "bob: {frames} frames, the script with ow-vm's VInterpTo bit-equal to the record's rule restated in this test (oracle::vinterp_to, written apart from it) at {equal}/{frames}, largest Z difference {}, highest point {} above the lift's end; the rule itself was matched live to RMS 0.001 uu, and VInterpTo's precision is UNVERIFIED",
        ow_core::fmt::fmt_g(worst, 6),
        g9(peak)
    );
    Ok(())
}

fn part_cast(vm: &mut Vm, out: &mut String) -> Result<(), String> {
    let a = find(vm, "GD_Siren_Skills.Phaselock.ActionSkill_Phaselock")?;
    // The cast's actors as the world's spawn makes them (each with its own components in Off).
    let controller = spawned_of(vm, "WillowGame.WillowPlayerController")?;
    let pawn = spawned_of(vm, "WillowGame.WillowPlayerPawn")?;
    let target = spawned_of(vm, "WillowGame.WillowAIPawn")?;
    let r = call(
        vm,
        "WillowGame.LiftActionSkill.OnActionSkillStarted",
        Some(a),
        vec![
            Value::Object(Some(pawn)),
            Value::Object(Some(controller)),
            Value::Object(Some(target)),
        ],
    );
    let _ = writeln!(
        out,
        "OnActionSkillStarted: {}",
        match r {
            Ok(v) => format!("returned {v:?}"),
            Err(e) => format!("stops: {e}"),
        }
    );
    Ok(())
}

#[test]
#[ignore = "needs the installed game (OW_BL2_ROOT)"]
fn installed_game_maya_skills_duration_and_lift() {
    let Some(root) = std::env::var_os("OW_BL2_ROOT") else {
        eprintln!("OW_BL2_ROOT is not set: skipped");
        return;
    };
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/w6-skills");
    std::fs::create_dir_all(&dir).unwrap();
    // Off first (the file name W6-SKILLS wrote), then Reference (lane FIX-DESIGNER-RESOLVER).
    for (mode, file) in [
        (ParityMode::Off, "maya_installed.txt"),
        (ParityMode::Reference, "maya_installed_reference.txt"),
    ] {
        let root = PathBuf::from(&root);
        let text = ow_vm::interp::on_vm_stack(move || run_all(root, mode)).unwrap();
        std::fs::write(dir.join(file), &text).unwrap();
        println!("{text}");
    }
}
