//! Owner: lane W5-DIALOG. The dialog rules on an invented dialog package (AGENTS.md rule 9): invented classes,
//! names and values, shaped only as the rules read them. AI-assisted (Claude Code).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::ScriptToy;
use ow_package::testkit::{Export, ints};

use super::*;

const PACKAGE: &str = "DialogPkg";

/// The invented package's builder: classes and structs declared once, then instances.
struct Pkg {
    t: ScriptToy,
    object: i32,
    event_data: i32,
    talk_entry: i32,
    link: i32,
    out_link: i32,
    template: i32,
    tag: i32,
    group: i32,
    template_group: i32,
    talk: i32,
    trigger: i32,
    instigator_var: i32,
    switch: i32,
    globals: i32,
    body: i32,
    name_tag: i32,
    expansion: i32,
}

/// One `DialogEvents` entry.
struct Event {
    tag: i32,
    enabled: bool,
    action: i32,
}

/// One `TalkData` entry: (name tag, audio event), 0 for none.
type Entry = (i32, i32);

impl Pkg {
    fn new() -> Pkg {
        let mut t = ScriptToy::with_imports(&[
            "Class",
            "Function",
            "ScriptStruct",
            "Enum",
            "IntProperty",
            "FloatProperty",
            "BoolProperty",
            "NameProperty",
            "ObjectProperty",
            "ArrayProperty",
            "StructProperty",
        ]);
        let object = t.class("Object", 0);
        let event_data = toy_struct(&mut t, object, "DialogEventData", None);
        t.prop("Object", event_data, "Tag", 0, object);
        t.prop("Bool", event_data, "bEnabled", 0, 0);
        t.prop("Object", event_data, "OutputAction", 0, object);
        let talk_entry = toy_struct(&mut t, object, "TalkEntry", None);
        t.prop("Object", talk_entry, "NameTag", 0, object);
        t.prop("Object", talk_entry, "TalkAkEvent", 0, object);
        let link = toy_struct(&mut t, object, "NodeLink", None);
        for f in ["FromNodeID", "LinkNumber", "ToNodeID"] {
            t.prop("Int", link, f, 0, 0);
        }
        let out_link = toy_struct(&mut t, object, "OutLink", None);
        toy_array_of(&mut t, out_link, "Links", "Object", object);
        let template = toy_struct(&mut t, object, "TalkTemplate", None);
        toy_array_of(&mut t, template, "TalkData", "Struct", talk_entry);
        t.prop("Float", template, "OutputDelay", 0, 0);
        t.prop("Bool", template, "bInstigatorTalker", 0, 0);
        t.prop("Object", template, "TalkerVariable", 0, object);
        t.prop("Object", template, "OutputAction", 0, object);

        let tag = t.class("GearboxDialogEventTag", object);
        t.prop("Object", tag, "Priority", 0, object);
        for b in [
            "bIsEchoEvent",
            "bDoesNotOverrideSamePriority",
            "bGroupEvent",
            "bSoundEffect",
            "bOncePerSession",
            "bMultiplayerOnly",
        ] {
            t.prop("Bool", tag, b, 0, 0);
        }
        let group = t.class("GearboxDialogGroup", object);
        toy_array_of(&mut t, group, "DialogEvents", "Struct", event_data);
        t.prop("Object", group, "ParentGroup", 0, group);
        toy_array_of(&mut t, group, "OutputLinksToStructs", "Struct", link);
        toy_array_of(&mut t, group, "TalkActs", "Struct", template);
        toy_array_of(&mut t, group, "Nodes", "Object", object);
        let template_group = t.class("GearboxDialogTemplateGroup", group);
        let act = t.class("GearboxDialogAct", object);
        toy_array_of(&mut t, act, "OutputLinks", "Struct", out_link);
        toy_array_of(&mut t, act, "VariableLinks", "Struct", out_link);
        t.prop("Int", act, "NodeID", 0, 0);
        let talk = t.class("GearboxDialogAct_Talk", act);
        t.prop("Bool", talk, "bEnableNoMatch", 0, 0);
        t.prop("Object", talk, "TalkerVariable", 0, object);
        t.prop("Float", talk, "OutputDelay", 0, 0);
        t.prop("Bool", talk, "bInstigatorTalker", 0, 0);
        toy_array_of(&mut t, talk, "TalkData", "Struct", talk_entry);
        let trigger = t.class("WillowDialogAct_Trigger", act);
        t.prop("Object", trigger, "DialogEvent", 0, tag);
        let instigator_var = t.class("GearboxDialogVar_Instigator", object);
        let switch = t.class("GearboxDialogAct_ObjectParameterSwitch", act);
        toy_array_of(&mut t, switch, "Outputs", "Object", object);
        let globals = t.class("DialogGlobalsDefinition", object);
        toy_array_of(&mut t, globals, "Priorities", "Object", object);
        for f in [
            "ActiveMissionMinPriorityStart",
            "ActiveSideMissionMinPriority",
            "ActivePlotMissionMinPriority",
            "DefaultTemplateGroup",
        ] {
            t.prop("Object", globals, f, 0, object);
        }
        toy_array_of(&mut t, globals, "NPCDialogGroups", "Object", group);
        let body = t.class("BodyClassDefinition", object);
        t.prop("Object", body, "DialogName", 0, object);
        toy_array_of(&mut t, body, "DialogGroups", "Object", group);
        t.prop("Bool", body, "bNPCDialog", 0, 0);
        let name_tag = t.class("DialogNameTag", object);
        t.prop("Object", name_tag, "DlcExpansion", 0, object);
        let expansion = t.class("DlcExpansionDefinition", object);
        toy_array_of(&mut t, expansion, "NPCDialogGroups", "Object", group);
        Pkg {
            t,
            object,
            event_data,
            talk_entry,
            link,
            out_link,
            template,
            tag,
            group,
            template_group,
            talk,
            trigger,
            instigator_var,
            switch,
            globals,
            body,
            name_tag,
            expansion,
        }
    }

    /// An ordinary object export (`outer` 0: the package root): one word, the tags, `None`.
    fn add(&mut self, class: i32, name: &str, outer: i32, tags: &[Vec<u8>]) -> i32 {
        let mut payload = ints(&[0]);
        for tag in tags {
            payload.extend_from_slice(tag);
        }
        payload.extend_from_slice(&self.t.none);
        let mut e = Export::new(class, name, payload);
        e.outer = outer;
        self.t.p.add_export_with(e)
    }

    /// The body of one struct element: its tags, then `None`.
    fn element(&mut self, tags: &[Vec<u8>]) -> Vec<u8> {
        let mut body = Vec::new();
        for tag in tags {
            body.extend_from_slice(tag);
        }
        body.extend_from_slice(&self.t.none);
        body
    }

    fn objects(&mut self, name: &str, refs: &[i32]) -> Vec<u8> {
        let body: Vec<u8> = refs.iter().flat_map(|r| r.to_le_bytes()).collect();
        self.t
            .p
            .array_tag(name, i32::try_from(refs.len()).unwrap(), &body)
    }

