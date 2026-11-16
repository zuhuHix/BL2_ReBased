//! Owner: lane W6-MISSION. The tracker rules on invented packages (AGENTS.md rule 9). AI-assisted (Claude Code).
//!
//! The packages re-express `tests/behavior_test.py`'s `build_mission` (the toy mission of its section 6, and the
//! mission vocabulary of its `build_willowgame`) with `ow_package::testkit`, with every object property naming the toy
//! `Core.Object`, as the Rust reflection needs; the extra objects (an optional objective, a lent weapon, an objective
//! dependency, a sequence-state behavior, a dialog behavior, a set of another class) are invented for the rules that
//! test does not reach. Every expected value is written by hand from the rule.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::registry::AttributeSystem;
use ow_core::{DetMap, DetSet, Missing, PackageName, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_package::testkit::PackageBuilder;
use ow_package::testkit::reflect::{Declarations, PropSpec};
use ow_vm::api::{NativePack, Vm};

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);

// ---------------------------------------------------------------------------------------------------------------
// The invented install
// ---------------------------------------------------------------------------------------------------------------

const VARIABLE_TYPES: [&str; 3] = ["BVAR_None", "BVAR_Object", "BVAR_Int"];
const LINK_TYPES: [&str; 3] = ["BVARLINK_Unknown", "BVARLINK_Input", "BVARLINK_Output"];

fn object_import(p: &mut PackageBuilder) -> i32 {
    let core = p.add_import("Core", "Package", 0, "Core");
    p.add_import("Core", "Class", core, "Object")
}

fn core() -> Vec<u8> {
    let mut d = Declarations::new(&[]);
    let object = d.cls("Object", 0);
    d.cls("Package", object);
    d.build()
}

fn engine() -> Vec<u8> {
    let mut d = Declarations::new(&[]);
    let object = object_import(&mut d.p);
    let targetable = d.cls("ITargetable", object);
    d.enum_(targetable, "EChangeStatus", &["CHANGE_Toggle", "CHANGE_Enable", "CHANGE_Disable"]);
    d.build()
}

/// The provider vocabulary of `ow-behavior`'s fixtures, and two behavior classes of the framework.
fn gearbox() -> Vec<u8> {
    let mut d = Declarations::new(&["Int", "Float", "Bool", "Name", "Object", "Array", "Struct", "Byte"]);
    let object = object_import(&mut d.p);
    let bpd = d.cls("BehaviorProviderDefinition", object);
    let var_types = d.enum_(bpd, "EBehaviorVariableType", &VARIABLE_TYPES);
    let link_types = d.enum_(bpd, "EBehaviorVariableLinkType", &LINK_TYPES);
    let sub = d.struct_("SubarrayData", None);
    d.prop("Int", sub, "ArrayIndexAndLength", 0);
    let defaults = {
        let mut t = d.p.bool_tag("bEnabled", true);
        t.extend_from_slice(&d.none);
        t
    };
    let user = d.struct_("BehaviorEventUserData", Some(&defaults));
    d.prop("Name", user, "EventName", 0);
    d.prop("Bool", user, "bEnabled", 0);
    d.prop("Int", user, "MaxTriggerCount", 0);
    d.prop("Float", user, "ReTriggerDelay", 0);
    let event = d.struct_("BehaviorEventData2", None);
    d.prop("Struct", event, "UserData", user);
    d.prop("Struct", event, "OutputVariables", sub);
    d.prop("Struct", event, "OutputLinks", sub);
    let data = d.struct_("BehaviorData2", None);
    d.prop("Object", data, "Behavior", object);
    d.prop("Struct", data, "LinkedVariables", sub);
    d.prop("Struct", data, "OutputLinks", sub);
    let link = d.struct_("BehaviorOutputLinkData", None);
    d.prop("Int", link, "LinkIdAndLinkedBehavior", 0);
    d.prop("Float", link, "ActivateDelay", 0);
    let var = d.struct_("BehaviorVariableData", None);
    d.prop("Name", var, "Name", 0);
    d.prop("Byte", var, "Type", var_types);
    let vlink = d.struct_("BehaviorVariableLinkData2", None);
    d.prop("Name", vlink, "PropertyName", 0);
    d.prop("Byte", vlink, "VariableLinkType", link_types);
    d.prop("Byte", vlink, "ConnectionIndex", 0);
    d.prop("Struct", vlink, "LinkedVariables", sub);
    let seq = d.struct_("BehaviorSequenceData", None);
    d.prop("Name", seq, "BehaviorSequenceName", 0);
    d.prop("Bool", seq, "bEnabledOnSpawn", 0);
    d.prop("Bool", seq, "bSequenceEnabledMutex", 0);
    d.prop("Object", seq, "CustomEnableCondition", object);
    d.array_of(seq, "EventData2", "Struct", event);
    d.array_of(seq, "BehaviorData2", "Struct", data);
    d.array_of(seq, "VariableData", "Struct", var);
    d.array_of(seq, "ConsolidatedOutputLinkData", "Struct", link);
    d.array_of(seq, "ConsolidatedVariableLinkData", "Struct", vlink);
    d.array_of(seq, "ConsolidatedLinkedVariables", "Int", 0);
    d.array_of(bpd, "BehaviorSequences", "Struct", seq);
    // Behavior_ChangeRemoteBehaviorSequenceState: the path's name slots are a static array of three names.
    let path = d.struct_("NameBasedObjectPath", None);
    let mut slots = PropSpec::new("Name", path, "PathComponentNames");
    slots.dim = 3;
    d.prop_with(&slots);
    let change = d.cls("Behavior_ChangeRemoteBehaviorSequenceState", object);
    d.prop("Struct", change, "ProviderDefinitionPathName", path);
    d.prop("Name", change, "SequenceName", 0);
    d.prop("Byte", change, "Action", 0);
    d.cls("Behavior_TriggerDialogEvent", object);
    d.build()
}

