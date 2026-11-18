//! The invented packages of the provider and kernel tests (AGENTS.md rule 9): the vocabulary the kernel reads by name
//! (`BehaviorProviderDefinition`, `BehaviorSequenceData`, `BVAR_Object`, ...) over invented objects and values, built
//! with `ow_package::testkit` (`Declarations` re-expresses `declarations()` of `tests/behavior_test.py`). As in that
//! test, the toy `EBehaviorVariableType` orders its values differently from the game, so a decoder that assumed enum
//! positions would fail. Unlike it, every object property names a class (the toy `Core.Object`) and every toy class
//! derives from it, which the Rust reflection requires (it refuses an object property whose class reference is 0).
//!
//! Only `ow_package::testkit` is used here, so `ow-cli`'s tests include this same file (`cmd/behavior.rs`) to pin
//! the CLI output on these bytes against what the C++ binary prints for them.

use ow_package::testkit::PackageBuilder;
use ow_package::testkit::reflect::Declarations;

/// The toy enum orders (not the game's).
pub const VARIABLE_TYPES: [&str; 10] = [
    "BVAR_None",
    "BVAR_Object",
    "BVAR_Int",
    "BVAR_Float",
    "BVAR_InstanceData",
    "BVAR_NamedVariable",
    "BVAR_Mystery",
    "BVAR_NamedKismetVariable",
    "BVAR_Bool",
    "BVAR_Vector",
];
pub const LINK_TYPES: [&str; 4] = [
    "BVARLINK_Unknown",
    "BVARLINK_Input",
    "BVARLINK_Output",
    "BVARLINK_Context",
];

fn core() -> Vec<u8> {
    let mut d = Declarations::new(&[]);
    let object = d.cls("Object", 0);
    // The class of a package export (a package cooked into another file holds one).
    d.cls("Package", object);
    d.build()
}

/// The toy `Core.Object` import of a package (made once per builder).
fn object_import(p: &mut PackageBuilder) -> i32 {
    let core = p.add_import("Core", "Package", 0, "Core");
    p.add_import("Core", "Class", core, "Object")
}

fn gearbox() -> Vec<u8> {
    let mut d = Declarations::new(&[
        "Int", "Float", "Bool", "Name", "Object", "Array", "Struct", "Byte",
    ]);
    let object = object_import(&mut d.p);
    let bpd = d.cls("BehaviorProviderDefinition", object);
    d.cls("AIBehaviorProviderDefinition", bpd);
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
    d.build()
}

fn willowgame() -> Vec<u8> {
    let mut d = Declarations::new(&[
        "Float", "Object", "Struct", "Bool", "Array", "Int", "Name", "Byte",
    ]);
    let object = object_import(&mut d.p);
    // No defaults: bSupportsDefaultOutputLink is false unless tagged.
    let context = d.struct_("ToyContext", None);
    d.prop("Bool", context, "bSupportsDefaultOutputLink", 0);
    d.prop("Object", context, "ContextObject", object);
    d.prop("Byte", context, "BehaviorContext", 0);
    let compare = d.cls("Behavior_CompareObject", object);
    d.prop("Object", compare, "ObjectA", object);
    d.prop("Object", compare, "ObjectB", object);
    d.prop("Struct", compare, "Context", context);
    d.cls("Behavior_ToySame", object);
    d.cls("Behavior_ToyDifferent", object);
    let data = d.struct_("ToyBehaviorContextData", None);
    d.prop("Object", data, "ContextObject", object);
    d.prop("Byte", data, "BehaviorContext", 0);
    let toy = d.cls("Behavior_ToyInputs", object);
    d.prop("Int", toy, "Count", 0);
    d.prop("Float", toy, "Scale", 0);
    d.prop("Object", toy, "Target", object);
    d.prop("Struct", toy, "Ctx", data);
    d.prop("Struct", toy, "Context", context);
    d.cls("ToyCondition", object);
    d.build()
}

fn things() -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    let object = object_import(&mut p);
    let none = p.fname("None", 0);
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend_from_slice(&none);
    let group = p.add_export(object, "DamageType", payload.clone());
    for leaf in ["ToyFire", "ToyIce"] {
        let r = p.add_export(object, leaf, payload.clone());
        p.set_outer(r, group);
    }
    p.build()
}