    fn talk_data(&mut self, entries: &[Entry]) -> Vec<u8> {
        let mut body = Vec::new();
        for &(name, event) in entries {
            let mut tags = Vec::new();
            if name != 0 {
                tags.push(self.t.p.object_tag("NameTag", name));
            }
            if event != 0 {
                tags.push(self.t.p.object_tag("TalkAkEvent", event));
            }
            body.extend(self.element(&tags));
        }
        self.t
            .p
            .array_tag("TalkData", i32::try_from(entries.len()).unwrap(), &body)
    }

    fn out_links(&mut self, name: &str, outputs: &[&[i32]]) -> Vec<u8> {
        let mut body = Vec::new();
        for links in outputs {
            let tag = self.objects("Links", links);
            body.extend(self.element(&[tag]));
        }
        self.t
            .p
            .array_tag(name, i32::try_from(outputs.len()).unwrap(), &body)
    }

    /// A tag object with its priority object and flags.
    fn tag_object(&mut self, name: &str, priority: i32, flags: &[&str]) -> i32 {
        let mut tags = Vec::new();
        if priority != 0 {
            tags.push(self.t.p.object_tag("Priority", priority));
        }
        for f in flags {
            tags.push(self.t.p.bool_tag(f, true));
        }
        let class = self.tag;
        self.add(class, name, 0, &tags)
    }

    /// A talk act.
    fn talk_act(&mut self, name: &str, entries: &[Entry], delay: f32, extra: Vec<Vec<u8>>) -> i32 {
        let mut tags = vec![
            self.talk_data(entries),
            self.t.p.float_tag("OutputDelay", delay),
        ];
        tags.extend(extra);
        let class = self.talk;
        self.add(class, name, 0, &tags)
    }

    /// A dialog group (`class` group or template group) with its events and more tags.
    fn group_object(
        &mut self,
        class: i32,
        name: &str,
        events: &[Event],
        extra: Vec<Vec<u8>>,
    ) -> i32 {
        let mut body = Vec::new();
        for e in events {
            let mut tags = vec![
                self.t.p.object_tag("Tag", e.tag),
                self.t.p.bool_tag("bEnabled", e.enabled),
            ];
            if e.action != 0 {
                tags.push(self.t.p.object_tag("OutputAction", e.action));
            }
            body.extend(self.element(&tags));
        }
        let mut tags =
            vec![
                self.t
                    .p
                    .array_tag("DialogEvents", i32::try_from(events.len()).unwrap(), &body),
            ];
        tags.extend(extra);
        self.add(class, name, 0, &tags)
    }
}

/// The objects of the standard fixture, by export reference.
#[derive(Debug, Clone, Copy)]
struct Ids {
    marcus: i32,
    echo_name: i32,
    group: i32,
    act: i32,
    p_high: i32,
    p_low: i32,
    tag_low: i32,
    tag_high: i32,
    tag_echo: i32,
    tag_once: i32,
    tag_sound: i32,
    tag_template: i32,
    tag_parent: i32,
    tag_nolink: i32,
    tag_link0: i32,
    tag_trigger: i32,
    tag_switch: i32,
    child: i32,
    three_tag: i32,
    three: i32,
    body_npc: i32,
    body_plain: i32,
    g_dlc: i32,
    g_npc: i32,
    g_default: i32,
    speaker: i32,
}