/// The mission vocabulary of `behavior_test.py`'s `build_willowgame`, typed, plus the fields the tracker reads that
/// it does not declare.
fn willowgame() -> Vec<u8> {
    let mut d = Declarations::new(&["Float", "Object", "Struct", "Bool", "Array", "Int", "Name", "Str", "Byte"]);
    let object = object_import(&mut d.p);
    let mission = d.cls("MissionDefinition", object);
    d.array_of(mission, "ObjectiveSetDefs", "Object", object);
    d.prop("Object", mission, "InitialObjectiveSet", object);
    d.prop("Object", mission, "BehaviorProvider", object);
    d.prop("Bool", mission, "bActivateInitialObjectiveSet", 0);
    d.prop("Str", mission, "MissionName", 0);
    d.prop("Object", mission, "MissionWeapon", object);
    d.array_of(mission, "Dependencies", "Object", object);
    d.prop("Bool", mission, "bPlotCritical", 0);
    d.prop("Bool", mission, "bRepeatable", 0);
    let dependency = d.struct_("ObjectiveDependencyData", None);
    d.prop("Object", dependency, "Objective", object);
    d.prop("Byte", dependency, "Status", 0);
    d.prop("Struct", mission, "ObjectiveDependency", dependency);
    let set = d.cls("MissionObjectiveSetDefinition", object);
    d.array_of(set, "ObjectiveDefinitions", "Object", object);
    d.prop("Object", set, "NextSet", object);
    d.prop("Bool", set, "bCanCompleteMission", 0);
    d.prop("Bool", set, "bAutoEnableNextSet", 0);
    // A set class the tracker does not activate (collection sets are not modelled).
    d.cls("MissionObjectiveSetCollectionDefinition", object);
    let objective = d.cls("MissionObjectiveDefinition", object);
    d.prop("Int", objective, "ObjectiveCount", 0);
    d.prop("Bool", objective, "bRememberItemsWithinObjective", 0);
    d.prop("Bool", objective, "bObjectiveIsOptional", 0);
    d.prop("Str", objective, "ProgressMessage", 0);
    let advance = d.cls("Behavior_AdvanceObjectiveSet", object);
    d.prop("Object", advance, "ObjectiveSetToAdvanceTo", object);
    let remote = d.cls("Behavior_MissionRemoteEvent", object);
    d.prop("Name", remote, "EventName", 0);
    let update = d.cls("Behavior_UpdateMissionObjective", object);
    d.prop("Object", update, "MissionObjective", object);
    let weapon = d.cls("MissionWeaponBalanceDefinition", object);
    d.prop("Object", weapon, "MissionObjective", object);
    d.build()
}

/// One behavior tag of the mission's provider.
#[derive(Clone)]
enum Tag {
    Name(&'static str, &'static str),
    /// An object tag naming an objective or a set of the mission by name.
    Obj(&'static str, &'static str),
    /// `ProviderDefinitionPathName { PathComponentNames = names }`, `SequenceName`, `Action` (a byte).
    Change(&'static [&'static str], &'static str, u8),
}

struct Beh {
    name: &'static str,
    package: &'static str,
    class: &'static str,
    tags: Vec<Tag>,
}

fn beh(name: &'static str, class: &'static str, tags: Vec<Tag>) -> Beh {
    let package = if class.starts_with("Behavior_ChangeRemote") || class == "Behavior_TriggerDialogEvent" {
        "GearboxFramework"
    } else {
        "WillowGame"
    };
    Beh {
        name,
        package,
        class,
        tags,
    }
}

fn remote(event: &'static str) -> Beh {
    beh(Box::leak(format!("R{event}").into_boxed_str()), "Behavior_MissionRemoteEvent", vec![Tag::Name("EventName", event)])
}

struct Obj {
    name: &'static str,
    count: i32,
    mask: bool,
    optional: bool,
    message: &'static str,
}

fn obj(name: &'static str, count: i32) -> Obj {
    Obj {
        name,
        count,
        mask: false,
        optional: false,
        message: "",
    }
}

struct Set {
    name: &'static str,
    class: &'static str,
    members: Vec<&'static str>,
    next: Option<&'static str>,
    can_complete: bool,
    auto: bool,
}

fn set(name: &'static str, members: Vec<&'static str>, next: Option<&'static str>, can_complete: bool, auto: bool) -> Set {
    Set {
        name,
        class: "MissionObjectiveSetDefinition",
        members,
        next,
        can_complete,
        auto,
    }
}

/// A mission package: `ToyMission` with its objectives, sets and provider (`ToyMissionBpd`, one sequence `Main`).
struct Mission {
    objectives: Vec<Obj>,
    sets: Vec<Set>,
    initial: &'static str,
    activate_initial: bool,
    behaviors: Vec<Beh>,
    /// Event name -> (link id, behavior name).
    events: Vec<(&'static str, Vec<(i32, &'static str)>)>,
    weapon_objective: Option<&'static str>,
    dependency: Option<(&'static str, u8)>,
    dependencies: Vec<&'static str>,
    provider: bool,
    repeatable: bool,
    plot_critical: bool,
}

/// `behavior_test.py`'s `build_mission`: SetA {Count3 (3), Echo (3)} -> NextSet SetB {Mask (bit mask, 2), auto-enable}
/// -> SetC {Last, bCanCompleteMission}. Provider events (link id -> behavior): Default 1/7/9/10/12 -> remote events,
/// SetA 4/5, SetB 4, SetC 4 -> remote events, Count3 3 -> UpdateMissionObjective(Echo), Count3 2 ->
/// AdvanceObjectiveSet(SetC, not the NextSet) and a remote event, custom GoB 0 -> AdvanceObjectiveSet(SetB).
fn toy_mission() -> Mission {
    let mut behaviors: Vec<Beh> = [
        "Activated", "Ready", "Done", "Kick", "Load", "SetAActive", "SetADone", "SetBActive", "SetCActive", "Count3Done",
    ]
    .into_iter()
    .map(remote)
    .collect();
    behaviors.push(beh("UEcho", "Behavior_UpdateMissionObjective", vec![Tag::Obj("MissionObjective", "Echo")]));
    behaviors.push(beh("AdvC", "Behavior_AdvanceObjectiveSet", vec![Tag::Obj("ObjectiveSetToAdvanceTo", "SetC")]));
    behaviors.push(beh("AdvB", "Behavior_AdvanceObjectiveSet", vec![Tag::Obj("ObjectiveSetToAdvanceTo", "SetB")]));
    let mut mask = obj("Mask", 2);
    mask.mask = true;
    Mission {
        objectives: vec![obj("Count3", 3), obj("Echo", 3), mask, obj("Last", 1)],
        sets: vec![
            set("SetA", vec!["Count3", "Echo"], Some("SetB"), false, false),
            set("SetB", vec!["Mask"], Some("SetC"), false, true),
            set("SetC", vec!["Last"], None, true, false),
        ],
        initial: "SetA",
        activate_initial: true,
        behaviors,
        events: vec![
            ("Default", vec![(1, "RLoad"), (7, "RActivated"), (9, "RReady"), (10, "RDone"), (12, "RKick")]),
            ("SetA", vec![(4, "RSetAActive"), (5, "RSetADone")]),
            ("SetB", vec![(4, "RSetBActive")]),
            ("SetC", vec![(4, "RSetCActive")]),
            ("Count3", vec![(3, "UEcho"), (2, "AdvC"), (2, "RCount3Done")]),
            ("GoB", vec![(0, "AdvB")]),
        ],
        weapon_objective: None,
        dependency: None,
        dependencies: Vec::new(),
        provider: true,
        repeatable: false,
        plot_critical: false,
    }
}

