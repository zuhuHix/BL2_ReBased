//! Owner: lane W5-KISMET. Tests of the Kismet executor on invented packages (AGENTS.md rule 9), built with
//! `ow_package::testkit`. The fixture re-expresses `tests/kismet_test.py` (toy `Engine`, `WillowGame`, `Missions`,
//! `Other` and sequence packages carrying the public UE3 property names the executor reads): every toy class derives
//! from a toy `Engine.Object` and every object property names that class, because the Rust reflection layer refuses
//! an object property without a class (the C++ bag does not). Every case of the Python script has a test here with the
//! same expected values; the cases after them cover the Rust-only rules of the module
//! documentation ("Differences from the C++").

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_core::ParityMode;
use ow_object::decode::{DecodeState, ValueCx};
use ow_object::heap::ObjectSpace;
use ow_object::reflect::CodeIndex;
use ow_package::store::PackageStore;
use ow_package::testkit::reflect::{Declarations, property_payload};
use ow_package::testkit::{Export, PackageBuilder, ints};

use super::*;

// ---------------------------------------------------------------------------------------------------------------
// The invented packages
// ---------------------------------------------------------------------------------------------------------------

fn engine() -> Vec<u8> {
    let mut d = Declarations::new(&[
        "Int", "Float", "Bool", "Str", "Name", "Object", "Array", "Struct",
    ]);
    let object = d.cls("Object", 0);
    let s_in = d.struct_("SeqOpInputLink", None);
    d.prop("Str", s_in, "LinkDesc", 0);
    d.prop("Bool", s_in, "bDisabled", 0);
    d.prop("Float", s_in, "ActivateDelay", 0);
    let s_link = d.struct_("SeqOpOutputInputLink", None);
    d.prop("Object", s_link, "LinkedOp", object);
    d.prop("Int", s_link, "InputLinkIdx", 0);
    let s_out = d.struct_("SeqOpOutputLink", None);
    d.array_of(s_out, "Links", "Struct", s_link);
    d.prop("Str", s_out, "LinkDesc", 0);
    d.prop("Bool", s_out, "bDisabled", 0);
    d.prop("Float", s_out, "ActivateDelay", 0);
    let s_var = d.struct_("SeqVarLink", None);
    d.prop("Str", s_var, "LinkDesc", 0);
    d.array_of(s_var, "LinkedVariables", "Object", object);

    let op = d.cls("SequenceOp", object);
    d.array_of(op, "InputLinks", "Struct", s_in);
    d.array_of(op, "OutputLinks", "Struct", s_out);
    d.array_of(op, "VariableLinks", "Struct", s_var);
    d.prop("Int", op, "ActivateCount", 0);
    d.cls("Sequence", op);
    let event = d.cls("SequenceEvent", op);
    d.prop("Int", event, "TriggerCount", 0);
    d.prop("Object", event, "Originator", object);
    let remote = d.cls("SeqEvent_RemoteEvent", event);
    d.prop("Name", remote, "EventName", 0);
    let action = d.cls("SequenceAction", op);
    let activate = d.cls("SeqAct_ActivateRemoteEvent", action);
    d.prop("Name", activate, "EventName", 0);
    let gate = d.cls("SeqAct_Gate", action);
    d.prop("Bool", gate, "bOpen", 0);
    d.prop("Int", gate, "AutoCloseCount", 0);
    d.prop("Int", gate, "CurrentCloseCount", 0);
    let set_bool = d.cls("SeqAct_SetBool", action);
    d.prop("Bool", set_bool, "DefaultValue", 0);
    let compare = d.cls("SeqCond_CompareBool", op);
    d.prop("Bool", compare, "bResult", 0);
    let delay = d.cls("SeqAct_Delay", action);
    d.prop("Float", delay, "Duration", 0);
    d.cls("SeqAct_ToyWorldAction", action); // invented: an action no handler knows, so the host gets it
    d.cls("SeqCond_ToyUnhandled", op); // invented: a non-action op no handler knows
    d.cls("ToyBare", op); // invented: an op class declaring none of the logic ops' properties
    let var = d.cls("SeqVar_Bool", object);
    d.prop("Int", var, "bValue", 0);
    let var_float = d.cls("SeqVar_Float", object);
    d.prop("Float", var_float, "FloatValue", 0);
    d.p.build()
}

/// The mission-aware remote event class (derives the plain one) and its mission property.
fn willowgame() -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    let class = p.add_import("Core", "Class", 0, "Class");
    let object_property = p.add_import("Core", "Class", 0, "ObjectProperty");
    let engine = p.add_import("Core", "Package", 0, "Engine");
    let remote = p.add_import("Core", "Class", engine, "SeqEvent_RemoteEvent");
    let object = p.add_import("Core", "Class", engine, "Object");
    let none = p.fname("None", 0);
    let mut mission = Export::new(
        class,
        "WillowSeqEvent_MissionRemoteEvent",
        ints(&[0, 0, 0, 0]),
    );
    mission.super_ = remote;
    let mission = p.add_export_with(mission);
    let mut prop = Export::new(
        object_property,
        "AssociatedMissionDefinition",
        property_payload(&none, 1, 0, &ints(&[object])),
    );
    prop.outer = mission;
    p.add_export_with(prop);
    p.build()
}

/// A package of plain objects (class `Engine.Object`); only their identity is used.
fn plain(group: Option<&str>, leaves: &[&str]) -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    let engine = p.add_import("Core", "Package", 0, "Engine");
    let class = p.add_import("Core", "Class", engine, "Object");
    let none = p.fname("None", 0);
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&none);
    let outer = group.map_or(0, |g| p.add_export(class, g, payload.clone()));
    for leaf in leaves {
        let mut e = Export::new(class, leaf, payload.clone());
        e.outer = outer;
        p.add_export_with(e);
    }
    p.build()
}

// ---------------------------------------------------------------------------------------------------------------
// Sequence builder
// ---------------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Target {
    Op(&'static str),
    Ext(&'static [&'static str]),
    Null,
}

#[derive(Clone, Debug)]
struct Out {
    desc: &'static str,
    to: Vec<(Target, i32)>,
    disabled: bool,
    delay: f32,
}

fn out(desc: &'static str, to: &[(Target, i32)]) -> Out {
    Out {
        desc,
        to: to.to_vec(),
        disabled: false,
        delay: 0.0,
    }
}