fn fixture() -> (Vec<u8>, Ids) {
    let mut k = Pkg::new();
    let object = k.object;
    // Priorities: index 0 is the more important.
    let p_high = k.add(object, "PriorityHigh", 0, &[]);
    let p_low = k.add(object, "PriorityLow", 0, &[]);
    let names = k.add(object, "Names", 0, &[]);
    let name_tag = k.name_tag;
    let marcus = k.add(name_tag, "DialogName_Marcus", names, &[]);
    let echo_name = k.add(name_tag, "DialogName_Echo", names, &[]);
    let speaker = k.add(name_tag, "DialogName_Speaker", names, &[]);
    let hi = k.add(object, "Ak_Hi", 0, &[]);
    let tag_low = k.tag_object("Tag_Low", p_low, &[]);
    let tag_high = k.tag_object("Tag_High", p_high, &[]);
    let tag_echo = k.tag_object("Tag_Echo", p_low, &["bIsEchoEvent"]);
    let tag_once = k.tag_object("Tag_Once", p_low, &["bOncePerSession"]);
    let tag_sound = k.tag_object("Tag_Sound", p_low, &["bSoundEffect"]);
    let tag_template = k.tag_object("Tag_Template", p_low, &[]);
    let tag_parent = k.tag_object("Tag_Parent", p_low, &[]);
    let tag_nolink = k.tag_object("Tag_NoLink", p_low, &[]);
    let tag_link0 = k.tag_object("Tag_Link0", p_low, &[]);
    let tag_trigger = k.tag_object("Tag_Trigger", p_low, &[]);
    let tag_switch = k.tag_object("Tag_Switch", p_low, &[]);
    let echo_act = k.talk_act("Act_Echo", &[(echo_name, hi)], 0.0, vec![]);
    let act = k.talk_act("Act_Marcus", &[(marcus, hi)], 0.5, vec![]);
    let high = k.talk_act("Act_High", &[(marcus, hi)], 0.0, vec![]);
    let link0 = {
        let other = k.add(object, "Elsewhere", 0, &[]);
        let links = k.out_links("OutputLinks", &[&[other]]);
        k.talk_act("Act_Link0", &[(marcus, hi)], 0.0, vec![links])
    };
    // A trigger act on output 1 of a no-match talk act: fires Tag_Trigger on the instigator's own groups.
    let var = k.add(k.instigator_var, "Var_Instigator", 0, &[]);
    let trigger = {
        let event = k.t.p.object_tag("DialogEvent", tag_trigger);
        let vars = k.out_links("VariableLinks", &[&[var]]);
        let class = k.trigger;
        k.add(class, "Act_Trigger", 0, &[event, vars])
    };
    let no_match = {
        let links = k.out_links("OutputLinks", &[&[], &[trigger]]);
        let flags = vec![
            links,
            k.t.p.bool_tag("bEnableNoMatch", true),
            k.t.p.bool_tag("bInstigatorTalker", true),
        ];
        k.talk_act("Act_NoMatch", &[(marcus, hi)], 0.0, flags)
    };
    let switch = {
        let none = k.objects("Outputs", &[0]);
        let links = k.out_links("OutputLinks", &[&[]]);
        let class = k.switch;
        k.add(class, "Act_Switch", 0, &[none, links])
    };
    // The template act is reached by link (event 2, link 0) -> node 12, past the 11 events: TalkActs[0].
    let template_act = {
        let data = k.talk_data(&[(marcus, hi)]);
        let delay = k.t.p.float_tag("OutputDelay", 0.25);
        k.element(&[data, delay])
    };
    let talk_acts = k.t.p.array_tag("TalkActs", 1, &template_act);
    let link = {
        let f = k.t.p.int_tag("FromNodeID", 2);
        let n = k.t.p.int_tag("LinkNumber", 0);
        let to = k.t.p.int_tag("ToNodeID", 12);
        k.element(&[f, n, to])
    };
    let links = k.t.p.array_tag("OutputLinksToStructs", 1, &link);
    let class = k.group;
    let events = [
        Event {
            tag: tag_low,
            enabled: true,
            action: act,
        },
        Event {
            tag: tag_template,
            enabled: true,
            action: 0,
        },
        Event {
            tag: tag_high,
            enabled: true,
            action: high,
        },
        Event {
            tag: tag_echo,
            enabled: true,
            action: echo_act,
        },
        Event {
            tag: tag_once,
            enabled: true,
            action: act,
        },
        Event {
            tag: tag_sound,
            enabled: true,
            action: act,
        },
        Event {
            tag: tag_nolink,
            enabled: true,
            action: 0,
        },
        Event {
            tag: tag_link0,
            enabled: true,
            action: link0,
        },
        Event {
            tag: tag_low,
            enabled: false,
            action: high,
        },
        Event {
            tag: tag_trigger,
            enabled: true,
            action: act,
        },
        Event {
            tag: tag_switch,
            enabled: true,
            action: switch,
        },
    ];
    let group = k.group_object(class, "Group_Main", &events, vec![talk_acts, links]);
    // A group with no event for Tag_Parent whose parent has it.
    let parent = k.group_object(
        class,
        "Group_Parent",
        &[Event {
            tag: tag_parent,
            enabled: true,
            action: act,
        }],
        vec![],
    );
    let parent_tag = k.t.p.object_tag("ParentGroup", parent);
    let child = k.group_object(class, "Group_Child", &[], vec![parent_tag]);
    // A group whose talk act has the speaker no entry: no match, output 1.
    let no_match_group = k.group_object(
        class,
        "Group_NoMatch",
        &[Event {
            tag: tag_low,
            enabled: true,
            action: no_match,
        }],
        vec![],
    );
    let _ = no_match_group;

    // A group whose one act has three TalkData entries: the random pick chooses among them.
    let three_tag = k.tag_object("Tag_Three", p_low, &[]);
    let three_act = k.talk_act(
        "Act_Three",
        &[(marcus, hi), (echo_name, hi), (speaker, hi)],
        0.0,
        vec![],
    );
    let three = k.group_object(
        class,
        "Group_Three",
        &[Event {
            tag: three_tag,
            enabled: true,
            action: three_act,
        }],
        vec![],
    );

    // GetDialogGroups: the body's groups, the name tag's DLC groups, the globals' NPC groups, the default template.
    let g_dlc = k.group_object(class, "Group_Dlc", &[], vec![]);
    let g_npc = k.group_object(class, "Group_Npc", &[], vec![]);
    let template_class = k.template_group;
    let g_default = k.group_object(template_class, "Group_Default", &[], vec![]);
    let dlc_groups = k.objects("NPCDialogGroups", &[g_dlc]);
    let expansion_class = k.expansion;
    let dlc = k.add(expansion_class, "Dlc", 0, &[dlc_groups]);
    let dlc_tag = k.t.p.object_tag("DlcExpansion", dlc);
    let npc_name = k.add(name_tag, "DialogName_Npc", names, &[dlc_tag]);
    let body_class = k.body;
    let body_npc = {
        let name = k.t.p.object_tag("DialogName", npc_name);
        let groups = k.objects("DialogGroups", &[group, 0]);
        let npc = k.t.p.bool_tag("bNPCDialog", true);
        k.add(body_class, "Body_Npc", 0, &[name, groups, npc])
    };
    let body_plain = {
        let name = k.t.p.object_tag("DialogName", marcus);
        let groups = k.objects("DialogGroups", &[group]);
        k.add(body_class, "Body_Plain", 0, &[name, groups])
    };

    // The globals at GD_Globals.Dialog.DialogGlobals.
    let gd = k.add(object, "GD_Globals", 0, &[]);
    let dialog = k.add(object, "Dialog", gd, &[]);
    let globals_class = k.globals;
    let priorities = k.objects("Priorities", &[p_high, p_low]);
    let npc_groups = k.objects("NPCDialogGroups", &[g_npc]);
    let default = k.t.p.object_tag("DefaultTemplateGroup", g_default);
    let start = k.t.p.object_tag("ActiveMissionMinPriorityStart", p_high);
    let side = k.t.p.object_tag("ActiveSideMissionMinPriority", p_high);
    let plot = k.t.p.object_tag("ActivePlotMissionMinPriority", p_high);
    k.add(
        globals_class,
        "DialogGlobals",
        dialog,
        &[priorities, npc_groups, default, start, side, plot],
    );
    let _ = (k.event_data, k.talk_entry, k.link, k.out_link, k.template);
    let ids = Ids {
        marcus,
        echo_name,
        group,
        act,
        p_high,
        p_low,
        tag_low,
        tag_high,
        tag_echo,
        tag_once,
        tag_sound,
        tag_template,
        tag_parent,
        tag_nolink,
        tag_link0,
        tag_trigger,
        tag_switch,
        child,
        three_tag,
        three,
        body_npc,
        body_plain,
        g_dlc,
        g_npc,
        g_default,
        speaker,
    };
    (k.t.build(), ids)
}

static NEXT: AtomicU32 = AtomicU32::new(0);