fn packed(first: usize, length: usize) -> i32 {
    // The fixtures stay far below 2^15 entries.
    ((first << 16) | length) as i32
}

fn mission_package(m: &Mission) -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    let none = p.none();
    let willow = p.add_import("Core", "Package", 0, "WillowGame");
    let gearbox = p.add_import("Core", "Package", 0, "GearboxFramework");
    let mut classes: Vec<(String, i32)> = Vec::new();
    let mut class = |p: &mut PackageBuilder, package: &'static str, name: &str| -> i32 {
        let key = format!("{package}.{name}");
        if let Some(&(_, r)) = classes.iter().find(|(k, _)| *k == key) {
            return r;
        }
        let outer = if package == "GearboxFramework" { gearbox } else { willow };
        let r = p.add_import("Core", "Class", outer, name);
        classes.push((key, r));
        r
    };
    let c = class(&mut p, "WillowGame", "MissionDefinition");
    let mission = p.add_export(c, "ToyMission", Vec::new());
    let mut objectives = Vec::new();
    for o in &m.objectives {
        let c = class(&mut p, "WillowGame", "MissionObjectiveDefinition");
        let mut payload = 0i32.to_le_bytes().to_vec();
        payload.extend(p.int_tag("ObjectiveCount", o.count));
        if o.mask {
            payload.extend(p.bool_tag("bRememberItemsWithinObjective", true));
        }
        if o.optional {
            payload.extend(p.bool_tag("bObjectiveIsOptional", true));
        }
        if !o.message.is_empty() {
            payload.extend(p.str_tag("ProgressMessage", o.message));
        }
        payload.extend_from_slice(&none);
        let r = p.add_export(c, o.name, payload);
        p.set_outer(r, mission);
        objectives.push((o.name, r));
    }
    let mut sets = Vec::new();
    for s in &m.sets {
        let c = class(&mut p, "WillowGame", s.class);
        let r = p.add_export(c, s.name, Vec::new());
        p.set_outer(r, mission);
        sets.push((s.name, r));
    }
    let lookup = |name: &str| -> i32 {
        objectives
            .iter()
            .chain(sets.iter())
            .find(|(n, _)| *n == name)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("no fixture object {name}"))
    };
    for (s, &(_, r)) in m.sets.iter().zip(&sets) {
        let mut payload = 0i32.to_le_bytes().to_vec();
        let mut members = Vec::new();
        for member in &s.members {
            members.extend(lookup(member).to_le_bytes());
        }
        payload.extend(p.array_tag("ObjectiveDefinitions", s.members.len() as i32, &members));
        if let Some(next) = s.next {
            payload.extend(p.object_tag("NextSet", lookup(next)));
        }
        payload.extend(p.bool_tag("bCanCompleteMission", s.can_complete));
        if s.auto {
            payload.extend(p.bool_tag("bAutoEnableNextSet", true));
        }
        payload.extend_from_slice(&none);
        p.set_payload(r, payload);
    }
    let mut mission_tags = 0i32.to_le_bytes().to_vec();
    let mut set_refs = Vec::new();
    for (_, r) in &sets {
        set_refs.extend(r.to_le_bytes());
    }
    mission_tags.extend(p.array_tag("ObjectiveSetDefs", sets.len() as i32, &set_refs));
    mission_tags.extend(p.object_tag("InitialObjectiveSet", lookup(m.initial)));
    mission_tags.extend(p.bool_tag("bActivateInitialObjectiveSet", m.activate_initial));
    mission_tags.extend(p.str_tag("MissionName", "Toy mission"));
    if m.repeatable {
        mission_tags.extend(p.bool_tag("bRepeatable", true));
    }
    if m.plot_critical {
        mission_tags.extend(p.bool_tag("bPlotCritical", true));
    }
    if let Some(objective) = m.weapon_objective {
        let c = class(&mut p, "WillowGame", "MissionWeaponBalanceDefinition");
        let mut payload = 0i32.to_le_bytes().to_vec();
        payload.extend(p.object_tag("MissionObjective", lookup(objective)));
        payload.extend_from_slice(&none);
        let w = p.add_export(c, "ToyWeapon", payload);
        mission_tags.extend(p.object_tag("MissionWeapon", w));
    }
    if let Some((objective, status)) = m.dependency {
        let mut body = p.object_tag("Objective", lookup(objective));
        body.extend(p.byte_tag("Status", status));
        body.extend_from_slice(&none);
        mission_tags.extend(p.struct_tag("ObjectiveDependency", "ObjectiveDependencyData", &body));
    }
    if !m.dependencies.is_empty() {
        let mut refs = Vec::new();
        for dep in &m.dependencies {
            let c = class(&mut p, "WillowGame", "MissionDefinition");
            let r = p.add_export(c, dep, {
                let mut x = 0i32.to_le_bytes().to_vec();
                x.extend_from_slice(&none);
                x
            });
            refs.extend(r.to_le_bytes());
        }
        mission_tags.extend(p.array_tag("Dependencies", m.dependencies.len() as i32, &refs));
    }
    if m.provider {
        let c = class(&mut p, "GearboxFramework", "BehaviorProviderDefinition");
        let provider = p.add_export(c, "ToyMissionBpd", Vec::new());
        p.set_outer(provider, mission);
        let mut refs = Vec::new();
        for b in &m.behaviors {
            let c = class(&mut p, b.package, b.class);
            let r = p.add_export(c, b.name, Vec::new());
            p.set_outer(r, provider);
            refs.push((b.name, r));
        }
        for (b, &(_, r)) in m.behaviors.iter().zip(&refs) {
            let mut payload = 0i32.to_le_bytes().to_vec();
            for tag in &b.tags {
                let bytes = match tag {
                    Tag::Name(prop, value) => p.name_tag(prop, value),
                    Tag::Obj(prop, target) => p.object_tag(prop, lookup(target)),
                    Tag::Change(names, sequence, action) => {
                        let mut body = Vec::new();
                        for (i, n) in names.iter().enumerate() {
                            let name = p.fname("PathComponentNames", 0);
                            let kind = p.fname("NameProperty", 0);
                            body.extend(name);
                            body.extend(kind);
                            body.extend(8i32.to_le_bytes());
                            body.extend((i as i32).to_le_bytes());
                            body.extend(p.fname(n, 0));
                        }
                        body.extend_from_slice(&none);
                        let mut t = p.struct_tag("ProviderDefinitionPathName", "NameBasedObjectPath", &body);
                        t.extend(p.name_tag("SequenceName", sequence));
                        t.extend(p.byte_tag("Action", *action));
                        t
                    }
                };
                payload.extend(bytes);
            }
            payload.extend_from_slice(&none);
            p.set_payload(r, payload);
        }
        let index_of = |name: &str| refs.iter().position(|(n, _)| *n == name).expect("behavior") as i32;
        let sub = |p: &mut PackageBuilder, name: &str, first: usize, length: usize| {
            let mut body = p.int_tag("ArrayIndexAndLength", packed(first, length));
            body.extend_from_slice(&none);
            p.struct_tag(name, "SubarrayData", &body)
        };
        let mut links: Vec<(i32, i32)> = Vec::new();
        let mut event_data = Vec::new();
        for (name, targets) in &m.events {
            let mut user = p.name_tag("EventName", name);
            user.extend_from_slice(&none);
            event_data.extend(p.struct_tag("UserData", "BehaviorEventUserData", &user));
            event_data.extend(sub(&mut p, "OutputVariables", 0, 0));
            event_data.extend(sub(&mut p, "OutputLinks", links.len(), targets.len()));
            event_data.extend_from_slice(&none);
            for (id, target) in targets {
                links.push((*id, index_of(target)));
            }
        }
        let mut behavior_data = Vec::new();
        for (_, r) in &refs {
            behavior_data.extend(p.object_tag("Behavior", *r));
            behavior_data.extend(sub(&mut p, "LinkedVariables", 0, 0));
            behavior_data.extend(sub(&mut p, "OutputLinks", 0, 0));
            behavior_data.extend_from_slice(&none);
        }
        let mut link_data = Vec::new();
        for (id, target) in &links {
            link_data.extend(p.int_tag("LinkIdAndLinkedBehavior", target | (id << 24)));
            link_data.extend(p.float_tag("ActivateDelay", 0.0));
            link_data.extend_from_slice(&none);
        }
        let mut seq = p.name_tag("BehaviorSequenceName", "Main");
        seq.extend(p.bool_tag("bEnabledOnSpawn", true));
        seq.extend(p.array_tag("EventData2", m.events.len() as i32, &event_data));
        seq.extend(p.array_tag("BehaviorData2", refs.len() as i32, &behavior_data));
        seq.extend(p.array_tag("ConsolidatedOutputLinkData", links.len() as i32, &link_data));
        seq.extend_from_slice(&none);
        let mut payload = 0i32.to_le_bytes().to_vec();
        payload.extend(p.array_tag("BehaviorSequences", 1, &seq));
        payload.extend_from_slice(&none);
        p.set_payload(provider, payload);
        mission_tags.extend(p.object_tag("BehaviorProvider", provider));
    }
    mission_tags.extend_from_slice(&none);
    p.set_payload(mission, mission_tags);
    p.build()
}