#[derive(Clone, Debug)]
enum Scalar {
    Int(&'static str, i32),
    Float(&'static str, f32),
    Bool(&'static str, bool),
    Name(&'static str, &'static str),
    Obj(&'static str, Target),
}

#[derive(Clone, Debug)]
struct OpSpec {
    name: &'static str,
    cls: &'static str,
    inputs: Vec<(&'static str, bool, f32)>,
    outs: Vec<Out>,
    vars: Vec<(&'static str, Vec<&'static str>)>,
    scalars: Vec<Scalar>,
}

fn op(name: &'static str, cls: &'static str, inputs: &[&'static str], outs: Vec<Out>) -> OpSpec {
    OpSpec {
        name,
        cls,
        inputs: inputs.iter().map(|&d| (d, false, 0.0)).collect(),
        outs,
        vars: Vec::new(),
        scalars: Vec::new(),
    }
}

impl OpSpec {
    fn vars(mut self, vars: &[(&'static str, &[&'static str])]) -> Self {
        self.vars = vars.iter().map(|(d, v)| (*d, v.to_vec())).collect();
        self
    }
    fn scalars(mut self, scalars: &[Scalar]) -> Self {
        self.scalars = scalars.to_vec();
        self
    }
}

fn event(name: &'static str, event_name: &'static str, to: &[(Target, i32)]) -> OpSpec {
    op(name, "SeqEvent_RemoteEvent", &[], vec![out("Out", to)])
        .scalars(&[Scalar::Name("EventName", event_name)])
}

fn gate(name: &'static str, outs: Vec<Out>, open: bool, auto: i32) -> OpSpec {
    let mut scalars = vec![Scalar::Bool("bOpen", open)];
    if auto != 0 {
        scalars.push(Scalar::Int("AutoCloseCount", auto));
    }
    op(
        name,
        "SeqAct_Gate",
        &["In", "Open", "Close", "Toggle"],
        outs,
    )
    .scalars(&scalars)
}

fn world(name: &'static str) -> OpSpec {
    op(name, "SeqAct_ToyWorldAction", &["In"], Vec::new())
}

const IN: i32 = 0;
const OPEN: i32 = 1;
const CLOSE: i32 = 2;
const TOGGLE: i32 = 3;

fn t(name: &'static str, input: i32) -> (Target, i32) {
    (Target::Op(name), input)
}

struct SeqBuilder {
    p: PackageBuilder,
    chains: Vec<(Vec<&'static str>, i32)>,
    index: Vec<(&'static str, i32)>,
}

impl SeqBuilder {
    /// Import reference for `Package.Group.Object`, creating the outer chain once.
    fn chain(&mut self, path: &[&'static str]) -> i32 {
        if let Some((_, r)) = self.chains.iter().find(|(p, _)| p == path) {
            return *r;
        }
        let outer = if path.len() > 1 {
            self.chain(&path[..path.len() - 1])
        } else {
            0
        };
        let class = if path.len() == 1 { "Package" } else { "Class" };
        let r = self
            .p
            .add_import("Core", class, outer, path[path.len() - 1]);
        self.chains.push((path.to_vec(), r));
        r
    }

    fn reference(&mut self, target: &Target) -> i32 {
        match target {
            Target::Null => 0,
            Target::Ext(path) => self.chain(path),
            Target::Op(name) => self
                .index
                .iter()
                .find(|(n, _)| n == name)
                .map(|&(_, i)| i)
                .expect("a known op"),
        }
    }

    fn class_ref(&mut self, cls: &'static str) -> i32 {
        match cls.split_once('.') {
            Some(("WillowGame", c)) => self.chain(&["WillowGame", leak(c)]),
            _ => self.chain(&["Engine", cls]),
        }
    }
}

fn leak(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}

fn sequence(ops: &[OpSpec], variables: &[(&'static str, i32)]) -> Vec<u8> {
    let mut b = SeqBuilder {
        p: PackageBuilder::new_empty(),
        chains: Vec::new(),
        index: vec![("Seq", 1)],
    };
    let none = b.p.fname("None", 0);
    for (i, o) in ops.iter().enumerate() {
        b.index.push((o.name, i32::try_from(i).unwrap() + 2));
    }
    for (j, (name, _)) in variables.iter().enumerate() {
        b.index
            .push((name, i32::try_from(ops.len() + j).unwrap() + 2));
    }
    let seq_class = b.chain(&["Engine", "Sequence"]);
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&none);
    b.p.add_export(seq_class, "Seq", payload);
    for o in ops {
        let mut body = ints(&[0]); // the 4-byte prefix
        for s in &o.scalars {
            let tag = match s {
                Scalar::Int(n, v) => b.p.int_tag(n, *v),
                Scalar::Float(n, v) => b.p.float_tag(n, *v),
                Scalar::Bool(n, v) => b.p.bool_tag(n, *v),
                Scalar::Name(n, v) => b.p.name_tag(n, v),
                Scalar::Obj(n, target) => {
                    let r = b.reference(target);
                    b.p.object_tag(n, r)
                }
            };
            body.extend(tag);
        }
        if !o.inputs.is_empty() {
            let mut elements = Vec::new();
            for &(desc, disabled, delay) in &o.inputs {
                elements.extend(b.p.str_tag("LinkDesc", desc));
                if disabled {
                    elements.extend(b.p.bool_tag("bDisabled", true));
                }
                if delay != 0.0 {
                    elements.extend(b.p.float_tag("ActivateDelay", delay));
                }
                elements.extend(none);
            }
            body.extend(b.p.array_tag(
                "InputLinks",
                i32::try_from(o.inputs.len()).unwrap(),
                &elements,
            ));
        }
        if !o.outs.is_empty() {
            let mut elements = Vec::new();
            for link in &o.outs {
                let mut targets = Vec::new();
                for (target, input) in &link.to {
                    let r = b.reference(target);
                    targets.extend(b.p.object_tag("LinkedOp", r));
                    targets.extend(b.p.int_tag("InputLinkIdx", *input));
                    targets.extend(none);
                }
                elements.extend(b.p.array_tag(
                    "Links",
                    i32::try_from(link.to.len()).unwrap(),
                    &targets,
                ));
                elements.extend(b.p.str_tag("LinkDesc", link.desc));
                if link.disabled {
                    elements.extend(b.p.bool_tag("bDisabled", true));
                }
                if link.delay != 0.0 {
                    elements.extend(b.p.float_tag("ActivateDelay", link.delay));
                }
                elements.extend(none);
            }
            body.extend(b.p.array_tag(
                "OutputLinks",
                i32::try_from(o.outs.len()).unwrap(),
                &elements,
            ));
        }
        if !o.vars.is_empty() {
            let mut elements = Vec::new();
            for (desc, names) in &o.vars {
                elements.extend(b.p.str_tag("LinkDesc", desc));
                let mut refs = Vec::new();
                for n in names {
                    refs.extend(b.reference(&Target::Op(n)).to_le_bytes());
                }
                elements.extend(b.p.array_tag(
                    "LinkedVariables",
                    i32::try_from(names.len()).unwrap(),
                    &refs,
                ));
                elements.extend(none);
            }
            body.extend(b.p.array_tag(
                "VariableLinks",
                i32::try_from(o.vars.len()).unwrap(),
                &elements,
            ));
        }
        body.extend(none);
        let class = b.class_ref(o.cls);
        let mut e = Export::new(class, o.name, body);
        e.outer = 1;
        b.p.add_export_with(e);
    }
    for (name, value) in variables {
        let (class, field) = if name.starts_with("Float") {
            (
                b.class_ref("SeqVar_Float"),
                b.p.float_tag("FloatValue", *value as f32),
            )
        } else {
            (b.class_ref("SeqVar_Bool"), b.p.int_tag("bValue", *value))
        };
        let mut body = ints(&[0]);
        body.extend(field);
        body.extend(none);
        let mut e = Export::new(class, name, body);
        e.outer = 1;
        b.p.add_export_with(e);
    }
    b.p.build()
}

// ---------------------------------------------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------------------------------------------

static TREES: AtomicU32 = AtomicU32::new(0);

/// A temporary cooked folder of the invented packages, removed on drop.
struct Tree(PathBuf);

impl Tree {
    fn new(ops: &[OpSpec], variables: &[(&'static str, i32)]) -> Tree {
        let n = TREES.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("ow-mission-kismet-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Engine.upk"), engine()).unwrap();
        std::fs::write(dir.join("WillowGame.upk"), willowgame()).unwrap();
        std::fs::write(dir.join("Other.upk"), plain(None, &["Elsewhere"])).unwrap();
        std::fs::write(
            dir.join("Missions.upk"),
            plain(Some("ToyGroup"), &["ToyMission", "OtherMission"]),
        )
        .unwrap();
        std::fs::write(dir.join("TestSeq.upk"), sequence(ops, variables)).unwrap();
        Tree(dir)
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The decode context's parts over one tree.
struct Parts {
    _tree: Tree,
    store: PackageStore,
    code: CodeIndex,
    space: ObjectSpace,
    state: DecodeState,
}

impl Parts {
    fn new(ops: &[OpSpec], variables: &[(&'static str, i32)]) -> Parts {
        let tree = Tree::new(ops, variables);
        let store = PackageStore::open(tree.0.clone()).unwrap();
        Parts {
            _tree: tree,
            store,
            code: CodeIndex::new(),
            space: ObjectSpace::new(),
            state: DecodeState::new(),
        }
    }
    fn cx(&mut self) -> ValueCx<'_> {
        ValueCx::new(
            &mut self.store,
            &mut self.code,
            &mut self.space,
            &mut self.state,
            ParityMode::Off,
        )
    }
}

#[derive(Debug, Clone, Copy)]
enum Entry {
    Remote(&'static str),
    Mission(&'static str, &'static str),
    Op(&'static str),
    Originator(&'static str),
}

/// What `--kismet-run` prints, and its exit code (0 when no error line was written and the entry matched).
#[derive(Debug, PartialEq)]
struct Outcome {
    ops: usize,
    matched: usize,
    executed: usize,
    trace: Vec<String>,
    host: Vec<String>,
    errors: Vec<String>,
    log: Vec<String>,
    code: i32,
}

/// Loads `Seq` with a host binding on `Engine.SequenceAction` (as the CLI does) and hands it to `body`.
fn with_kismet<R>(
    ops: &[OpSpec],
    variables: &[(&'static str, i32)],
    body: impl FnOnce(&mut Kismet, &mut ValueCx<'_>, &Rc<RefCell<Vec<String>>>) -> R,
) -> R {
    let mut parts = Parts::new(ops, variables);
    let mut cx = parts.cx();
    let package = cx.code.package(cx.store, "TestSeq").unwrap();
    let mut kismet = Kismet::new(&mut cx, package, "Seq").unwrap();
    let host: Rc<RefCell<Vec<String>>> = Rc::default();
    let calls = Rc::clone(&host);
    kismet.handle(
        "Engine.SequenceAction",
        Rc::new(
            move |k: &mut Kismet, cx: &mut ValueCx<'_>, op: usize, input: i32| {
                let desc = k.input_desc(cx, op, input)?;
                calls.borrow_mut().push(format!(
                    "{}:{} <- {desc}",
                    k.ops()[op].cls,
                    k.ops()[op].name
                ));
                Ok(())
            },
        ),
    );
    body(&mut kismet, &mut cx, &host)
}

fn enter(k: &mut Kismet, cx: &mut ValueCx<'_>, entry: Entry) -> usize {
    match entry {
        Entry::Remote(name) => k.remote_event(cx, name).unwrap(),
        Entry::Mission(path, name) => k.mission_remote_event(cx, path, name).unwrap(),
        Entry::Op(name) => match k.find(name) {
            Some(op) => {
                k.activate_event(cx, op).unwrap();
                1
            }
            None => 0,
        },
        Entry::Originator(path) => {
            let events = k.events_for_originator(cx, path).unwrap();
            for &op in &events {
                k.activate_event(cx, op).unwrap();
            }
            events.len()
        }
    }
}

fn kismet(
    ops: &[OpSpec],
    variables: &[(&'static str, i32)],
    entry: Entry,
    ticks: &[f64],
) -> Outcome {
    with_kismet(ops, variables, |k, cx, host| {
        let matched = enter(k, cx, entry);
        k.run(cx).unwrap();
        for &s in ticks {
            k.tick(cx, s).unwrap();
        }
        Outcome {
            ops: k.ops().len(),
            matched,
            executed: k.executed,
            trace: k.trace.clone(),
            host: host.borrow().clone(),
            errors: k.errors.clone(),
            log: cx.state.log.clone(),
            code: i32::from(!(k.errors.is_empty() && matched > 0)),
        }
    })
}

fn lines(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_string()).collect()
}

fn host(names: &[&str]) -> Vec<String> {
    names
        .iter()
        .map(|n| format!("Engine.SeqAct_ToyWorldAction:{n} <- In"))
        .collect()
}

/// Whole-outcome comparison: fields not given are empty (lists), 1 (`matched`) or 0 (`code`).
#[allow(clippy::too_many_arguments)]
fn expect(
    got: &Outcome,
    ops: usize,
    matched: usize,
    executed: usize,
    trace: &[&str],
    host_lines: Vec<String>,
    errors: &[&str],
    code: i32,
) {
    let want = Outcome {
        ops,
        matched,
        executed,
        trace: lines(trace),
        host: host_lines,
        errors: lines(errors),
        log: Vec::new(),
        code,
    };
    assert_eq!(got, &want);
}

fn chain_ops() -> Vec<OpSpec> {
    vec![
        event("Ev", "Ping", &[t("G1", IN)]),
        gate(
            "G1",
            vec![out("Out", &[t("Act1", 0), t("Act2", 0)])],
            true,
            0,
        ),
        world("Act1"),
        world("Act2"),
        event("Idle", "Nope", &[t("Act3", 0)]),
        world("Act3"),
    ]
}

// ---------------------------------------------------------------------------------------------------------------
// The cases of tests/kismet_test.py
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn case_1a_remote_event_propagates_through_decoded_links_in_order() {
    let got = kismet(&chain_ops(), &[], Entry::Remote("Ping"), &[]);
    expect(
        &got,
        6,
        1,
        4,
        &[
            "event Ev",
            "Ev output 0 -> 1 link(s)",
            "G1 <- In",
            "G1 output 0 -> 2 link(s)",
            "Act1 <- In",
            "Act2 <- In",
        ],
        host(&["Act1", "Act2"]),
        &[],
        0,
    );
}

#[test]
fn case_1b_activate_remote_event_reenters_and_its_own_links_run_first() {
    let ops = vec![
        event("Ev", "Go", &[t("Hop", 0)]),
        op(
            "Hop",
            "SeqAct_ActivateRemoteEvent",
            &["In"],
            vec![out("Out", &[t("Act1", 0)])],
        )
        .scalars(&[Scalar::Name("EventName", "Pong")]),
        event("Ev2", "Pong", &[t("Act2", 0)]),
        world("Act1"),
        world("Act2"),
    ];
    let got = kismet(&ops, &[], Entry::Remote("Go"), &[]);
    expect(
        &got,
        5,
        1,
        5,
        &[
            "event Ev",
            "Ev output 0 -> 1 link(s)",
            "Hop <- In",
            "Hop remote 'Pong'",
            "event Ev2",
            "Hop output 0 -> 1 link(s)",
            "Act1 <- In",
            "Ev2 output 0 -> 1 link(s)",
            "Act2 <- In",
        ],
        host(&["Act1", "Act2"]),
        &[],
        0,
    );
}

#[test]
fn case_1c_op_entry_and_the_full_sequence_path() {
    let ops = chain_ops();
    let mut parts = Parts::new(&ops, &[]);
    let mut cx = parts.cx();
    let package = cx.code.package(cx.store, "TestSeq").unwrap();
    // The package prefix is dropped, as the C++ findExport does.
    let mut k = Kismet::new(&mut cx, package, "TestSeq.Seq").unwrap();
    k.handle("Engine.SequenceAction", Rc::new(|_, _, _, _| Ok(())));
    let ev = k.find("Ev").unwrap();
    k.activate_event(&mut cx, ev).unwrap();
    k.run(&mut cx).unwrap();
    assert_eq!(k.executed, 4);
    assert_eq!(
        k.trace,
        lines(&[
            "event Ev",
            "Ev output 0 -> 1 link(s)",
            "G1 <- In",
            "G1 output 0 -> 2 link(s)",
            "Act1 <- In",
            "Act2 <- In"
        ])
    );
}

#[test]
fn case_1d_unknown_op_matches_nothing() {
    let got = kismet(&chain_ops(), &[], Entry::Op("Missing"), &[]);
    expect(&got, 6, 0, 0, &[], Vec::new(), &[], 1);
}

#[test]
fn case_2a_delayed_impulses_do_not_run_at_time_zero() {
    let ops = vec![
        event("Ev", "Ping", &[t("Del", 0), t("G", IN)]),
        op(
            "Del",
            "SeqAct_Delay",
            &["Start", "Stop"],
            vec![out("Finished", &[t("ActDone", 0)])],
        )
        .scalars(&[Scalar::Float("Duration", 2.5)]),
        gate(
            "G",
            vec![Out {
                delay: 1.5,
                ..out("Out", &[t("ActLate", 0)])
            }],
            true,
            0,
        ),
        world("ActDone"),
        world("ActLate"),
    ];
    let got = kismet(&ops, &[], Entry::Remote("Ping"), &[]);
    expect(
        &got,
        5,
        1,
        3,
        &[
            "event Ev",
            "Ev output 0 -> 2 link(s)",
            "Del <- Start",
            "G <- In",
            "G output 0 -> 1 link(s)",
        ],
        Vec::new(),
        &[],
        0,
    );
    // Rust-only: the ticks the CLI cannot reach from the Python script. The gate's link is due at 1.5 s, the delay's
    // completion at 2.5 s.
    let got = kismet(&ops, &[], Entry::Remote("Ping"), &[1.5]);
    assert_eq!(got.host, host(&["ActLate"]));
    let got = kismet(&ops, &[], Entry::Remote("Ping"), &[1.5, 1.0]);
    assert_eq!(got.host, host(&["ActLate", "ActDone"]));
    assert_eq!(got.executed, 6);
    assert_eq!(
        &got.trace[5..],
        &lines(&[
            "ActLate <- In",
            "Del finished",
            "Del output 0 -> 1 link(s)",
            "ActDone <- In"
        ])
    );
}

#[test]
fn case_2b_zero_duration_delay_completes_in_the_same_run() {
    let ops = vec![
        event("Ev", "Ping", &[t("D0", 0)]),
        op(
            "D0",
            "SeqAct_Delay",
            &["Start", "Stop"],
            vec![out("Finished", &[t("Act", 0)])],
        ),
        world("Act"),
    ];
    let got = kismet(&ops, &[], Entry::Remote("Ping"), &[]);
    expect(
        &got,
        3,
        1,
        4,
        &[
            "event Ev",
            "Ev output 0 -> 1 link(s)",
            "D0 <- Start",
            "D0 finished",
            "D0 output 0 -> 1 link(s)",
            "Act <- In",
        ],
        host(&["Act"]),
        &[],
        0,
    );
}

fn gate_case(gate_op: OpSpec, inputs: &[i32], tail: &[&str], hosts: usize, executed: usize) {
    let to: Vec<(Target, i32)> = inputs.iter().map(|&i| t("G", i)).collect();
    let ops = vec![event("Ev", "Ping", &to), gate_op, world("Act")];
    let got = kismet(&ops, &[], Entry::Remote("Ping"), &[]);
    let head = format!("Ev output 0 -> {} link(s)", inputs.len());
    let mut trace = vec!["event Ev", head.as_str()];
    trace.extend_from_slice(tail);
    expect(
        &got,
        3,
        1,
        executed,
        &trace,
        host(&vec!["Act"; hosts]),
        &[],
        0,
    );
}

fn through() -> Vec<Out> {
    vec![out("Out", &[t("Act", 0)])]
}

#[test]
fn case_3_gate_inputs() {
    gate_case(gate("G", through(), false, 0), &[IN], &["G <- In"], 0, 2);
    gate_case(
        gate("G", through(), false, 0),
        &[OPEN, IN],
        &[
            "G <- Open",
            "G <- In",
            "G output 0 -> 1 link(s)",
            "Act <- In",
        ],
        1,
        4,
    );
    gate_case(
        gate("G", through(), false, 0),
        &[IN, OPEN],
        &["G <- In", "G <- Open"],
        0,
        3,
    );
    gate_case(
        gate("G", through(), false, 0),
        &[TOGGLE, IN],
        &[
            "G <- Toggle",
            "G <- In",
            "G output 0 -> 1 link(s)",
            "Act <- In",
        ],
        1,
        4,
    );
    gate_case(
        gate("G", through(), true, 0),
        &[CLOSE, IN],
        &["G <- Close", "G <- In"],
        0,
        3,
    );
    gate_case(
        gate("G", through(), true, 0),
        &[TOGGLE, IN],
        &["G <- Toggle", "G <- In"],
        0,
        3,
    );
    gate_case(
        gate("G", through(), true, 0),
        &[TOGGLE, TOGGLE, IN],
        &[
            "G <- Toggle",
            "G <- Toggle",
            "G <- In",
            "G output 0 -> 1 link(s)",
            "Act <- In",
        ],
        1,
        5,
    );
}

#[test]
fn case_3g_auto_close_count_closes_after_two_passes() {
    gate_case(
        gate("G", through(), true, 2),
        &[IN, IN, IN],
        &[
            "G <- In",
            "G output 0 -> 1 link(s)",
            "Act <- In",
            "G <- In",
            "G output 0 -> 1 link(s)",
            "Act <- In",
            "G <- In",
        ],
        2,
        6,
    );
}

fn setbool_case(
    link_value: Option<bool>,
    default: bool,
    target_initial: bool,
    expected_true: bool,
) {
    let variables = [
        ("VarSrc", i32::from(link_value == Some(true))),
        ("VarDst", i32::from(target_initial)),
    ];
    let mut sb_vars: Vec<(&'static str, &[&'static str])> = Vec::new();
    if link_value.is_some() {
        sb_vars.push(("Value", &["VarSrc"]));
    }
    sb_vars.push(("Target", &["VarDst"]));
    let ops = vec![
        event("Ev", "Ping", &[t("SB", 0)]),
        op(
            "SB",
            "SeqAct_SetBool",
            &["In"],
            vec![out("Out", &[t("Cmp", 0)])],
        )
        .vars(&sb_vars)
        .scalars(&[Scalar::Bool("DefaultValue", default)]),
        op(
            "Cmp",
            "SeqCond_CompareBool",
            &["In"],
            vec![out("True", &[t("ActT", 0)]), out("False", &[t("ActF", 0)])],
        )
        .vars(&[("Bool", &["VarDst"])]),
        world("ActT"),
        world("ActF"),
    ];
    let got = kismet(&ops, &variables, Entry::Remote("Ping"), &[]);
    let mut trace = vec![
        "event Ev",
        "Ev output 0 -> 1 link(s)",
        "SB <- In",
        "SB output 0 -> 1 link(s)",
        "Cmp <- In",
    ];
    if expected_true {
        trace.extend(["Cmp output 0 -> 1 link(s)", "ActT <- In"]);
    } else {
        trace.extend(["Cmp output 1 -> 1 link(s)", "ActF <- In"]);
    }
    expect(
        &got,
        5,
        1,
        4,
        &trace,
        host(&[if expected_true { "ActT" } else { "ActF" }]),
        &[],
        0,
    );
}

#[test]
fn case_4a_to_4d_set_bool_and_compare_bool() {
    setbool_case(Some(true), false, false, true);
    setbool_case(Some(false), true, true, false);
    setbool_case(None, true, false, true);
    setbool_case(None, false, true, false);
}

#[test]
fn case_4e_set_bool_without_target_is_an_error() {
    let ops = vec![
        event("Ev", "Ping", &[t("SB", 0)]),
        op(
            "SB",
            "SeqAct_SetBool",
            &["In"],
            vec![out("Out", &[t("Act", 0)])],
        ),
        world("Act"),
    ];
    let got = kismet(&ops, &[], Entry::Remote("Ping"), &[]);
    expect(
        &got,
        3,
        1,
        2,
        &["event Ev", "Ev output 0 -> 1 link(s)", "SB <- In"],
        Vec::new(),
        &["SB: SetBool without a Target"],
        1,
    );
}

#[test]
fn case_4f_compare_bool_without_bool_is_an_error() {
    let ops = vec![
        event("Ev", "Ping", &[t("Cmp", 0)]),
        op(
            "Cmp",
            "SeqCond_CompareBool",
            &["In"],
            vec![out("True", &[t("Act", 0)])],
        ),
        world("Act"),
    ];
    let got = kismet(&ops, &[], Entry::Remote("Ping"), &[]);
    expect(
        &got,
        3,
        1,
        2,
        &["event Ev", "Ev output 0 -> 1 link(s)", "Cmp <- In"],
        Vec::new(),
        &["Cmp: CompareBool without a Bool variable"],
        1,
    );
}

#[test]
fn case_5_unsupported_op_is_reported_and_nothing_downstream_runs() {
    let ops = vec![
        event("Ev", "Ping", &[t("W", 0), t("U", 0)]),
        world("W"),
        op(
            "U",
            "SeqCond_ToyUnhandled",
            &["In"],
            vec![out("Out", &[t("After", 0)])],
        ),
        world("After"),
    ];
    let got = kismet(&ops, &[], Entry::Remote("Ping"), &[]);
    expect(
        &got,
        4,
        1,
        3,
        &["event Ev", "Ev output 0 -> 2 link(s)", "W <- In", "U <- In"],
        host(&["W"]),
        &["unsupported op class Engine.SeqCond_ToyUnhandled (U)"],
        1,
    );
}

#[test]
fn case_6_disabled_output_is_traced_and_does_not_propagate() {
    let control = kismet(
        &[event("Ev", "Ping", &[t("Act", 0)]), world("Act")],
        &[],
        Entry::Remote("Ping"),
        &[],
    );
    assert_eq!(control.trace.last().map(String::as_str), Some("Act <- In"));
    let ops = vec![
        op(
            "Ev",
            "SeqEvent_RemoteEvent",
            &[],
            vec![Out {
                disabled: true,
                ..out("Out", &[t("Act", 0)])
            }],
        )
        .scalars(&[Scalar::Name("EventName", "Ping")]),
        world("Act"),
    ];
    let got = kismet(&ops, &[], Entry::Remote("Ping"), &[]);
    expect(
        &got,
        2,
        1,
        1,
        &["event Ev", "Ev output 0 disabled"],
        Vec::new(),
        &[],
        0,
    );
}

#[test]
fn case_7_links_outside_the_sequence() {
    let ops = vec![
        event(
            "Ev",
            "Ping",
            &[(Target::Ext(&["Other", "Elsewhere"]), 0), t("Act", 0)],
        ),
        world("Act"),
    ];
    let got = kismet(&ops, &[], Entry::Remote("Ping"), &[]);
    expect(
        &got,
        2,
        1,
        2,
        &["event Ev", "Ev output 0 -> 2 link(s)", "Act <- In"],
        host(&["Act"]),
        &["Ev: linked op is outside this sequence: Elsewhere"],
        1,
    );
    let ops = vec![event("Ev", "Ping", &[t("Var", 0)]), world("Act")];
    let got = kismet(&ops, &[("Var", 0)], Entry::Remote("Ping"), &[]);
    expect(
        &got,
        2,
        1,
        1,
        &["event Ev", "Ev output 0 -> 1 link(s)"],
        Vec::new(),
        &["Ev: linked op is outside this sequence: Var"],
        1,
    );
    let ops = vec![event("Ev", "Ping", &[(Target::Null, 0)]), world("Act")];
    let got = kismet(&ops, &[], Entry::Remote("Ping"), &[]);
    expect(
        &got,
        2,
        1,
        1,
        &["event Ev", "Ev output 0 -> 1 link(s)"],
        Vec::new(),
        &[],
        0,
    );
}

#[test]
fn case_8_unknown_remote_event() {
    let got = kismet(&chain_ops(), &[], Entry::Remote("Nobody"), &[]);
    expect(&got, 6, 0, 0, &[], Vec::new(), &[], 1);
}

#[test]
fn case_9_a_ring_stops_at_the_per_run_limit() {
    let ops = vec![
        event("Ev", "Ping", &[t("GA", IN)]),
        gate("GA", vec![out("Out", &[t("GB", IN)])], true, 0),
        gate("GB", vec![out("Out", &[t("GA", IN)])], true, 0),
    ];
    let got = kismet(&ops, &[], Entry::Remote("Ping"), &[]);
    assert_eq!(got.code, 1);
    assert_eq!(got.errors, lines(&["kismet execution limit exceeded"]));
    assert_eq!(got.executed, 1000);
    assert_eq!(got.trace.len(), 2 + 2 * 999);
    assert_eq!(
        &got.trace[..6],
        &lines(&[
            "event Ev",
            "Ev output 0 -> 1 link(s)",
            "GA <- In",
            "GA output 0 -> 1 link(s)",
            "GB <- In",
            "GB output 0 -> 1 link(s)"
        ])
    );
    assert!(got.log.is_empty());
}

const TOY_MISSION: &[&str] = &["Missions", "ToyGroup", "ToyMission"];
const OTHER_MISSION: &[&str] = &["Missions", "ToyGroup", "OtherMission"];

fn mission_event(
    name: &'static str,
    mission: &'static [&'static str],
    to: &[(Target, i32)],
) -> OpSpec {
    op(
        name,
        "WillowGame.WillowSeqEvent_MissionRemoteEvent",
        &[],
        vec![out("Out", to)],
    )
    .scalars(&[
        Scalar::Name("EventName", "Go"),
        Scalar::Obj("AssociatedMissionDefinition", Target::Ext(mission)),
    ])
}

fn mission_ops() -> Vec<OpSpec> {
    vec![
        mission_event("MEv", TOY_MISSION, &[t("ActM", 0)]),
        mission_event("OEv", OTHER_MISSION, &[t("ActO", 0)]),
        event("REv", "Go", &[t("ActR", 0)]),
        world("ActM"),
        world("ActO"),
        world("ActR"),
    ]
}

#[test]
fn case_10_mission_remote_events_match_their_mission_and_name() {
    let got = kismet(
        &mission_ops(),
        &[],
        Entry::Mission("ToyGroup.ToyMission", "Go"),
        &[],
    );
    expect(
        &got,
        6,
        1,
        2,
        &["event MEv", "MEv output 0 -> 1 link(s)", "ActM <- In"],
        host(&["ActM"]),
        &[],
        0,
    );
    let got = kismet(
        &mission_ops(),
        &[],
        Entry::Mission("ToyGroup.OtherMission", "Go"),
        &[],
    );
    expect(
        &got,
        6,
        1,
        2,
        &["event OEv", "OEv output 0 -> 1 link(s)", "ActO <- In"],
        host(&["ActO"]),
        &[],
        0,
    );
    let got = kismet(
        &mission_ops(),
        &[],
        Entry::Mission("ToyGroup.NoSuchMission", "Go"),
        &[],
    );
    expect(&got, 6, 0, 0, &[], Vec::new(), &[], 1);
    let got = kismet(
        &mission_ops(),
        &[],
        Entry::Mission("ToyGroup.ToyMission", "Stop"),
        &[],
    );
    expect(&got, 6, 0, 0, &[], Vec::new(), &[], 1);
    // A plain remote activation does not fire mission events that share the event name.
    let got = kismet(&mission_ops(), &[], Entry::Remote("Go"), &[]);
    expect(
        &got,
        6,
        1,
        2,
        &["event REv", "REv output 0 -> 1 link(s)", "ActR <- In"],
        host(&["ActR"]),
        &[],
        0,
    );
}

#[test]
fn case_11_link_census() {
    let variables = [("VarSrc", 1), ("VarDst", 0)];
    let ops = vec![
        event("Ev", "Ping", &[t("SB", 0)]),
        op(
            "SB",
            "SeqAct_SetBool",
            &["In"],
            vec![out("Out", &[t("Cmp", 0)])],
        )
        .vars(&[("Value", &["VarSrc"]), ("Target", &["VarDst"])]),
        op(
            "Cmp",
            "SeqCond_CompareBool",
            &["In"],
            vec![
                out("True", &[t("ActT", 0)]),
                out(
                    "False",
                    &[t("ActF", 0), (Target::Ext(&["Other", "Elsewhere"]), 0)],
                ),
            ],
        )
        .vars(&[("Bool", &["VarDst"])]),
        world("ActT"),
        world("ActF"),
    ];
    with_kismet(&ops, &variables, |k, cx, _| {
        assert_eq!(k.ops().len(), 5);
        assert_eq!(
            k.link_stats(cx).unwrap(),
            LinkStats {
                outputs: 4,
                links: 5,
                unresolved: 1,
                variable_links: 3
            }
        );
        assert!(cx.state.log.is_empty());
    });
}

#[test]
fn case_12_a_missing_sequence_is_an_error() {
    let mut parts = Parts::new(&chain_ops(), &[]);
    let mut cx = parts.cx();
    let package = cx.code.package(cx.store, "TestSeq").unwrap();
    let e = Kismet::new(&mut cx, package, "NoSuchSeq").unwrap_err();
    assert!(matches!(e, KismetError::SequenceNotFound(_)));
    assert_eq!(e.to_string(), "kismet sequence not found: NoSuchSeq");
    assert!(std::error::Error::source(&e).is_none());
}

fn origin_ops() -> Vec<OpSpec> {
    let elsewhere = Target::Ext(&["Other", "Elsewhere"]);
    vec![
        op(
            "P1",
            "SequenceEvent",
            &[],
            vec![out("Out", &[t("Act1", 0)])],
        )
        .scalars(&[Scalar::Obj("Originator", elsewhere.clone())]),
        op(
            "P2",
            "SequenceEvent",
            &[],
            vec![out("Out", &[t("Act2", 0)])],
        )
        .scalars(&[Scalar::Obj("Originator", elsewhere.clone())]),
        op(
            "P3",
            "SequenceEvent",
            &[],
            vec![out("Out", &[t("Act3", 0)])],
        )
        .scalars(&[Scalar::Obj("Originator", Target::Ext(TOY_MISSION))]),
        op("NotEvent", "SeqAct_ToyWorldAction", &["In"], Vec::new())
            .scalars(&[Scalar::Obj("Originator", elsewhere)]),
        world("Act1"),
        world("Act2"),
        world("Act3"),
    ]
}

#[test]
fn case_13_originator_events() {
    // Both events are queued before the run; the most recently queued (P2) runs first.
    let got = kismet(&origin_ops(), &[], Entry::Originator("Elsewhere"), &[]);
    expect(
        &got,
        7,
        2,
        4,
        &[
            "event P1",
            "event P2",
            "P2 output 0 -> 1 link(s)",
            "Act2 <- In",
            "P1 output 0 -> 1 link(s)",
            "Act1 <- In",
        ],
        host(&["Act2", "Act1"]),
        &[],
        0,
    );
    let got = kismet(
        &origin_ops(),
        &[],
        Entry::Originator("ToyGroup.ToyMission"),
        &[],
    );
    expect(
        &got,
        7,
        1,
        2,
        &["event P3", "P3 output 0 -> 1 link(s)", "Act3 <- In"],
        host(&["Act3"]),
        &[],
        0,
    );
    let got = kismet(&origin_ops(), &[], Entry::Originator("Nowhere"), &[]);
    expect(&got, 7, 0, 0, &[], Vec::new(), &[], 1);
}

#[test]
fn case_14a_input_delay_adds_to_output_delay() {
    let mut act = op("Act", "SeqAct_ToyWorldAction", &[], Vec::new());
    act.inputs = vec![("In", false, 0.5)];
    let ops = vec![
        op(
            "Ev",
            "SeqEvent_RemoteEvent",
            &[],
            vec![Out {
                delay: 1.0,
                ..out("Out", &[t("Act", 0)])
            }],
        )
        .scalars(&[Scalar::Name("EventName", "Ping")]),
        act,
    ];
    let got = kismet(&ops, &[], Entry::Remote("Ping"), &[1.4]);
    expect(
        &got,
        2,
        1,
        1,
        &["event Ev", "Ev output 0 -> 1 link(s)"],
        Vec::new(),
        &[],
        0,
    );
    let got = kismet(&ops, &[], Entry::Remote("Ping"), &[1.4, 0.2]);
    expect(
        &got,
        2,
        1,
        2,
        &["event Ev", "Ev output 0 -> 1 link(s)", "Act <- In"],
        host(&["Act"]),
        &[],
        0,
    );
}

#[test]
fn case_14b_a_disabled_input_receives_nothing() {
    let mut act = op("Act", "SeqAct_ToyWorldAction", &[], Vec::new());
    act.inputs = vec![("In", true, 0.0)];
    let got = kismet(
        &[event("Ev", "Ping", &[t("Act", 0)]), act],
        &[],
        Entry::Remote("Ping"),
        &[],
    );
    expect(
        &got,
        2,
        1,
        1,
        &[
            "event Ev",
            "Ev output 0 -> 1 link(s)",
            "Act input 0 disabled",
        ],
        Vec::new(),
        &[],
        0,
    );
}

#[test]
fn case_14c_depth_first_in_link_order() {
    let ops = vec![
        event("Ev", "Ping", &[t("GA", IN), t("GB", IN)]),
        gate("GA", vec![out("Out", &[t("A2", 0)])], true, 0),
        gate("GB", vec![out("Out", &[t("B2", 0)])], true, 0),
        world("A2"),
        world("B2"),
    ];
    let got = kismet(&ops, &[], Entry::Remote("Ping"), &[]);
    expect(
        &got,
        5,
        1,
        5,
        &[
            "event Ev",
            "Ev output 0 -> 2 link(s)",
            "GA <- In",
            "GA output 0 -> 1 link(s)",
            "A2 <- In",
            "GB <- In",
            "GB output 0 -> 1 link(s)",
            "B2 <- In",
        ],
        host(&["A2", "B2"]),
        &[],
        0,
    );
}

#[test]
fn case_14d_an_input_hit_twice_runs_twice() {
    let ops = vec![
        event("Ev", "Ping", &[t("G", IN), t("Act", 0)]),
        gate("G", vec![out("Out", &[t("Act", 0)])], true, 0),
        world("Act"),
    ];
    let got = kismet(&ops, &[], Entry::Remote("Ping"), &[]);
    expect(
        &got,
        3,
        1,
        4,
        &[
            "event Ev",
            "Ev output 0 -> 2 link(s)",
            "G <- In",
            "G output 0 -> 1 link(s)",
            "Act <- In",
            "Act <- In",
        ],
        host(&["Act", "Act"]),
        &[],
        0,
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Rust-side rules
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn counts_are_written_in_their_own_kind() {
    let ops = vec![event("Ev", "Ping", &[t("Act", 0)]), world("Act")];
    with_kismet(&ops, &[], |k, cx, _| {
        enter(k, cx, Entry::Remote("Ping"));
        k.run(cx).unwrap();
        let ev = k.find("Ev").unwrap();
        let act = k.find("Act").unwrap();
        assert_eq!(k.prop(cx, ev, "TriggerCount").unwrap(), Some(Value::Int(1)));
        // The event ran through "fire all outputs", not execute: ActivateCount counts executed inputs only.
        assert_eq!(
            k.prop(cx, ev, "ActivateCount").unwrap(),
            Some(Value::Int(0))
        );
        assert_eq!(
            k.prop(cx, act, "ActivateCount").unwrap(),
            Some(Value::Int(1))
        );
    });
}

#[test]
fn a_count_at_its_limit_is_an_error_line_and_stays() {
    let ops = vec![event("Ev", "Ping", &[]).scalars(&[
        Scalar::Name("EventName", "Ping"),
        Scalar::Int("TriggerCount", i32::MAX),
    ])];
    with_kismet(&ops, &[], |k, cx, _| {
        enter(k, cx, Entry::Remote("Ping"));
        assert_eq!(k.errors, lines(&["Ev: TriggerCount cannot count further"]));
        let ev = k.find("Ev").unwrap();
        assert_eq!(
            k.prop(cx, ev, "TriggerCount").unwrap(),
            Some(Value::Int(i32::MAX))
        );
    });
}

#[test]
fn a_logic_op_on_a_class_without_its_property_is_an_error_line() {
    let ops = vec![
        event("Ev", "Ping", &[t("B", 0)]),
        op("B", "ToyBare", &["Start"], vec![out("Out", &[])]),
    ];
    with_kismet(&ops, &[], |k, cx, _| {
        k.handle("Engine.ToyBare", Rc::new(super::gate));
        enter(k, cx, Entry::Remote("Ping"));
        k.run(cx).unwrap();
        assert_eq!(k.errors, lines(&["B: Engine.ToyBare without bOpen"]));
        for handler in [
            super::set_bool
                as fn(&mut Kismet, &mut ValueCx<'_>, usize, i32) -> Result<(), KismetError>,
            super::delay,
        ] {
            k.errors.clear();
            k.handle("Engine.ToyBare", Rc::new(handler));
            let b = k.find("B").unwrap();
            k.activate(cx, b, 0, 0.0).unwrap();
            k.run(cx).unwrap();
            assert_eq!(k.errors.len(), 1, "{:?}", k.errors);
            assert!(
                k.errors[0].starts_with("B: Engine.ToyBare without "),
                "{:?}",
                k.errors
            );
        }
    });
}

#[test]
fn set_bool_writes_a_float_variable_as_a_float() {
    let ops = vec![
        event("Ev", "Ping", &[t("SB", 0)]),
        op("SB", "SeqAct_SetBool", &["In"], vec![out("Out", &[])])
            .vars(&[("Target", &["FloatDst"])])
            .scalars(&[Scalar::Bool("DefaultValue", true)]),
    ];
    with_kismet(&ops, &[("FloatDst", 0)], |k, cx, _| {
        enter(k, cx, Entry::Remote("Ping"));
        k.run(cx).unwrap();
        assert!(k.errors.is_empty(), "{:?}", k.errors);
        let sb = k.find("SB").unwrap();
        let (_, name, value) = k.variable_value(cx, sb, "Target", 0).unwrap().unwrap();
        assert_eq!((name, value), ("FloatValue", Value::Float(1.0)));
    });
}

#[test]
fn a_delay_reads_a_linked_duration_variable() {
    let ops = vec![
        event("Ev", "Ping", &[t("Del", 0)]),
        op(
            "Del",
            "SeqAct_Delay",
            &["Start"],
            vec![out("Finished", &[t("Act", 0)])],
        )
        .vars(&[("Duration", &["FloatDur"])])
        .scalars(&[Scalar::Float("Duration", 100.0)]),
        world("Act"),
    ];
    let got = kismet(&ops, &[("FloatDur", 3)], Entry::Remote("Ping"), &[2.9]);
    assert!(got.host.is_empty());
    let got = kismet(&ops, &[("FloatDur", 3)], Entry::Remote("Ping"), &[2.9, 0.1]);
    assert_eq!(got.host, host(&["Act"]));
}

#[test]
fn unknown_gate_and_delay_inputs_and_a_missing_output_are_error_lines() {
    let ops = vec![
        event("Ev", "Ping", &[t("G", 4), t("Del", 1), t("D2", 0)]),
        op(
            "G",
            "SeqAct_Gate",
            &["In", "Open", "Close", "Toggle", "Shake"],
            through(),
        ),
        op(
            "Del",
            "SeqAct_Delay",
            &["Start", "Stop"],
            vec![out("Finished", &[])],
        ),
        op("D2", "SeqAct_Delay", &["Start"], vec![out("Other", &[])]),
        world("Act"),
    ];
    let got = kismet(&ops, &[], Entry::Remote("Ping"), &[]);
    assert_eq!(
        got.errors,
        lines(&[
            "G: unsupported gate input 'shake'",
            "Del: delay input 'stop' is unsupported",
            "D2: delay without a Finished output"
        ])
    );
}

#[test]
fn activate_remote_event_without_a_name_is_an_error_line_and_unmatched_names_reach_the_sink() {
    let ops = vec![
        event("Ev", "Ping", &[t("Hop", 0), t("Hop2", 0)]),
        op(
            "Hop",
            "SeqAct_ActivateRemoteEvent",
            &["In"],
            vec![out("Out", &[])],
        )
        .scalars(&[Scalar::Name("EventName", "")]),
        op(
            "Hop2",
            "SeqAct_ActivateRemoteEvent",
            &["In"],
            vec![out("Out", &[])],
        )
        .scalars(&[Scalar::Name("EventName", "Far")]),
    ];
    with_kismet(&ops, &[], |k, cx, _| {
        let sunk: Rc<RefCell<Vec<String>>> = Rc::default();
        let sink = Rc::clone(&sunk);
        k.remote_sink = Some(Rc::new(move |name: &str| {
            sink.borrow_mut().push(name.to_string())
        }));
        enter(k, cx, Entry::Remote("Ping"));
        k.run(cx).unwrap();
        assert_eq!(k.errors, lines(&["Hop: remote event without a name"]));
        assert_eq!(*sunk.borrow(), lines(&["Far"]));
        assert!(k.trace.contains(&"Hop2 remote 'Far'".to_string()));
    });
}

#[test]
fn a_nearer_class_handler_wins_and_paths_match_in_any_case() {
    let ops = vec![event("Ev", "Ping", &[t("Act", 0)]), world("Act")];
    with_kismet(&ops, &[], |k, cx, host| {
        let seen: Rc<RefCell<usize>> = Rc::default();
        let s = Rc::clone(&seen);
        k.handle(
            "engine.seqact_toyworldaction",
            Rc::new(move |_, _, _, _| {
                *s.borrow_mut() += 1;
                Ok(())
            }),
        );
        enter(k, cx, Entry::Remote("Ping"));
        k.run(cx).unwrap();
        assert_eq!(*seen.borrow(), 1);
        assert!(host.borrow().is_empty());
    });
}

#[test]
fn an_input_on_an_event_fires_its_first_output_and_idle_reports_the_queues() {
    let ops = vec![
        op(
            "Ev",
            "SequenceEvent",
            &["In"],
            vec![out("Out", &[t("A", 0)]), out("Second", &[t("B", 0)])],
        ),
        world("A"),
        world("B"),
    ];
    with_kismet(&ops, &[], |k, cx, host| {
        assert!(k.idle());
        let ev = k.find("Ev").unwrap();
        k.activate(cx, ev, 0, 2.0).unwrap();
        assert!(!k.idle());
        k.tick(cx, 2.0).unwrap();
        assert!(k.idle());
        assert_eq!(*host.borrow(), host_lines(&["A"]));
        assert_eq!(k.now(), 2.0);
    });
}

fn host_lines(names: &[&str]) -> Vec<String> {
    host(names)
}

#[test]
fn a_missing_class_fails_the_executor_with_the_reflect_text() {
    // Without Engine.SequenceOp there is no executor (the C++ findClass throws).
    let mut parts = Parts::new(&chain_ops(), &[]);
    std::fs::remove_file(parts._tree.0.join("Engine.upk")).unwrap();
    parts.store = PackageStore::open(parts._tree.0.clone()).unwrap();
    let mut cx = parts.cx();
    let package = cx.code.package(cx.store, "TestSeq").unwrap();
    let e = Kismet::new(&mut cx, package, "Seq").unwrap_err();
    assert!(matches!(e, KismetError::Reflect(_)), "{e:?}");
}

#[test]
fn value_reading_follows_the_cpp_value() {
    assert_eq!(number(&Value::Bool(true)), 1.0);
    assert_eq!(number(&Value::string("x")), 0.0);
    assert_eq!(integer(&Value::Float(-2.7)), Some(-2));
    assert_eq!(integer(&Value::Float(f32::NAN)), None);
    assert_eq!(integer(&Value::Float(1e30)), None);
    assert_eq!(integer(&Value::string("x")), Some(0));
    assert_eq!(same_kind(&Value::Bool(false), 2), Some(Value::Bool(true)));
    assert_eq!(same_kind(&Value::Byte(0), 256), None);
    assert_eq!(same_kind(&Value::Int(0), i64::from(i32::MAX) + 1), None);
    assert_eq!(same_kind(&Value::string(""), 1), None);
}

#[test]
fn equal_delays_run_in_activation_order_and_earlier_ones_first() {
    let ops = vec![world("A"), world("B"), world("C")];
    with_kismet(&ops, &[], |k, cx, host| {
        let (a, b, c) = (
            k.find("A").unwrap(),
            k.find("B").unwrap(),
            k.find("C").unwrap(),
        );
        k.activate(cx, a, 0, 2.0).unwrap();
        k.activate(cx, b, 0, 1.0).unwrap();
        k.activate(cx, c, 0, 2.0).unwrap();
        k.tick(cx, 5.0).unwrap();
        // All three are due; each due impulse is queued on the stack, so the last queued runs first.
        assert_eq!(*host.borrow(), host_lines(&["C", "A", "B"]));
    });
}

#[test]
fn a_reference_to_an_instantiated_export_lands_like_a_stand_in() {
    let ops = vec![
        event("Ev", "Ping", &[t("SB", 0)]),
        op("SB", "SeqAct_SetBool", &["In"], vec![out("Out", &[])]).vars(&[("Target", &["VarDst"])]),
    ];
    with_kismet(&ops, &[("VarDst", 0)], |k, cx, _| {
        let sb = k.find("SB").unwrap();
        let var = k.variables(cx, sb, "Target").unwrap()[0];
        // The variable is an instantiated export (Origin::Export), as a definition store's holder would give.
        assert!(matches!(
            cx.space.heap.get(var).unwrap().origin,
            Origin::Export { .. }
        ));
        let package = k.package();
        let found = resource(cx.space, &Value::Object(Some(var)));
        assert_eq!(found.map(|(p, _)| p), Some(package));
        assert_eq!(found.map(|(_, i)| k.find_by_index(i).is_none()), Some(true));
        // An op's own object (also Origin::Export) resolves to that op.
        let op_object = k.ops()[sb].object;
        let (p, i) = resource(cx.space, &Value::Object(Some(op_object))).unwrap();
        assert_eq!((p, k.find_by_index(i)), (package, Some(sb)));
        assert_eq!(resource(cx.space, &Value::Object(None)), None);
    });
}

#[test]
#[should_panic]
fn an_op_position_out_of_range_panics() {
    with_kismet(&chain_ops(), &[], |k, cx, _| {
        let _ = k.prop(cx, 99, "TriggerCount");
    });
}