struct Fx {
    vm: Vm,
    package: PackageId,
    ids: Ids,
    dir: PathBuf,
    lines: Rc<RefCell<Vec<(Line, String)>>>,
    stopped: Rc<RefCell<Vec<i32>>>,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx(mode: ParityMode) -> (Fx, DialogSystem) {
    let dir = std::env::temp_dir().join(format!(
        "ow-dialog-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let (bytes, ids) = fixture();
    let file = format!("{PACKAGE}.upk");
    let (gearbox, provider) = (kernel::gearbox(), kernel::provider_package());
    let defs = DefStore::from_synthetic(
        &dir,
        &[
            (file.as_str(), &bytes),
            ("GearboxFramework.upk", &gearbox),
            ("TestBpd.upk", &provider),
        ],
        IdentityPolicy::FirstInScanOrder,
        mode,
    )
    .unwrap();
    let mut vm = Vm::new(
        ObjectSpace::new(),
        defs,
        Rc::new(AttributeSystem::new_empty()),
        mode,
    );
    let (code, store) = vm.defs.code_mut();
    let package = code.package(store, PACKAGE).unwrap();
    let mut dialog = DialogSystem::new(Some(package));
    let lines = Rc::new(RefCell::new(Vec::new()));
    let sink = lines.clone();
    dialog.on_line = Some(Box::new(move |line: &Line, outcome: &str| {
        sink.borrow_mut().push((line.clone(), outcome.to_string()));
    }));
    let stopped = Rc::new(RefCell::new(Vec::new()));
    let sink = stopped.clone();
    dialog.on_line_stopped = Some(Box::new(move |id| sink.borrow_mut().push(id)));
    (
        Fx {
            vm,
            package,
            ids,
            dir,
            lines,
            stopped,
        },
        dialog,
    )
}

impl Fx {
    fn object(&mut self, index: i32) -> ObjRef {
        self.vm
            .defs
            .resolve_ref(&mut self.vm.space, self.package, index)
            .unwrap()
            .unwrap()
    }

    fn value(&mut self, index: i32) -> Value {
        Value::Object(Some(self.object(index)))
    }

    fn path(&self, index: i32) -> String {
        self.vm
            .defs
            .packages()
            .get(self.package)
            .path_of(index)
            .unwrap()
    }

    fn outcomes(&self) -> Vec<String> {
        self.lines.borrow().iter().map(|(_, o)| o.clone()).collect()
    }
}

fn marcus(f: &Fx, d: &mut DialogSystem) {
    let name =
        f.vm.defs
            .packages()
            .get(f.package)
            .path_of(f.ids.marcus)
            .unwrap();
    d.register_talker(&name, None);
}

/// Triggers `tag` on group `group` from the mission (no instigator).
fn trigger(f: &mut Fx, d: &mut DialogSystem, group: i32, tag: i32) -> Result<Handle, DialogError> {
    let (g, t) = (f.value(group), f.value(tag));
    d.trigger(&mut f.vm, &g, &t)
}

#[test]
fn priority_blocks_a_less_important_line_and_a_more_important_one_interrupts() {
    let (mut f, mut d) = fx(ParityMode::Off);
    marcus(&f, &mut d);
    d.set_test_line_length(10.0);
    let (group, low, high) = (f.ids.group, f.ids.tag_low, f.ids.tag_high);
    let first = trigger(&mut f, &mut d, group, high).unwrap();
    assert!(d.active(first));
    // Marcus is speaking at index 0; index 1 is less important: blocked.
    trigger(&mut f, &mut d, group, low).unwrap();
    assert_eq!(f.outcomes(), ["started", "blocked by priority"]);
    // The same priority does not override either (not an echo event).
    trigger(&mut f, &mut d, group, high).unwrap();
    assert_eq!(f.outcomes()[2], "blocked by priority");
    assert!(d.active(first));
    // Once the first line ends, the next one starts.
    d.tick(10.0);
    assert!(!d.active(first));
    trigger(&mut f, &mut d, group, low).unwrap();
    assert_eq!(f.outcomes()[3], "started");
}

#[test]
fn a_talker_s_next_line_stops_its_live_one() {
    let (mut f, mut d) = fx(ParityMode::Off);
    marcus(&f, &mut d);
    d.set_test_line_length(10.0);
    let (group, low, high) = (f.ids.group, f.ids.tag_low, f.ids.tag_high);
    let first = trigger(&mut f, &mut d, group, low).unwrap();
    let second = trigger(&mut f, &mut d, group, high).unwrap();
    assert_eq!(f.outcomes(), ["started", "started"]);
    assert!(!d.active(first) && d.active(second));
    assert_eq!(
        *f.stopped.borrow(),
        [1],
        "line 1 was stopped while its audio played"
    );
    // A stopped event data is reused by the next trigger, with a new use count.
    d.tick(10.0);
    let third = trigger(&mut f, &mut d, group, low).unwrap();
    assert_eq!(third.index, first.index);
    assert_ne!(third.use_count, first.use_count);
}

#[test]
fn an_echo_event_creates_an_echo_caller_and_a_plain_one_has_no_talker() {
    let (mut f, mut d) = fx(ParityMode::Off);
    d.set_test_line_length(1.0);
    let (group, echo, low) = (f.ids.group, f.ids.tag_echo, f.ids.tag_low);
    trigger(&mut f, &mut d, group, echo).unwrap();
    trigger(&mut f, &mut d, group, low).unwrap();
    let lines = f.lines.borrow().clone();
    assert_eq!(lines[0].1, "started");
    assert!(lines[0].0.echo);
    assert_eq!(lines[0].0.talker, f.path(f.ids.echo_name));
    assert_eq!(lines[1].1, "no talker");
    assert_eq!(lines[1].0.talker, f.path(f.ids.marcus));
}

#[test]
fn a_once_per_session_tag_plays_once() {
    let (mut f, mut d) = fx(ParityMode::Off);
    marcus(&f, &mut d);
    d.set_test_line_length(1.0);
    let (group, once) = (f.ids.group, f.ids.tag_once);
    let first = trigger(&mut f, &mut d, group, once).unwrap();
    assert!(first.index >= 0);
    d.tick(2.0);
    let again = trigger(&mut f, &mut d, group, once).unwrap();
    assert_eq!(again.index, -1);
    assert_eq!(f.outcomes(), ["started"]);
}

#[test]
fn the_last_enabled_entry_is_the_event_and_a_template_is_reached_by_link() {
    let (mut f, mut d) = fx(ParityMode::Off);
    marcus(&f, &mut d);
    d.set_test_line_length(1.0);
    let (group, template) = (f.ids.group, f.ids.tag_template);
    trigger(&mut f, &mut d, group, template).unwrap();
    let (line, outcome) = f.lines.borrow()[0].clone();
    assert_eq!(outcome, "started");
    assert_eq!(line.talk_act, format!("{}.TalkActs[0]", f.path(group)));
    assert_eq!(line.output_delay, 0.25);
    // Tag_Low's disabled second entry is skipped: the enabled first one (Act_Marcus, delay 0.5) runs.
    d.tick(2.0);
    let low = f.ids.tag_low;
    trigger(&mut f, &mut d, group, low).unwrap();
    assert_eq!(f.lines.borrow()[1].0.talk_act, f.path(f.ids.act));
}

#[test]
fn an_event_with_nothing_linked_takes_event_data_and_ends_the_chain() {
    let (mut f, mut d) = fx(ParityMode::Off);
    let (group, nolink, switch) = (f.ids.group, f.ids.tag_nolink, f.ids.tag_switch);
    let h = trigger(&mut f, &mut d, group, nolink).unwrap();
    assert!(h.index >= 0 && !d.active(h));
    // An object-parameter switch whose chosen (None) output has no link ends the chain too.
    let h = trigger(&mut f, &mut d, group, switch).unwrap();
    assert!(h.index >= 0 && !d.active(h));
    assert!(f.lines.borrow().is_empty());
}

#[test]
fn nodes_off_the_route_are_not_implemented() {
    let (mut f, mut d) = fx(ParityMode::Off);
    let (group, sound, link0) = (f.ids.group, f.ids.tag_sound, f.ids.tag_link0);
    let e = trigger(&mut f, &mut d, group, sound).unwrap_err();
    assert_eq!(
        e.cpp_text(),
        "dialog: not implemented: sound-effect dialog events"
    );
    let e = trigger(&mut f, &mut d, group, link0).unwrap_err();
    assert_eq!(
        e.cpp_text(),
        "dialog: not implemented: an output link on a talk act"
    );
    let e = d
        .trigger(&mut f.vm, &Value::Object(None), &Value::Object(None))
        .unwrap_err();
    assert_eq!(e.cpp_text(), "dialog: unresolved object reference");
}

#[test]
fn a_component_trigger_searches_the_parent_group_appended_at_the_end() {
    let (mut f, mut d) = fx(ParityMode::Off);
    d.set_test_line_length(1.0);
    let speaker = f.path(f.ids.speaker);
    let marcus_name = f.path(f.ids.marcus);
    d.register_talker(&marcus_name, None);
    let groups = vec![f.value(f.ids.child), Value::Object(None)];
    let tag = f.value(f.ids.tag_parent);
    assert!(
        d.trigger_on_component(&mut f.vm, &speaker, &groups, &tag)
            .unwrap()
    );
    let child = f.path(f.ids.child);
    assert_eq!(d.last_search(), [child, "Group_Parent".to_string()]);
    // The act's random entry is Marcus's (no instigator entry asked for).
    assert_eq!(f.lines.borrow()[0].0.talker, marcus_name);
    // No group has an event for this tag: nothing happens.
    let none = f.value(f.ids.tag_high);
    assert!(
        !d.trigger_on_component(&mut f.vm, &speaker, &groups, &none)
            .unwrap()
    );
}

#[test]
fn a_no_match_talk_act_triggers_its_event_on_the_instigator_s_own_groups() {
    let (mut f, mut d) = fx(ParityMode::Off);
    d.set_test_line_length(1.0);
    // The speaker has no entry in Act_NoMatch (only Marcus), so output 1, the Trigger act, fires Tag_Trigger on the
    // speaker's own groups (Group_Main, whose Tag_Trigger event runs Act_Marcus).
    let speaker = f.path(f.ids.speaker);
    let marcus_name = f.path(f.ids.marcus);
    d.register_talker(&marcus_name, None);
    let no_match_group = {
        let pkg = f.vm.defs.packages().get(f.package);
        let index = pkg
            .exports
            .iter()
            .position(|e| e.name == "Group_NoMatch")
            .unwrap();
        i32::try_from(index + 1).unwrap()
    };
    let groups = vec![f.value(no_match_group), f.value(f.ids.group)];
    let tag = f.value(f.ids.tag_low);
    assert!(
        d.trigger_on_component(&mut f.vm, &speaker, &groups, &tag)
            .unwrap()
    );
    let (line, outcome) = f.lines.borrow()[0].clone();
    assert_eq!(outcome, "started");
    assert_eq!(line.event_tag, f.path(f.ids.tag_trigger));
    assert_eq!(line.group, f.path(f.ids.group));
}

#[test]
fn pawn_dialog_lists_body_dlc_npc_and_default_template_groups() {
    let (mut f, mut d) = fx(ParityMode::Off);
    let body = f.value(f.ids.body_npc);
    let got = d.pawn_dialog(&mut f.vm, &body).unwrap();
    assert_eq!(got.name_tag, "Names.DialogName_Npc");
    let expected = vec![
        f.value(f.ids.group),
        Value::Object(None),
        f.value(f.ids.g_dlc),
        f.value(f.ids.g_npc),
        f.value(f.ids.g_default),
    ];
    assert_eq!(got.groups, expected);
    let body = f.value(f.ids.body_plain);
    let got = d.pawn_dialog(&mut f.vm, &body).unwrap();
    assert_eq!(
        got.groups,
        vec![f.value(f.ids.group), f.value(f.ids.g_default)]
    );
    assert_eq!(got.name_tag, f.path(f.ids.marcus));
    assert_eq!(
        d.pawn_dialog(&mut f.vm, &Value::None).unwrap(),
        PawnDialog::default()
    );
}

#[test]
fn the_globals_give_priority_indices_and_without_them_every_index_is_minus_one() {
    let (mut f, mut d) = fx(ParityMode::Off);
    let (high, low) = (f.path(f.ids.p_high), f.path(f.ids.p_low));
    assert_eq!(d.index_of(&mut f.vm, &high).unwrap(), 0);
    assert_eq!(d.index_of(&mut f.vm, &low).unwrap(), 1);
    assert_eq!(d.index_of(&mut f.vm, "Elsewhere").unwrap(), i32::MAX);
    assert_eq!(d.index_of(&mut f.vm, "").unwrap(), i32::MAX);
    let mut none = DialogSystem::new(None);
    assert_eq!(none.index_of(&mut f.vm, &high).unwrap(), -1);
}

#[test]
fn the_tracked_mission_floors_its_group_s_priority() {
    let (mut f, mut d) = fx(ParityMode::Off);
    marcus(&f, &mut d);
    d.set_test_line_length(10.0);
    let group_path = f.path(f.ids.group);
    let (group, low, high) = (f.ids.group, f.ids.tag_low, f.ids.tag_high);
    // Floors are index 0: a low line of the tracked group counts as index 0, so a high line no longer overrides it.
    d.set_tracked_mission(&group_path, false);
    trigger(&mut f, &mut d, group, low).unwrap();
    trigger(&mut f, &mut d, group, high).unwrap();
    assert_eq!(f.outcomes(), ["started", "blocked by priority"]);
}

#[test]
fn a_timed_line_finishes_at_its_audio_end_plus_the_delay() {
    let (mut f, mut d) = fx(ParityMode::Off);
    marcus(&f, &mut d);
    d.set_timed_line_player(|_: &Line| 1.25);
    let (group, low) = (f.ids.group, f.ids.tag_low);
    let h = trigger(&mut f, &mut d, group, low).unwrap();
    // One long update past the end: the finish time is 1.25 + 0.5, reached.
    d.tick(1.5);
    assert!(d.active(h), "1.5 < 1.75");
    d.tick(0.25);
    assert!(!d.active(h));
    // A negative length is not played.
    d.set_timed_line_player(|_: &Line| -1.0);
    trigger(&mut f, &mut d, group, low).unwrap();
    assert_eq!(f.outcomes(), ["started", "not playing"]);
}

#[test]
fn a_plain_player_s_line_ends_when_the_host_says_so() {
    let (mut f, mut d) = fx(ParityMode::Off);
    marcus(&f, &mut d);
    d.set_line_player(|_: &Line| true);
    let (group, low) = (f.ids.group, f.ids.tag_low);
    let h = trigger(&mut f, &mut d, group, low).unwrap();
    d.tick(100.0);
    assert!(d.active(h));
    d.line_ended(1);
    d.tick(0.25);
    assert!(d.active(h));
    d.tick(0.25);
    assert!(!d.active(h));
    d.advance_to(50.0);
    assert_eq!(d.now(), 100.5);
}

#[test]
fn the_reference_pick_is_the_cxx_mt19937() {
    // The C++ standard's check value: the 10000th output of a default-seeded mt19937 is 4123659995.
    let mut mt = Mt19937::new(5489);
    assert_eq!(mt.next_u32(), 3_499_211_612);
    for _ in 1..9_999 {
        mt.next_u32();
    }
    assert_eq!(mt.next_u32(), 4_123_659_995);
    // Both modes pick the only entry of a one-entry act.
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let (mut f, mut d) = fx(mode);
        marcus(&f, &mut d);
        d.set_test_line_length(1.0);
        let (group, low) = (f.ids.group, f.ids.tag_low);
        trigger(&mut f, &mut d, group, low).unwrap();
        assert_eq!(f.lines.borrow()[0].0.talker, f.path(f.ids.marcus));
    }
}

#[test]
fn register_talker_keeps_one_pawn_per_name_tag_and_gives_it_an_actor_once() {
    let (mut f, mut d) = fx(ParityMode::Off);
    let a = f.object(f.ids.p_high);
    let b = f.object(f.ids.p_low);
    d.register_talker("X", None);
    d.register_talker("X", Some(a));
    d.register_talker("X", Some(b));
    assert_eq!(d.talkers.len(), 1);
    assert_eq!(d.talkers[0].actor, Some(a));
}

#[test]
fn the_talk_data_pick_follows_the_vm_s_parity_mode() {
    // Reference: the C++ mt19937 seeded 12345; Off: the world's Dialog stream. Each picks among the three entries.
    let expected_reference = usize::try_from(Mt19937::new(12345).next_u32() % 3).unwrap();
    let expected_off = usize::from(
        ow_core::rand::GameRng::from_world_seed(0, ParityMode::Off)
            .crt(CrtStream::Dialog)
            .rand15(),
    ) % 3;
    for (mode, expected) in [
        (ParityMode::Reference, expected_reference),
        (ParityMode::Off, expected_off),
    ] {
        let (mut f, mut d) = fx(mode);
        d.set_test_line_length(1.0);
        let names = [f.ids.marcus, f.ids.echo_name, f.ids.speaker].map(|i| f.path(i));
        for n in &names {
            d.register_talker(n, None);
        }
        let (group, tag) = (f.ids.three, f.ids.three_tag);
        trigger(&mut f, &mut d, group, tag).unwrap();
        assert_eq!(f.lines.borrow()[0].0.talker, names[expected], "{mode:?}");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The behavior, run by the real kernel (ow_behavior::kernel::Provider)
// ---------------------------------------------------------------------------------------------------------------

mod kernel {
    //! A provider package for the kernel (`TestBpd.upk`) and the `GearboxFramework` package it needs: the
    //! vocabulary the kernel reads (as `ow-behavior`'s own fixtures have it), `Behavior_TriggerDialogEvent`, and two
    //! invented boundary classes that only show which output the dialog behavior selected. Every class roots in the
    //! dialog package's toy `Object`, so the behavior's `EventTag` and `Group` take the dialog package's objects.

    use ow_package::testkit::PackageBuilder;
    use ow_package::testkit::reflect::Declarations;

    pub const OUT: &str = "GearboxFramework.Behavior_ToyOut";
    pub const FINISHED: &str = "GearboxFramework.Behavior_ToyFinished";

    pub fn gearbox() -> Vec<u8> {
        let mut d = Declarations::new(&[
            "Int", "Float", "Bool", "Name", "Object", "Array", "Struct", "Byte",
        ]);
        let package = d.p.add_import("Core", "Package", 0, super::PACKAGE);
        let object = d.p.add_import("Core", "Class", package, "Object");
        let bpd = d.cls("BehaviorProviderDefinition", object);
        let var_types = d.enum_(
            bpd,
            "EBehaviorVariableType",
            &["BVAR_None", "BVAR_Object", "BVAR_Int"],
        );
        let link_types = d.enum_(
            bpd,
            "EBehaviorVariableLinkType",
            &[
                "BVARLINK_Unknown",
                "BVARLINK_Context",
                "BVARLINK_Output",
                "BVARLINK_Input",
            ],
        );
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
        let trigger = d.cls("Behavior_TriggerDialogEvent", object);
        d.prop("Object", trigger, "EventTag", object);
        d.prop("Object", trigger, "Group", object);
        d.prop("Bool", trigger, "bForcePlayImmediate", 0);
        d.cls("Behavior_ToyOut", object);
        d.cls("Behavior_ToyFinished", object);
        d.build()
    }

    /// One dialog behavior of a sequence: name, event tag (an export name of the dialog package, or none), group
    /// (likewise), `bForcePlayImmediate`, whether its outputs are linked (Out -> a `Behavior_ToyOut`, Finished -> a
    /// `Behavior_ToyFinished`), and whether its Context link names the sequence's object variable.
    #[derive(Clone, Copy)]
    struct Talk {
        sequence: &'static str,
        event: &'static str,
        tag: Option<&'static str>,
        group: Option<&'static str>,
        now: bool,
        linked: bool,
        context: bool,
    }

    const TALKS: [Talk; 5] = [
        Talk {
            sequence: "Main",
            event: "OnTalk",
            tag: Some("Tag_Low"),
            group: Some("Group_Main"),
            now: false,
            linked: true,
            context: false,
        },
        Talk {
            sequence: "Now",
            event: "OnNow",
            tag: Some("Tag_Low"),
            group: Some("Group_Main"),
            now: true,
            linked: true,
            context: false,
        },
        Talk {
            sequence: "Bare",
            event: "OnBare",
            tag: None,
            group: Some("Group_Main"),
            now: false,
            linked: true,
            context: false,
        },
        Talk {
            sequence: "NoGroup",
            event: "OnNoGroup",
            tag: Some("Tag_Low"),
            group: None,
            now: false,
            linked: false,
            context: false,
        },
        Talk {
            sequence: "Ctx",
            event: "OnCtx",
            tag: Some("Tag_Low"),
            group: Some("Group_NoMatch"),
            now: false,
            linked: false,
            context: true,
        },
    ];

    fn packed(first: u32, length: u32) -> i32 {
        i32::try_from((first << 16) | length).unwrap()
    }

    /// `TestBpd.Provider`: one sequence per [`TALKS`] entry, enabled on spawn, its event starting the dialog
    /// behavior. The Unlinked case is `NoGroup`'s shape with a tag and group (sequence `Unlinked`).
    pub fn provider_package() -> Vec<u8> {
        let mut p = PackageBuilder::new_empty();
        let gearbox = p.add_import("Core", "Package", 0, "GearboxFramework");
        let provider_class = p.add_import("Core", "Class", gearbox, "BehaviorProviderDefinition");
        let trigger = p.add_import("Core", "Class", gearbox, "Behavior_TriggerDialogEvent");
        let out = p.add_import("Core", "Class", gearbox, "Behavior_ToyOut");
        let finished = p.add_import("Core", "Class", gearbox, "Behavior_ToyFinished");
        let dialog = p.add_import("Core", "Package", 0, super::PACKAGE);
        let mut imports: Vec<(&str, i32)> = Vec::new();
        let mut import = |p: &mut PackageBuilder, class: &str, name: &'static str| -> i32 {
            if let Some(&(_, r)) = imports.iter().find(|(n, _)| *n == name) {
                return r;
            }
            let r = p.add_import(super::PACKAGE, class, dialog, name);
            imports.push((name, r));
            r
        };
        let none = p.none();
        let provider = p.add_export(provider_class, "Provider", Vec::new());
        let mut talks: Vec<Talk> = TALKS.to_vec();
        talks.push(Talk {
            sequence: "Unlinked",
            event: "OnUnlinked",
            tag: Some("Tag_Low"),
            group: Some("Group_Main"),
            now: false,
            linked: false,
            context: false,
        });
        let mut sequences = Vec::new();
        let sub = |p: &mut PackageBuilder, name: &str, (first, length): (u32, u32)| -> Vec<u8> {
            let mut body = p.int_tag("ArrayIndexAndLength", packed(first, length));
            body.extend_from_slice(&p.none());
            p.struct_tag(name, "SubarrayData", &body)
        };
        let mut block = Vec::new();
        for talk in &talks {
            // The behaviors: the dialog behavior, then (linked) its Out and Finished targets.
            let mut tags = 0i32.to_le_bytes().to_vec();
            if let Some(t) = talk.tag {
                let r = import(&mut p, "GearboxDialogEventTag", t);
                tags.extend(p.object_tag("EventTag", r));
            }
            if let Some(g) = talk.group {
                let r = import(&mut p, "GearboxDialogGroup", g);
                tags.extend(p.object_tag("Group", r));
            }
            if talk.now {
                tags.extend(p.bool_tag("bForcePlayImmediate", true));
            }
            tags.extend_from_slice(&none);
            let name = format!("Talk{}", talk.sequence);
            let mut refs = vec![p.add_export(trigger, &name, tags)];
            if talk.linked {
                for (class, n) in [(out, "Out"), (finished, "Finished")] {
                    let mut payload = 0i32.to_le_bytes().to_vec();
                    payload.extend_from_slice(&none);
                    refs.push(p.add_export(class, &format!("{n}{}", talk.sequence), payload));
                }
            }
            for &r in &refs {
                p.set_outer(r, provider);
            }
            let mut body = p.name_tag("BehaviorSequenceName", talk.sequence);
            body.extend(p.bool_tag("bEnabledOnSpawn", true));
            // The event: its output variable (Ctx only) and its one link, to the dialog behavior.
            let mut user = p.name_tag("EventName", talk.event);
            user.extend_from_slice(&none);
            let mut items = p.struct_tag("UserData", "BehaviorEventUserData", &user);
            items.extend(sub(
                &mut p,
                "OutputVariables",
                if talk.context { (0, 1) } else { (0, 0) },
            ));
            items.extend(sub(&mut p, "OutputLinks", (0, 1)));
            items.extend_from_slice(&none);
            body.extend(p.array_tag("EventData2", 1, &items));
            let mut items = Vec::new();
            for (i, &r) in refs.iter().enumerate() {
                items.extend(p.object_tag("Behavior", r));
                let vars = if i == 0 && talk.context {
                    (1, 1)
                } else {
                    (0, 0)
                };
                items.extend(sub(&mut p, "LinkedVariables", vars));
                let links = if i == 0 && talk.linked {
                    (1, 2)
                } else {
                    (0, 0)
                };
                items.extend(sub(&mut p, "OutputLinks", links));
                items.extend_from_slice(&none);
            }
            body.extend(p.array_tag("BehaviorData2", i32::try_from(refs.len()).unwrap(), &items));
            // Links: 0 the event's (id -1), 1 Out (id 0), 2 Finished (id 1).
            let mut links: Vec<(u32, u32)> = vec![(0, 0xFF)];
            if talk.linked {
                links.extend([(1, 0), (2, 1)]);
            }
            let mut items = Vec::new();
            for (target, id) in &links {
                items.extend(p.int_tag(
                    "LinkIdAndLinkedBehavior",
                    i32::from_le_bytes((target | (id << 24)).to_le_bytes()),
                ));
                items.extend(p.float_tag("ActivateDelay", 0.0));
                items.extend_from_slice(&none);
            }
            body.extend(p.array_tag(
                "ConsolidatedOutputLinkData",
                i32::try_from(links.len()).unwrap(),
                &items,
            ));
            if talk.context {
                // Variable 0 `Who` (an object): the event's output, the behavior's Context.
                let mut items = p.name_tag("Name", "Who");
                items.extend(p.enum_tag("Type", "EBehaviorVariableType", "BVAR_Object"));
                items.extend_from_slice(&none);
                body.extend(p.array_tag("VariableData", 1, &items));
                let mut items = Vec::new();
                for (property, kind) in [
                    ("Speaker", "BVARLINK_Output"),
                    ("Context", "BVARLINK_Context"),
                ] {
                    items.extend(p.name_tag("PropertyName", property));
                    items.extend(p.enum_tag("VariableLinkType", "EBehaviorVariableLinkType", kind));
                    items.extend(p.byte_tag("ConnectionIndex", 0));
                    items.extend(sub(&mut p, "LinkedVariables", (0, 1)));
                    items.extend_from_slice(&none);
                }
                body.extend(p.array_tag("ConsolidatedVariableLinkData", 2, &items));
                body.extend(p.array_tag("ConsolidatedLinkedVariables", 1, &0i32.to_le_bytes()));
                block.extend(0i32.to_le_bytes());
            }
            body.extend_from_slice(&none);
            sequences.extend(body);
        }
        let mut payload = 0i32.to_le_bytes().to_vec();
        payload.extend(p.array_tag(
            "BehaviorSequences",
            i32::try_from(talks.len()).unwrap(),
            &sequences,
        ));
        payload.extend_from_slice(&none);
        payload.extend(block);
        p.set_payload(provider, payload);
        p.build()
    }
}

/// The kernel over `TestBpd.Provider` with the dialog's handler registered in the VM, and the two toy output
/// classes at the boundary.
fn kernel_fx() -> (Fx, Rc<RefCell<DialogSystem>>, Provider) {
    let (mut f, d) = fx(ParityMode::Off);
    let dialog = Rc::new(RefCell::new(d));
    register(&mut f.vm, Rc::clone(&dialog)).unwrap();
    let definition = {
        let name = ow_core::PackageName::parse("TestBpd").unwrap();
        let (_, store) = f.vm.defs.code_mut();
        let id = store.load(&name).unwrap();
        let index = store.get(id).find_export("Provider").unwrap().unwrap();
        f.vm.defs
            .load_export(
                &mut f.vm.space,
                id,
                index,
                ow_object::prefix::PropertyPrefix::Ordinary,
            )
            .unwrap()
    };
    let mut provider = Provider::new(&mut f.vm, definition).unwrap();
    for class in [kernel::OUT, kernel::FINISHED] {
        provider.report_at_boundary(class, None, Vec::new());
    }
    (f, dialog, provider)
}

fn fire(
    f: &mut Fx,
    provider: &mut Provider,
    event: &str,
    payload: &[Option<ObjRef>],
) -> Result<(), BehaviorError> {
    provider.fire_event(&mut f.vm, event, &ow_core::DetMap::new(), -1, payload)
}

fn boundary(event: &str, class: &str, name: &str) -> String {
    format!("{event} -> {class}:{name}")
}

#[test]
fn in_the_kernel_the_behavior_outs_waits_talks_polls_and_finishes() {
    let (mut f, dialog, mut provider) = kernel_fx();
    marcus(&f, &mut dialog.borrow_mut());
    dialog.borrow_mut().set_test_line_length(1.0);
    fire(&mut f, &mut provider, "OnTalk", &[]).unwrap();
    // First run: Out at once, latent for one wake, no line yet.
    assert_eq!(
        provider.boundary,
        [boundary("OnTalk", kernel::OUT, "OutMain")]
    );
    assert!(f.lines.borrow().is_empty());
    // The next wake triggers the dialog; the line is live, so the behavior polls.
    provider.tick(&mut f.vm, 0.05).unwrap();
    assert_eq!(f.outcomes(), ["started"]);
    assert_eq!(provider.boundary.len(), 1);
    provider.tick(&mut f.vm, 0.5).unwrap();
    assert_eq!(provider.boundary.len(), 1, "still live: no Finished");
    // The test player's line ends at the dialog update that reaches 1.0 (here 2.0), and the act OutputDelay (0.5)
    // after it; the next poll selects Finished.
    dialog.borrow_mut().tick(2.0);
    provider.tick(&mut f.vm, 0.5).unwrap();
    assert_eq!(provider.boundary.len(), 1, "the act ends at 2.5");
    dialog.borrow_mut().tick(0.5);
    provider.tick(&mut f.vm, 0.5).unwrap();
    assert_eq!(
        provider.boundary,
        [
            boundary("OnTalk", kernel::OUT, "OutMain"),
            boundary("OnTalk", kernel::FINISHED, "FinishedMain")
        ]
    );
    assert!(provider.errors.is_empty(), "{:?}", provider.errors);
}

#[test]
fn in_the_kernel_without_an_audio_device_the_behavior_finishes_on_its_second_run() {
    let (mut f, dialog, mut provider) = kernel_fx();
    marcus(&f, &mut dialog.borrow_mut());
    fire(&mut f, &mut provider, "OnTalk", &[]).unwrap();
    provider.tick(&mut f.vm, 0.05).unwrap();
    assert_eq!(f.outcomes(), ["no audio device"]);
    assert_eq!(
        provider.boundary,
        [
            boundary("OnTalk", kernel::OUT, "OutMain"),
            boundary("OnTalk", kernel::FINISHED, "FinishedMain")
        ]
    );
}

#[test]
fn in_the_kernel_force_play_immediate_finishes_and_outs_in_the_first_run() {
    let (mut f, dialog, mut provider) = kernel_fx();
    marcus(&f, &mut dialog.borrow_mut());
    dialog.borrow_mut().set_test_line_length(1.0);
    fire(&mut f, &mut provider, "OnNow", &[]).unwrap();
    assert_eq!(f.outcomes(), ["started"]);
    let mut got = provider.boundary.clone();
    got.sort();
    assert_eq!(
        got,
        [
            boundary("OnNow", kernel::FINISHED, "FinishedNow"),
            boundary("OnNow", kernel::OUT, "OutNow")
        ]
    );
    // Finished and Out in the first run: nothing waits.
    let runs = provider.trace.len();
    provider.tick(&mut f.vm, 1.0).unwrap();
    assert_eq!(provider.trace.len(), runs);
}

#[test]
fn in_the_kernel_a_behavior_without_a_tag_finishes_and_outs_at_once() {
    let (mut f, _dialog, mut provider) = kernel_fx();
    fire(&mut f, &mut provider, "OnBare", &[]).unwrap();
    assert_eq!(provider.boundary.len(), 2);
    assert!(f.lines.borrow().is_empty());
}

#[test]
fn in_the_kernel_a_dialog_error_is_the_kernel_s_error() {
    let (mut f, _dialog, mut provider) = kernel_fx();
    fire(&mut f, &mut provider, "OnNoGroup", &[]).unwrap();
    let e = provider.tick(&mut f.vm, 0.05).unwrap_err();
    assert_eq!(
        e.to_string(),
        "dialog: not implemented: a dialog event without a group (talker-owned events)"
    );
    assert!(matches!(e, BehaviorError::Data(_)));
}

#[test]
fn in_the_kernel_an_unlinked_behavior_finishes_while_its_line_plays() {
    let (mut f, dialog, mut provider) = kernel_fx();
    marcus(&f, &mut dialog.borrow_mut());
    dialog.borrow_mut().set_test_line_length(10.0);
    fire(&mut f, &mut provider, "OnUnlinked", &[]).unwrap();
    provider.tick(&mut f.vm, 0.05).unwrap();
    assert_eq!(f.outcomes(), ["started"]);
    // It selected Finished at once: no poll runs it again.
    let runs = provider.trace.len();
    provider.tick(&mut f.vm, 1.0).unwrap();
    assert_eq!(provider.trace.len(), runs);
}

#[test]
fn in_the_kernel_the_context_object_is_the_instigator_and_an_empty_context_does_not_run() {
    let (mut f, dialog, mut provider) = kernel_fx();
    // The speaker's pawn is the context object: it is the instigator, has no entry in Act_NoMatch, and its own
    // groups (none) cannot talk the Trigger act's event, so no line plays (from the mission, Marcus would speak).
    let actor = f.object(f.ids.p_high);
    let speaker = f.path(f.ids.speaker);
    dialog.borrow_mut().register_talker(&speaker, Some(actor));
    marcus(&f, &mut dialog.borrow_mut());
    dialog.borrow_mut().set_test_line_length(1.0);
    fire(&mut f, &mut provider, "OnCtx", &[Some(actor)]).unwrap();
    provider.tick(&mut f.vm, 0.05).unwrap();
    assert!(f.lines.borrow().is_empty(), "{:?}", f.outcomes());
    assert!(dialog.borrow().not_run().is_empty());
    // Fired with no object: the Context variable is empty, so the behavior does not run at all.
    let (mut f, dialog, mut provider) = kernel_fx();
    fire(&mut f, &mut provider, "OnCtx", &[]).unwrap();
    assert_eq!(dialog.borrow().not_run(), ["TalkCtx"]);
}

#[test]
fn a_dialog_error_keeps_its_kind_in_the_kernel() {
    let e: BehaviorError = DialogError::Vm(VmError::StepLimit).into();
    assert!(matches!(e, BehaviorError::Vm(VmError::StepLimit)));
    let e: BehaviorError = DialogError::Unresolved(None).into();
    assert_eq!(e.to_string(), "dialog: unresolved object reference");
}