/// A VM over the invented install with mission `m` in `TestMission.upk`.
struct Env {
    vm: Vm,
    dir: PathBuf,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Env {
    fn new(m: &Mission) -> Env {
        Env::with_mode(m, ParityMode::Off)
    }

    fn with_mode(m: &Mission, mode: ParityMode) -> Env {
        let dir = std::env::temp_dir().join(format!(
            "ow-mission-tracker-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let files: Vec<(String, Vec<u8>)> = vec![
            ("Core.upk".into(), core()),
            ("Engine.upk".into(), engine()),
            ("GearboxFramework.upk".into(), gearbox()),
            ("WillowGame.upk".into(), willowgame()),
            ("TestMission.upk".into(), mission_package(m)),
        ];
        let borrowed: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
        let defs =
            DefStore::from_synthetic(&dir, &borrowed, IdentityPolicy::FirstInScanOrder, mode).expect("synthetic store");
        let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
        ow_behavior::handlers::Pack.register(&mut vm).expect("world handlers");
        Env { vm, dir }
    }

    fn mission(&mut self) -> MissionSystem {
        self.try_mission().expect("mission")
    }

    fn try_mission(&mut self) -> Result<MissionSystem, MissionError> {
        let name = PackageName::parse("TestMission").unwrap();
        MissionSystem::new(&mut self.vm, &name, "ToyMission")
    }
}

/// The effects as `behavior_test.py` writes them.
fn short(effects: &[Effect]) -> Vec<String> {
    effects
        .iter()
        .map(|e| match e.kind {
            EffectKind::RemoteEvent => format!("R:{}", e.a),
            EffectKind::ObjectiveUpdated => format!("U:{}={}", e.a, e.b),
            EffectKind::ObjectiveComplete => format!("C:{}", e.a),
            EffectKind::ObjectiveSetActive => format!("S:{}", e.a),
            EffectKind::StatusChanged => format!("status:{}", e.a),
            other => other.json_name().to_string(),
        })
        .collect()
}

fn leaf(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or("")
}

// ---------------------------------------------------------------------------------------------------------------
// The rules
// ---------------------------------------------------------------------------------------------------------------

/// `behavior_test.py` section 6 (NATIVE_MISSION_DISPATCH.md acceptance test 6), step by step: B1 to B4 and B7 on the
/// toy mission.
#[test]
fn the_toy_mission_runs_as_behavior_test_section_six() {
    let mut env = Env::new(&toy_mission());
    let m = env.mission();
    let vm = &mut env.vm;
    let none = DetSet::new();
    type Step<'a> = (&'a str, bool, &'a str, Vec<&'a str>);
    let want: Vec<Step<'_>> = vec![
        ("accept", true, "SetA", vec!["status:Active", "R:Activated", "S:SetA", "R:SetAActive"]),
        ("kickoff", true, "SetA", vec!["R:Kick"]),
        ("obj:Count3", true, "SetA", vec!["U:Count3=1", "U:Echo=1"]),
        ("obj:Count3", true, "SetA", vec!["U:Count3=2", "U:Echo=2"]),
        ("obj:Count3", true, "SetA", vec!["U:Count3=3", "C:Count3", "R:Count3Done", "U:Echo=3", "C:Echo", "R:SetADone"]),
        ("obj:Count3", false, "SetA", vec![]),
        ("custom:GoB", true, "SetB", vec!["S:SetB", "R:SetBActive"]),
        ("obj:Mask:1", true, "SetB", vec!["U:Mask=1"]),
        ("obj:Mask:1", true, "SetB", vec!["U:Mask=1"]),
        ("obj:Mask:0", false, "SetB", vec![]),
        ("obj:Mask:2", true, "SetC", vec!["U:Mask=2", "C:Mask", "S:SetC", "R:SetCActive"]),
        ("obj:Last", true, "SetC", vec!["U:Last=1", "C:Last", "status:ReadyToTurnIn", "R:Ready"]),
        ("turnin", true, "", vec!["status:Complete", "R:Done", "reward"]),
    ];
    for (step, ok, set, effects) in want {
        let got = match step {
            "accept" => m.accept(vm, &none).unwrap(),
            "kickoff" => m.kickoff(vm, false).unwrap(),
            "turnin" => m.turn_in_mission(vm).unwrap(),
            "custom:GoB" => m.custom_event(vm, "GoB").unwrap(),
            other => {
                let rest = other.strip_prefix("obj:").unwrap();
                let (name, bit) = rest.split_once(':').map_or((rest, 0), |(n, b)| (n, b.parse().unwrap()));
                m.update_objective(vm, name, bit).unwrap()
            }
        };
        assert_eq!(got, ok, "{step}");
        assert_eq!(leaf(&m.active_set()), set, "{step}");
        assert_eq!(short(&m.drain()), effects, "{step}");
    }
    assert!(m.errors().is_empty(), "{:?}", m.errors());
    assert_eq!(m.status(), Status::Complete);
}