/// A tag of a behavior export.
#[derive(Debug, Clone)]
pub enum Tag {
    Obj(&'static str, Target),
    Int(&'static str, i32),
    Float(&'static str, f32),
    /// `Context.bSupportsDefaultOutputLink`.
    DefaultOutput(bool),
}

/// What an object tag or word names.
#[derive(Debug, Clone, Copy)]
pub enum Target {
    Fire,
    /// A reference outside the package's tables.
    Bad,
}

/// One behavior: name, class (`WillowGame.<class>`), tags, linked-variables range, output-links range.
#[derive(Debug, Clone)]
pub struct Beh {
    pub name: &'static str,
    pub class: &'static str,
    pub tags: Vec<Tag>,
    pub vars: (u32, u32),
    pub links: (u32, u32),
}

pub fn beh(name: &'static str, class: &'static str, links: (u32, u32)) -> Beh {
    Beh {
        name,
        class,
        tags: Vec::new(),
        vars: (0, 0),
        links,
    }
}

/// One event: name, gates (bEnabled, MaxTriggerCount, ReTriggerDelay), output-variables range, links range.
#[derive(Debug, Clone)]
pub struct Ev {
    pub name: &'static str,
    pub enabled: Option<bool>,
    pub max: Option<i32>,
    pub retrigger: Option<f32>,
    pub vars: (u32, u32),
    pub links: (u32, u32),
}

pub fn ev(name: &'static str, links: (u32, u32)) -> Ev {
    Ev {
        name,
        enabled: None,
        max: None,
        retrigger: None,
        vars: (0, 0),
        links,
    }
}

/// One property-to-variable link: property, link type, connection index, linked-variables range.
#[derive(Debug, Clone)]
pub struct VLink(pub &'static str, pub &'static str, pub u8, pub (u32, u32));

#[derive(Debug, Clone, Default)]
pub struct Seq {
    pub name: &'static str,
    pub enabled_on_spawn: Option<bool>,
    pub mutex: bool,
    pub condition: bool,
    pub events: Vec<Ev>,
    pub behaviors: Vec<Beh>,
    /// (target behavior, id byte as a signed value, delay)
    pub links: Vec<(u32, i32, f32)>,
    pub variables: Vec<(&'static str, &'static str)>,
    pub vlinks: Vec<VLink>,
    pub linked: Vec<i32>,
}

/// One word of the value block.
#[derive(Debug, Clone, Copy)]
pub enum Word {
    I(i32),
    F(f32),
    Obj(Target),
    /// An FName (8 bytes).
    Name(&'static str),
}

/// A provider package: the provider `name` of class `GearboxFramework.<class>`, its sequences and its value block.
#[derive(Debug, Clone)]
pub struct Spec {
    pub name: &'static str,
    pub class: &'static str,
    pub sequences: Vec<Seq>,
    pub block: Vec<Word>,
    /// When set, the provider sits in an export of class `Core.Package` of that name, as a package cooked into
    /// another file holds it: its path is then `<name>.<provider>` in every file that holds it (a cooked copy).
    pub outer_package: Option<&'static str>,
}

impl Spec {
    pub fn new(name: &'static str, sequences: Vec<Seq>, block: Vec<Word>) -> Self {
        Spec {
            name,
            class: "BehaviorProviderDefinition",
            sequences,
            block,
            outer_package: None,
        }
    }
}

fn packed(first: u32, length: u32) -> i32 {
    ((first << 16) | length) as i32
}

/// The provider package of `spec`.
pub fn provider_package(spec: &Spec) -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    let core = p.add_import("Core", "Package", 0, "Core");
    let class = p.add_import("Core", "Class", core, "Class");
    let _ = class;
    let things = p.add_import("Core", "Package", 0, "Things");
    let group = p.add_import("Core", "Object", things, "DamageType");
    let fire = p.add_import("Core", "Object", group, "ToyFire");
    let ice = p.add_import("Core", "Object", group, "ToyIce");
    let gearbox = p.add_import("Core", "Package", 0, "GearboxFramework");
    let provider_class = p.add_import("Core", "Class", gearbox, spec.class);
    let willow = p.add_import("Core", "Package", 0, "WillowGame");
    let mut classes: Vec<(&str, i32)> = Vec::new();
    let mut class_of = |p: &mut PackageBuilder, name: &'static str| -> i32 {
        if let Some(&(_, r)) = classes.iter().find(|(n, _)| *n == name) {
            return r;
        }
        let r = p.add_import("Core", "Class", willow, name);
        classes.push((name, r));
        r
    };
    let provider = p.add_export(provider_class, spec.name, Vec::new());
    if let Some(outer) = spec.outer_package {
        let package_class = p.add_import("Core", "Class", core, "Package");
        let mut payload = 0i32.to_le_bytes().to_vec();
        payload.extend_from_slice(&p.none());
        let package = p.add_export(package_class, outer, payload);
        p.set_outer(provider, package);
    }
    // Behavior exports, sequence by sequence.
    let mut behavior_refs: Vec<Vec<i32>> = Vec::new();
    for sequence in &spec.sequences {
        let mut refs = Vec::new();
        for b in &sequence.behaviors {
            let c = class_of(&mut p, b.class);
            let r = p.add_export(c, b.name, Vec::new());
            p.set_outer(r, provider);
            refs.push(r);
        }
        behavior_refs.push(refs);
    }
    let condition_class = class_of(&mut p, "ToyCondition");
    let condition = p.add_export(condition_class, "Condition", Vec::new());
    p.set_outer(condition, provider);
    let mut none_payload = 0i32.to_le_bytes().to_vec();
    let none = p.none();
    none_payload.extend_from_slice(&none);
    p.set_payload(condition, none_payload);
    let _ = ice;
    let target = |t: Target| match t {
        Target::Fire => fire,
        Target::Bad => 999,
    };
    for (s, sequence) in spec.sequences.iter().enumerate() {
        for (b, behavior) in sequence.behaviors.iter().enumerate() {
            let mut payload = 0i32.to_le_bytes().to_vec();
            for tag in &behavior.tags {
                let bytes = match tag {
                    Tag::Obj(name, t) => p.object_tag(name, target(*t)),
                    Tag::Int(name, v) => p.int_tag(name, *v),
                    Tag::Float(name, v) => p.float_tag(name, *v),
                    Tag::DefaultOutput(v) => {
                        let mut body = p.bool_tag("bSupportsDefaultOutputLink", *v);
                        body.extend_from_slice(&p.none());
                        p.struct_tag("Context", "ToyContext", &body)
                    }
                };
                payload.extend_from_slice(&bytes);
            }
            payload.extend_from_slice(&p.none());
            p.set_payload(behavior_refs[s][b], payload);
        }
    }

    let sub = |p: &mut PackageBuilder, name: &str, (first, length): (u32, u32)| -> Vec<u8> {
        let mut body = p.int_tag("ArrayIndexAndLength", packed(first, length));
        body.extend_from_slice(&p.none());
        p.struct_tag(name, "SubarrayData", &body)
    };
    let mut sequences = Vec::new();
    for (s, sequence) in spec.sequences.iter().enumerate() {
        let mut body = p.name_tag("BehaviorSequenceName", sequence.name);
        if let Some(e) = sequence.enabled_on_spawn {
            body.extend(p.bool_tag("bEnabledOnSpawn", e));
        }
        if sequence.mutex {
            body.extend(p.bool_tag("bSequenceEnabledMutex", true));
        }
        if sequence.condition {
            body.extend(p.object_tag("CustomEnableCondition", condition));
        }
        if !sequence.events.is_empty() {
            let mut items = Vec::new();
            for e in &sequence.events {
                let mut user = p.name_tag("EventName", e.name);
                if let Some(v) = e.enabled {
                    user.extend(p.bool_tag("bEnabled", v));
                }
                if let Some(v) = e.max {
                    user.extend(p.int_tag("MaxTriggerCount", v));
                }
                if let Some(v) = e.retrigger {
                    user.extend(p.float_tag("ReTriggerDelay", v));
                }
                user.extend_from_slice(&p.none());
                items.extend(p.struct_tag("UserData", "BehaviorEventUserData", &user));
                items.extend(sub(&mut p, "OutputVariables", e.vars));
                items.extend(sub(&mut p, "OutputLinks", e.links));
                items.extend_from_slice(&p.none());
            }
            body.extend(p.array_tag("EventData2", sequence.events.len() as i32, &items));
        }
        if !sequence.behaviors.is_empty() {
            let mut items = Vec::new();
            for (b, behavior) in sequence.behaviors.iter().enumerate() {
                items.extend(p.object_tag("Behavior", behavior_refs[s][b]));
                items.extend(sub(&mut p, "LinkedVariables", behavior.vars));
                items.extend(sub(&mut p, "OutputLinks", behavior.links));
                items.extend_from_slice(&p.none());
            }
            body.extend(p.array_tag("BehaviorData2", sequence.behaviors.len() as i32, &items));
        }
        if !sequence.variables.is_empty() {
            let mut items = Vec::new();
            for (name, ty) in &sequence.variables {
                items.extend(p.name_tag("Name", name));
                items.extend(p.enum_tag("Type", "EBehaviorVariableType", ty));
                items.extend_from_slice(&p.none());
            }
            body.extend(p.array_tag("VariableData", sequence.variables.len() as i32, &items));
        }
        if !sequence.links.is_empty() {
            let mut items = Vec::new();
            for &(target, id, delay) in &sequence.links {
                let raw = (target | ((id as u32 & 0xFF) << 24)) as i32;
                items.extend(p.int_tag("LinkIdAndLinkedBehavior", raw));
                items.extend(p.float_tag("ActivateDelay", delay));
                items.extend_from_slice(&p.none());
            }
            body.extend(p.array_tag(
                "ConsolidatedOutputLinkData",
                sequence.links.len() as i32,
                &items,
            ));
        }
        if !sequence.vlinks.is_empty() {
            let mut items = Vec::new();
            for VLink(property, kind, connection, range) in &sequence.vlinks {
                items.extend(p.name_tag("PropertyName", property));
                items.extend(p.enum_tag("VariableLinkType", "EBehaviorVariableLinkType", kind));
                items.extend(p.byte_tag("ConnectionIndex", *connection));
                items.extend(sub(&mut p, "LinkedVariables", *range));
                items.extend_from_slice(&p.none());
            }
            body.extend(p.array_tag(
                "ConsolidatedVariableLinkData",
                sequence.vlinks.len() as i32,
                &items,
            ));
        }
        if !sequence.linked.is_empty() {
            let words: Vec<u8> = sequence
                .linked
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect();
            body.extend(p.array_tag(
                "ConsolidatedLinkedVariables",
                sequence.linked.len() as i32,
                &words,
            ));
        }
        body.extend_from_slice(&p.none());
        sequences.extend(body);
    }
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend(p.array_tag("BehaviorSequences", spec.sequences.len() as i32, &sequences));
    payload.extend_from_slice(&p.none());
    for word in &spec.block {
        match *word {
            Word::I(v) => payload.extend(v.to_le_bytes()),
            Word::F(v) => payload.extend(v.to_le_bytes()),
            Word::Obj(t) => payload.extend(target(t).to_le_bytes()),
            Word::Name(n) => payload.extend(p.fname(n, 0)),
        }
    }
    p.set_payload(provider, payload);
    p.build()
}

/// ToyProvider of `tests/behavior_test.py`: sequence Main (OnHit -> Cmp -> Same / Diff) and sequence Extra (disabled,
/// two variables). `block_variant`: "exact", "short", "long"; `extra_type` the first Extra variable's type; `a_link`
/// the link type of `ObjectA`.
pub fn toy_provider(block_variant: &str, extra_type: &'static str, a_link: &'static str) -> Spec {
    let mut cmp = beh("Cmp", "Behavior_CompareObject", (1, 2));
    cmp.vars = (1, 2);
    let main = Seq {
        name: "Main",
        enabled_on_spawn: Some(true),
        events: vec![Ev {
            vars: (0, 1),
            ..ev("OnHit", (0, 1))
        }],
        behaviors: vec![
            cmp,
            beh("Same", "Behavior_ToySame", (0, 0)),
            beh("Diff", "Behavior_ToyDifferent", (0, 0)),
        ],
        links: vec![(0, -1, 0.0), (1, 0, 0.0), (2, 1, 0.0)],
        variables: vec![
            ("None", "BVAR_Object"),
            ("None", "BVAR_Object"),
            ("Count", "BVAR_Int"),
            ("Ref", "BVAR_NamedVariable"),
            ("None", "BVAR_Float"),
        ],
        vlinks: vec![
            VLink("DamageType", "BVARLINK_Output", 0, (0, 1)),
            VLink("ObjectA", a_link, 0, (1, 1)),
            VLink("ObjectB", "BVARLINK_Input", 0, (2, 1)),
        ],
        linked: vec![0, 0, 1],
        ..Seq::default()
    };
    let extra = Seq {
        name: "Extra",
        enabled_on_spawn: Some(false),
        variables: vec![("None", extra_type), ("None", "BVAR_Object")],
        ..Seq::default()
    };
    let mut block = vec![
        Word::I(0),
        Word::Obj(Target::Fire),
        Word::I(7),
        Word::F(1.5),
        Word::I(0),
        Word::Name("ToySwitch"),
        Word::I(0),
    ];
    match block_variant {
        "short" => {
            block.pop();
        }
        "long" => block.push(Word::I(0)),
        _ => {}
    }
    Spec::new("ToyProvider", vec![main, extra], block)
}

/// ToyRules of `tests/behavior_test.py` (`build_rules`).
pub fn rules() -> Spec {
    let names = [
        "A", "B", "A2", "B2", "P", "X", "Y", "X2", "Y2", "C4", "C5", "Twice", "CmpOff", "Dflt",
        "Zero", "One", "CmpOn",
    ];
    let at = |n: &str| names.iter().position(|x| *x == n).unwrap() as u32;
    let mut links: Vec<(u32, i32, f32)> = Vec::new();
    let mut add = |targets: &[(&str, i32)]| {
        let start = links.len() as u32;
        for &(t, id) in targets {
            links.push((at(t), id, 0.0));
        }
        (start, targets.len() as u32)
    };
    let mut out_of: Vec<(&str, (u32, u32))> = Vec::new();
    for (name, targets) in [
        ("A", vec![("A2", -1)]),
        ("B", vec![("B2", -1)]),
        ("P", vec![("X", -1), ("Y", -1)]),
        ("X", vec![("X2", -1)]),
        ("Y", vec![("Y2", -1)]),
        ("CmpOff", vec![("Dflt", -1), ("Zero", 0), ("One", 1)]),
        ("CmpOn", vec![("Dflt", -1), ("Zero", 0), ("One", 1)]),
    ] {
        out_of.push((name, add(&targets)));
    }
    let mut events = Vec::new();
    for (name, targets) in [
        ("Order", vec![("A", -1), ("B", -1)]),
        ("Branch", vec![("P", -1)]),
        ("Filter", vec![("C4", 4), ("C5", 5)]),
        ("Twice", vec![("Twice", 0), ("Twice", 0)]),
        ("Once", vec![("C4", 0)]),
        ("Delay2", vec![("C4", 0)]),
        ("Off", vec![("C4", 0)]),
        ("DefOff", vec![("CmpOff", -1)]),
        ("DefOn", vec![("CmpOn", -1)]),
    ] {
        let mut e = ev(name, add(&targets));
        match name {
            "Once" => e.max = Some(1),
            "Delay2" => e.retrigger = Some(2.0),
            "Off" => e.enabled = Some(false),
            _ => {}
        }
        events.push(e);
    }
    let behaviors = names
        .iter()
        .map(|&n| {
            let links = out_of
                .iter()
                .find(|(o, _)| *o == n)
                .map_or((0, 0), |(_, r)| *r);
            if n.starts_with("Cmp") {
                let mut b = beh(n, "Behavior_CompareObject", links);
                if n == "CmpOn" {
                    b.tags.push(Tag::DefaultOutput(true));
                }
                b.tags.push(Tag::Obj("ObjectA", Target::Fire));
                b
            } else {
                beh(n, "Behavior_ToySame", links)
            }
        })
        .collect();
    Spec::new(
        "ToyRules",
        vec![Seq {
            name: "Rules",
            enabled_on_spawn: Some(true),
            events,
            behaviors,
            links,
            ..Seq::default()
        }],
        vec![],
    )
}

/// Every file of the invented install: the code packages, `Things` and the provider packages given.
pub fn install(providers: &[(&str, Vec<u8>)]) -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<(String, Vec<u8>)> = vec![
        ("Core.upk".into(), core()),
        ("GearboxFramework.upk".into(), gearbox()),
        ("WillowGame.upk".into(), willowgame()),
        ("Things.upk".into(), things()),
    ];
    for (name, bytes) in providers {
        files.push((format!("{name}.upk"), bytes.clone()));
    }
    files
}