/// B4: the status transitions are refused out of order; turn-in only from ReadyToTurnIn; the custom event is refused
/// once Complete.
#[test]
fn status_changes_only_in_order() {
    let mut env = Env::new(&toy_mission());
    let m = env.mission();
    let vm = &mut env.vm;
    assert!(!m.turn_in_mission(vm).unwrap(), "turn-in before acceptance");
    assert!(!m.kickoff(vm, false).unwrap(), "kickoff needs Active");
    assert!(m.accept(vm, &DetSet::new()).unwrap());
    assert!(!m.accept(vm, &DetSet::new()).unwrap(), "a second accept");
    assert!(!m.turn_in_mission(vm).unwrap(), "turn-in while Active");
    m.drain();
    assert_eq!(m.status().number(), 1);
    assert_eq!(Status::ReadyToTurnIn.number(), 3);
    assert_eq!(Status::Complete.number(), 4);
    assert_eq!(Status::ReadyToTurnIn.index(), 2);
}

/// The tracker tick consumes the pending kickoff record once: `PlayKickoff` (id 12) because it came from the
/// activation, the hook first; a later tick fires nothing.
#[test]
fn the_tick_plays_the_pending_kickoff_once() {
    let mut env = Env::new(&toy_mission());
    let m = env.mission();
    let calls = Rc::new(RefCell::new(0));
    let c = calls.clone();
    m.set_kickoff_hook(Some(Rc::new(move |_: &mut Vm| *c.borrow_mut() += 1)));
    let vm = &mut env.vm;
    m.accept(vm, &DetSet::new()).unwrap();
    assert!(m.kickoff_pending());
    m.drain();
    m.tick(vm, 0.5).unwrap();
    assert!(!m.kickoff_pending());
    let effects = m.drain();
    assert_eq!(short(&effects), ["R:Kick"]);
    assert_eq!(effects[0].time, 0.5, "effects carry the tracker's time");
    m.tick(vm, 0.5).unwrap();
    assert!(m.drain().is_empty());
    assert_eq!(*calls.borrow(), 1);
}

/// `kickoff(true)` is `PlayKickoffDialogOnly`: id 13, which the toy links to nothing.
#[test]
fn kickoff_dialog_only_fires_id_thirteen() {
    let mut toy = toy_mission();
    toy.behaviors.push(remote("DialogOnly"));
    toy.events[0].1.push((13, "RDialogOnly"));
    let mut env = Env::new(&toy);
    let m = env.mission();
    let vm = &mut env.vm;
    m.accept(vm, &DetSet::new()).unwrap();
    m.drain();
    assert!(m.kickoff(vm, true).unwrap());
    assert_eq!(short(&m.drain()), ["R:DialogOnly"]);
    assert!(!m.kickoff_pending());
}

/// B3: without `bActivateInitialObjectiveSet` no set is active after acceptance; the initial set is then the only
/// target an advance may name; `PlayTurnIn` fires id 14.
#[test]
fn without_the_initial_flag_only_the_initial_set_can_be_advanced_to() {
    let mut toy = toy_mission();
    toy.activate_initial = false;
    toy.behaviors.push(beh("AdvA", "Behavior_AdvanceObjectiveSet", vec![Tag::Obj("ObjectiveSetToAdvanceTo", "SetA")]));
    toy.behaviors.push(remote("TurnIn"));
    toy.events.push(("GoA", vec![(0, "AdvA")]));
    toy.events[0].1.push((14, "RTurnIn"));
    let mut env = Env::new(&toy);
    let m = env.mission();
    let vm = &mut env.vm;
    m.accept(vm, &DetSet::new()).unwrap();
    assert_eq!(m.active_set(), "");
    assert!(m.tracker_lines().is_empty());
    m.drain();
    m.custom_event(vm, "GoB").unwrap();
    assert_eq!(m.active_set(), "", "SetB is not the initial set");
    m.custom_event(vm, "GoA").unwrap();
    assert_eq!(leaf(&m.active_set()), "SetA");
    assert_eq!(short(&m.drain()), ["S:SetA", "R:SetAActive"]);
    m.play_turn_in(vm).unwrap();
    assert_eq!(short(&m.drain()), ["R:TurnIn"]);
}

/// B2: a `bCanCompleteMission` set whose remaining objectives are all optional makes the mission ReadyToTurnIn; the
/// optional one then stays open (updates need Active); the tracker lines flag it.
#[test]
fn only_optional_objectives_left_is_ready_to_turn_in() {
    let mut side = obj("Side", 1);
    side.optional = true;
    side.message = "Toy side step";
    let mut main = obj("Main", 2);
    main.message = "Toy main step";
    let toy = Mission {
        objectives: vec![main, side],
        sets: vec![set("Only", vec!["Main", "Side"], None, true, false)],
        initial: "Only",
        events: vec![],
        behaviors: vec![],
        ..toy_mission()
    };
    let mut env = Env::new(&toy);
    let m = env.mission();
    let vm = &mut env.vm;
    m.accept(vm, &DetSet::new()).unwrap();
    assert!(m.has_optional_objective());
    m.update_objective(vm, "Main", 0).unwrap();
    assert_eq!(m.status(), Status::Active, "one of two");
    m.update_objective(vm, "Main", 0).unwrap();
    assert_eq!(m.status(), Status::ReadyToTurnIn);
    assert!(!m.update_objective(vm, "Side", 0).unwrap(), "refused once ready");
    let lines = m.tracker_lines();
    assert_eq!(
        lines.iter().map(|l| (l.name.as_str(), l.message.as_str(), l.complete, l.optional, l.count)).collect::<Vec<_>>(),
        [("Main", "Toy main step", true, false, 2), ("Side", "Toy side step", false, true, 1)]
    );
}

/// B1: an objective outside the active set, an unknown name and a path of no objective are refused; the last is an
/// error line.
#[test]
fn updates_outside_the_active_set_are_refused() {
    let mut env = Env::new(&toy_mission());
    let m = env.mission();
    let vm = &mut env.vm;
    m.accept(vm, &DetSet::new()).unwrap();
    m.drain();
    assert!(!m.update_objective(vm, "Last", 0).unwrap());
    assert!(!m.update_objective(vm, "Nobody", 0).unwrap());
    assert!(m.drain().is_empty());
    assert!(m.errors().is_empty());
    assert!(!m.update_objective_by_path(vm, "ToyMission.Nobody", 0).unwrap());
    assert_eq!(m.errors(), ["objective is not part of this mission: ToyMission.Nobody"]);
    assert!(m.update_objective_by_path(vm, "ToyMission.Count3", 0).unwrap());
    assert_eq!(m.objective_progress("Count3"), 1);
}

/// The objective classification (NATIVE_BEHAVIOR_POPULATION.md section B) and the set query.
#[test]
fn objective_states_follow_the_tracker() {
    let mut env = Env::new(&toy_mission());
    let m = env.mission();
    let vm = &mut env.vm;
    assert_eq!(m.objective_state("ToyMission.Count3"), "NotStarted");
    assert_eq!(m.objective_state("ToyMission.Nobody"), "");
    m.accept(vm, &DetSet::new()).unwrap();
    assert_eq!(m.objective_state("ToyMission.Count3"), "Active");
    assert_eq!(m.objective_state("ToyMission.Last"), "NotStarted", "not in the active set");
    assert!(m.set_active("ToyMission.SetA"));
    assert!(!m.set_active("ToyMission.SetB"));
    assert!(!m.set_active(""));
    for _ in 0..3 {
        m.update_objective(vm, "Count3", 0).unwrap();
    }
    assert_eq!(m.objective_state("ToyMission.Count3"), "Complete");
    assert!(m.objective_complete("Count3"));
}

/// `GetCompletedBranch`: 0 until ReadyToTurnIn; then the last set of the `NextSet` chain: 1 when all its objectives
/// are complete, 2 when one is not.
#[test]
fn the_completed_branch_reads_the_last_set() {
    let mut side = obj("Side", 1);
    side.optional = true;
    let toy = Mission {
        objectives: vec![obj("Main", 1), side],
        sets: vec![set("Only", vec!["Main", "Side"], None, true, false)],
        initial: "Only",
        events: vec![],
        behaviors: vec![],
        ..toy_mission()
    };
    let mut env = Env::new(&toy);
    let m = env.mission();
    let vm = &mut env.vm;
    assert_eq!(m.completed_branch(), 0);
    m.accept(vm, &DetSet::new()).unwrap();
    assert_eq!(m.completed_branch(), 0);
    m.update_objective(vm, "Main", 0).unwrap();
    assert_eq!(m.status(), Status::ReadyToTurnIn);
    assert_eq!(m.completed_branch(), 2, "the optional objective is open");
    // A last set of another class gives 0.
    let mut other = toy;
    other.sets[0].class = "MissionObjectiveSetCollectionDefinition";
    let mut env = Env::with_mode(&other, ParityMode::Reference);
    let m = env.mission();
    m.load_state("status=2\nactive=\n").then_some(()).unwrap();
    assert_eq!(m.completed_branch(), 0);
}

/// B3: a set of a class the tracker does not activate. In Off mode the mission is refused at construction (`NotYet`,
/// before any state exists); Reference keeps the C++ error line, and the set stays inactive.
#[test]
fn an_unsupported_set_class_is_not_yet_or_the_cxx_error_line() {
    let mut toy = toy_mission();
    toy.sets[0].class = "MissionObjectiveSetCollectionDefinition";
    let refused = Env::new(&toy).try_mission().unwrap_err();
    assert!(matches!(refused.not_yet(), Some(n) if n.missing == Missing::NotPorted), "{refused}");
    let mut env = Env::with_mode(&toy, ParityMode::Reference);
    let m = env.mission();
    let vm = &mut env.vm;
    assert!(m.accept(vm, &DetSet::new()).unwrap());
    assert_eq!(m.active_set(), "");
    assert_eq!(
        m.errors(),
        ["unsupported objective set class WillowGame.MissionObjectiveSetCollectionDefinition (SetA)"]
    );
}

/// B6 and `CanStartMission`: every dependency complete; an objective dependency holds when the objective is Complete,
/// or Active for an Active dependency.
#[test]
fn availability_needs_the_dependencies() {
    let mut toy = toy_mission();
    toy.dependencies = vec!["Before"];
    let mut env = Env::new(&toy);
    let m = env.mission();
    assert_eq!(m.dependencies(), ["Before"]);
    let mut done = DetSet::new();
    assert!(!m.can_start(&done, &env.vm).unwrap());
    assert!(!m.accept(&mut env.vm, &done).unwrap());
    done.insert("Before".to_string());
    assert!(m.can_start(&done, &env.vm).unwrap());
    let mut toy = toy_mission();
    toy.dependency = Some(("Count3", 1));
    let mut env = Env::new(&toy);
    let m = env.mission();
    let states = |s: &str| -> DetMap<String, String> { [("ToyMission.Count3".to_string(), s.to_string())].into_iter().collect() };
    assert!(!m.available(&DetSet::new(), &DetMap::new()));
    assert!(m.available(&DetSet::new(), &states("Active")));
    assert!(m.available(&DetSet::new(), &states("Complete")));
    assert!(!m.available(&DetSet::new(), &states("NotStarted")));
    let mut toy = toy_mission();
    toy.dependency = Some(("Count3", 0));
    let mut env = Env::new(&toy);
    let m = env.mission();
    assert!(!m.available(&DetSet::new(), &states("Active")), "a Complete dependency");
    assert!(!m.can_end());
}

/// The lent weapon (observed, FIRE_MISSION_GOLDEN_TRACE.md lane L2): granted inside Active after the status, removed
/// inside Complete after the status and before the reward; not at ReadyToTurnIn. Only when the weapon's objective is
/// the mission's.
#[test]
fn the_lent_weapon_sits_in_the_status_routine() {
    let mut toy = toy_mission();
    toy.weapon_objective = Some("Count3");
    // Off mode: the weapon's region-stage condition is not read, so the status routine refuses before it changes
    // anything.
    let mut off = Env::new(&toy);
    let m = off.mission();
    let refused = m.accept(&mut off.vm, &DetSet::new()).unwrap_err();
    assert!(matches!(refused.not_yet(), Some(n) if n.missing == Missing::NotRead));
    assert_eq!(m.status(), Status::NotStarted);
    assert!(m.drain().is_empty());
    let mut env = Env::with_mode(&toy, ParityMode::Reference);
    let m = env.mission();
    assert!(m.lends_weapon());
    assert_eq!(m.weapon_definition(), "ToyWeapon");
    let vm = &mut env.vm;
    m.accept(vm, &DetSet::new()).unwrap();
    assert_eq!(short(&m.drain())[..2], ["status:Active".to_string(), "mission_weapon_granted".to_string()]);
    m.load_state("status=2\nactive=ToyMission.SetC\n");
    assert!(m.turn_in_mission(vm).unwrap());
    assert_eq!(short(&m.drain()), ["status:Complete", "mission_weapon_removed", "R:Done", "reward"]);
}

/// `Behavior_ChangeRemoteBehaviorSequenceState`: the path's last name that is neither empty nor `None`, the sequence,
/// and the action's enum name.
#[test]
fn a_sequence_state_change_names_the_provider_leaf() {
    let mut toy = toy_mission();
    toy.behaviors.push(beh(
        "Change",
        "Behavior_ChangeRemoteBehaviorSequenceState",
        vec![Tag::Change(&["Outer", "ToyProvider_3", "None"], "Hidden", 2)],
    ));
    toy.events.push(("Swap", vec![(0, "Change")]));
    let mut env = Env::new(&toy);
    let m = env.mission();
    let vm = &mut env.vm;
    m.accept(vm, &DetSet::new()).unwrap();
    m.drain();
    m.custom_event(vm, "Swap").unwrap();
    let effects = m.drain();
    assert_eq!(effects.len(), 1);
    assert_eq!(
        (effects[0].kind, effects[0].a.as_str(), effects[0].b.as_str(), effects[0].c.as_str()),
        (EffectKind::SetSequence, "ToyProvider_3", "Hidden", "CHANGE_Disable")
    );
}

/// The dialog behavior has no handler here (lane W5-DIALOG): the kernel's error line and its `NotYet`, and the branch
/// stops.
#[test]
fn the_dialog_behavior_is_not_yet() {
    let mut toy = toy_mission();
    toy.behaviors.push(beh("Talk", "Behavior_TriggerDialogEvent", vec![]));
    toy.events.push(("Chat", vec![(0, "Talk")]));
    let mut env = Env::new(&toy);
    let m = env.mission();
    let vm = &mut env.vm;
    m.accept(vm, &DetSet::new()).unwrap();
    m.custom_event(vm, "Chat").unwrap();
    assert!(m.errors().iter().any(|e| e.contains("GearboxFramework.Behavior_TriggerDialogEvent")), "{:?}", m.errors());
    let n = m.not_yet();
    assert_eq!(n.len(), 1);
    assert_eq!(n[0].missing, Missing::NotPorted);
}

/// The hooks, in the C++ order: the status hook inside the routine (before the status effect), the observers after
/// the tracker's own change, the objective hook after the "updated" notification and before the id-3 event.
#[test]
fn hooks_run_in_the_routine_order() {
    let mut env = Env::new(&toy_mission());
    let m = env.mission();
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let (l1, l2, l3) = (log.clone(), log.clone(), log.clone());
    let me = m.clone();
    m.set_status_hook(Some(Rc::new(move |_: &mut Vm, n: u8| {
        // The new status is already the tracker's, and no effect of it is raised yet.
        l1.borrow_mut().push(format!("status {n} {:?} {}", me.status(), me.drain().len()));
    })));
    m.set_notify_hook(Some(Rc::new(move |_: &mut Vm, n: Notification| l2.borrow_mut().push(format!("{n:?}")))));
    m.set_objective_hook(Some(Rc::new(move |_: &mut Vm, path: &str, bit: i32| l3.borrow_mut().push(format!("objective {path} {bit}")))));
    let vm = &mut env.vm;
    m.accept(vm, &DetSet::new()).unwrap();
    m.update_objective(vm, "Count3", 0).unwrap();
    assert_eq!(
        *log.borrow(),
        [
            "status 1 Active 0",
            "StatusChanged",
            "ObjectiveSetChanged",
            "ObjectiveUpdated",
            "objective ToyMission.Count3 0",
            // The id-3 event queues the Echo update behind this one.
            "ObjectiveUpdated",
            "objective ToyMission.Echo 0",
        ]
    );
}

/// A hook may call back into the tracker (the bridge's script does): the state is not borrowed across it.
#[test]
fn a_hook_can_read_and_change_the_tracker() {
    let mut env = Env::new(&toy_mission());
    let m = env.mission();
    let me = m.clone();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    m.set_objective_hook(Some(Rc::new(move |vm: &mut Vm, _: &str, _: i32| {
        s.borrow_mut().push(me.objective_progress("Count3"));
        // An update from inside the hook is queued behind the running one (B1).
        assert!(me.update_objective(vm, "Count3", 0).unwrap());
    })));
    let vm = &mut env.vm;
    m.accept(vm, &DetSet::new()).unwrap();
    m.update_objective(vm, "Count3", 0).unwrap();
    m.set_objective_hook(None);
    assert_eq!(m.objective_progress("Count3"), 3, "each update queued one more, until the count");
    assert_eq!(seen.borrow()[0], 1);
}

/// `lend` puts the running provider back in its slot for the call and takes it back; a slot that is not empty is the
/// busy refusal.
#[test]
fn lend_swaps_the_provider_and_refuses_an_occupied_slot() {
    let mut env = Env::new(&toy_mission());
    let m = env.mission();
    let mut p = m.inner.provider.borrow_mut().take().unwrap();
    let before = p.path().to_string();
    let seen = lend(&m.inner.provider, &m.inner.lost, &mut p, || {
        Ok(m.inner.provider.borrow().as_ref().map(|x| x.path().to_string()))
    })
    .unwrap();
    assert_eq!(seen.as_deref(), Some(before.as_str()));
    assert!(m.inner.provider.borrow().is_none(), "taken back");
    *m.inner.provider.borrow_mut() = Some(Provider::from_data(p.data().clone()));
    let refused = lend(&m.inner.provider, &m.inner.lost, &mut p, || Ok(()));
    assert!(matches!(refused, Err(e) if e.not_yet().is_some_and(|n| n.what == PROVIDER_BUSY)));
    let _ = &mut env;
}

/// Persistence: the text names the status, the active set, completed objectives, open progress and completed sets;
/// a restored mission has heard its kickoff; bad texts change nothing.
#[test]
fn the_state_saves_and_restores() {
    let mut env = Env::new(&toy_mission());
    let m = env.mission();
    let vm = &mut env.vm;
    m.accept(vm, &DetSet::new()).unwrap();
    m.update_objective(vm, "Count3", 0).unwrap();
    let text = m.save_state();
    assert_eq!(text, "status=1\nactive=ToyMission.SetA\nprogress=Count3:1\nprogress=Echo:1\n");
    let mut other = Env::new(&toy_mission());
    let n = other.mission();
    assert!(n.load_state(&text));
    assert_eq!(n.status(), Status::Active);
    assert_eq!(n.objective_progress("Echo"), 1);
    assert!(!n.kickoff_pending());
    n.tick(&mut other.vm, 0.1).unwrap();
    assert!(n.drain().is_empty(), "a restored mission does not replay its kickoff");
    for bad in ["", "status=9\n", "status=1\nactive=Nowhere\n", "progress=Count3\nstatus=1\n", "what=1\nstatus=0\n", "x\n"] {
        assert!(!n.load_state(bad), "{bad:?}");
        assert_eq!(n.status(), Status::Active, "{bad:?} changed nothing");
    }
    // A completed objective's progress is its count; a mask objective's, its full mask.
    assert!(n.load_state("status=1\nactive=ToyMission.SetB\nobjective=Mask\nobjective=Count3\n"));
    assert_eq!(n.objective_progress("Mask"), 2);
    assert_eq!(n.objective_progress("Count3"), 3);
}

/// The reading of the data: the name, a mission with no provider, and the C++ texts of a missing or wrong export.
#[test]
fn the_definition_is_read_with_the_cxx_texts() {
    let mut no_provider = toy_mission();
    no_provider.provider = false;
    let mut env = Env::new(&no_provider);
    let m = env.mission();
    assert_eq!(m.name(), "Toy mission");
    assert!(!m.has_provider());
    assert!(m.accept(&mut env.vm, &DetSet::new()).unwrap());
    m.tick(&mut env.vm, 1.0).unwrap();
    let name = PackageName::parse("TestMission").unwrap();
    let missing = MissionSystem::new(&mut env.vm, &name, "Nowhere").unwrap_err();
    assert_eq!(missing.to_string(), "mission not found: Nowhere");
    let wrong = MissionSystem::new(&mut env.vm, &name, "ToyMission.SetA").unwrap_err();
    assert_eq!(wrong.to_string(), "not a MissionDefinition: ToyMission.SetA");
}

/// A lend whose call takes the provider and does not give it back loses it: the mission is marked lost and every later
/// call is refused (`PROVIDER_LOST`), instead of the placeholder copy running.
#[test]
fn a_lost_provider_makes_the_mission_unusable() {
    let mut env = Env::new(&toy_mission());
    let m = env.mission();
    let mut p = m.inner.provider.borrow_mut().take().unwrap();
    let lost = lend(&m.inner.provider, &m.inner.lost, &mut p, || {
        drop(m.inner.provider.borrow_mut().take());
        Ok(())
    });
    assert!(matches!(lost, Err(e) if e.not_yet().is_some_and(|n| n.what == PROVIDER_LOST)));
    assert!(m.is_lost());
    let vm = &mut env.vm;
    for refused in [
        m.accept(vm, &DetSet::new()).map(|_| ()),
        m.kickoff(vm, false).map(|_| ()),
        m.update_objective(vm, "Count3", 0).map(|_| ()),
        m.custom_event(vm, "GoB").map(|_| ()),
        m.turn_in_mission(vm).map(|_| ()),
        m.tick(vm, 0.1),
        m.play_turn_in(vm),
        m.bind_kismet_variable("X", None),
    ] {
        assert!(matches!(refused, Err(e) if e.not_yet().is_some_and(|n| n.what == PROVIDER_LOST)));
    }
    assert!(!m.load_state("status=1\nactive=\n"));
    assert_eq!(m.status(), Status::NotStarted);
}

/// B1 and the save text: a zero bit on a bit-mask objective is refused before its progress entry is made, so the save
/// text has no `progress=` line for it (the C++ order, `mission.cpp`).
#[test]
fn a_refused_zero_bit_leaves_no_progress_line() {
    let mut env = Env::new(&toy_mission());
    let m = env.mission();
    let vm = &mut env.vm;
    m.accept(vm, &DetSet::new()).unwrap();
    m.custom_event(vm, "GoB").unwrap();
    assert!(!m.update_objective(vm, "Mask", 0).unwrap());
    assert_eq!(m.save_state(), "status=1\nactive=ToyMission.SetB\n");
}

/// `CanStartMission` of a Complete repeatable mission: restarting is not read, so `NotYet` in Off mode; Reference
/// answers the C++ false. A NotStarted repeatable mission starts as any other.
#[test]
fn a_repeatable_complete_mission_is_not_yet() {
    let mut toy = toy_mission();
    toy.repeatable = true;
    let mut env = Env::new(&toy);
    let m = env.mission();
    assert!(m.can_start(&DetSet::new(), &env.vm).unwrap());
    assert!(m.load_state("status=3\nactive=\n"));
    let refused = m.can_start(&DetSet::new(), &env.vm).unwrap_err();
    assert!(matches!(refused.not_yet(), Some(n) if n.missing == Missing::NotRead));
    assert!(m.accept(&mut env.vm, &DetSet::new()).is_err());
    let mut reference = Env::with_mode(&toy, ParityMode::Reference);
    let m = reference.mission();
    assert!(m.load_state("status=3\nactive=\n"));
    assert!(!m.can_start(&DetSet::new(), &reference.vm).unwrap());
    // Not repeatable: the C++ false in both modes.
    let mut env = Env::new(&toy_mission());
    let m = env.mission();
    assert!(m.load_state("status=3\nactive=\n"));
    assert!(!m.can_start(&DetSet::new(), &env.vm).unwrap());
}

/// A plot-critical mission whose activation finds a pending kickoff record: the overwrite is not read, so Off refuses
/// before the status changes; Reference keeps the C++, which does not overwrite: the record that was pending stays
/// (not one from this activation), so the tick plays `PlayKickoffDialogOnly` (id 13, linked to nothing here) instead of
/// the activation's `PlayKickoff` (id 12, `R:Kick`). A one-mission tracker reaches this state only through a test.
#[test]
fn a_plot_critical_overwrite_is_not_yet() {
    let mut toy = toy_mission();
    toy.plot_critical = true;
    let mut env = Env::new(&toy);
    let m = env.mission();
    assert!(m.plot_critical());
    m.inner.state.borrow_mut().kickoff_pending = true;
    let refused = m.accept(&mut env.vm, &DetSet::new()).unwrap_err();
    assert!(matches!(refused.not_yet(), Some(n) if n.missing == Missing::NotRead));
    assert_eq!(m.status(), Status::NotStarted);
    let mut env = Env::with_mode(&toy, ParityMode::Reference);
    let m = env.mission();
    m.inner.state.borrow_mut().kickoff_pending = true;
    assert!(m.accept(&mut env.vm, &DetSet::new()).unwrap());
    m.drain();
    m.tick(&mut env.vm, 0.5).unwrap();
    assert!(!short(&m.drain()).contains(&"R:Kick".to_string()), "the pending record was kept, not overwritten");
    // The same mission without a pending record: the activation's own record, PlayKickoff.
    let mut env = Env::with_mode(&toy, ParityMode::Reference);
    let m = env.mission();
    assert!(m.accept(&mut env.vm, &DetSet::new()).unwrap());
    m.drain();
    m.tick(&mut env.vm, 0.5).unwrap();
    assert_eq!(short(&m.drain()), ["R:Kick"]);
    // Without a pending record a plot-critical mission is accepted in Off mode too.
    let mut env = Env::new(&toy);
    let m = env.mission();
    assert!(m.accept(&mut env.vm, &DetSet::new()).unwrap());
}
