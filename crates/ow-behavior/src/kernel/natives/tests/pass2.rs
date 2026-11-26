//! Tests of pass 2 of `IntializeBehaviorProviderForConsumer` (NATIVE_BEHAVIOR_KERNEL_PASS2.md section 6, lane
//! IMPL-BEHAVIOR-KERNEL-PASS2) on invented data (AGENTS.md rule 9). AI-assisted (Claude Code).
//!
//! The condition, the world, the replication info, the tracker, the mission and the objective are toy classes declared
//! here on the game's class paths where the rule tests a class by name. The verdict comes from a toy
//! `MissionConditionAnswers` the test sets per condition. The providers are the fixtures' `ToyPass2` (a conditioned
//! sequence `Gated`, off at spawn) and `ToyPass2Spawn` (the same, on at spawn); each logs its enabled and disabled
//! events.

use std::cell::RefCell;

use ow_vm::mission_conditions::{self, MissionConditionAnswers, MissionConditions};

use super::*;
use crate::kernel::natives::pass2::{Record, WAYPOINT};

/// The toy answers: a verdict per condition, and the Complete missions.
#[derive(Default)]
struct Answers {
    verdicts: RefCell<DetMap<ObjRef, bool>>,
    complete: RefCell<Vec<ObjRef>>,
}

impl MissionConditionAnswers for Answers {
    fn verdict(&self, _vm: &Vm, condition: ObjRef) -> Result<bool, VmError> {
        Ok(self
            .verdicts
            .borrow()
            .get(&condition)
            .copied()
            .unwrap_or(false))
    }

    fn mission_complete(&self, _vm: &Vm, mission: ObjRef) -> Result<bool, VmError> {
        Ok(self.complete.borrow().contains(&mission))
    }
}

struct P2 {
    t: Toy,
    condition: ClassId,
    world: ClassId,
    gri: ClassId,
    plain_gri: ClassId,
    tracker: ClassId,
    mission: ClassId,
    int_mission: ClassId,
    objective: ClassId,
    consumer: ClassId,
    net_mode: Vec<String>,
    answers: Rc<Answers>,
}

fn p2(service: bool) -> P2 {
    p2_with(service, false)
}

/// [`p2`], the condition class declaring `bInstanced` as an int when `mistyped` (test 8's script error).
fn p2_with(service: bool, mistyped: bool) -> P2 {
    let mut t = toy(ParityMode::Off);
    Pack.register(&mut t.vm).unwrap();
    let bare = t.bare;
    let n = |t: &mut Toy, s: &str| t.vm.space.names.intern(s);
    let f = |t: &mut Toy, s: &str, k: PropKind| PropertyDecl::new(n(t, s), k);
    let o = PropKind::Object { class: bare };
    let net_mode = [
        "NM_Standalone",
        "NM_DedicatedServer",
        "NM_ListenServer",
        "NM_Client",
    ];
    let values = net_mode.iter().map(|v| n(&mut t, v)).collect();
    let mode_enum =
        t.vm.space
            .classes
            .define_enum("Engine.WorldInfo.ENetMode", values)
            .unwrap();
    let values = [
        "ROLE_None",
        "ROLE_SimulatedProxy",
        "ROLE_AutonomousProxy",
        "ROLE_Authority",
    ]
    .iter()
    .map(|v| n(&mut t, v))
    .collect();
    let role_enum =
        t.vm.space
            .classes
            .define_enum("Engine.Actor.ENetRole", values)
            .unwrap();
    let world_fields = vec![
        f(
            &mut t,
            "NetMode",
            PropKind::Byte {
                enum_: Some(mode_enum),
            },
        ),
        f(&mut t, "GRI", o.clone()),
    ];
    let world =
        t.vm.space
            .classes
            .define("Engine.WorldInfo", None, world_fields)
            .unwrap();
    let gri_fields = vec![f(&mut t, "MissionTracker", o.clone())];
    let gri =
        t.vm.space
            .classes
            .define("WillowGame.WillowGameReplicationInfo", None, gri_fields)
            .unwrap();
    let plain_gri =
        t.vm.space
            .classes
            .define("Engine.GameReplicationInfo", None, vec![])
            .unwrap();
    let tracker_fields = vec![f(
        &mut t,
        "Role",
        PropKind::Byte {
            enum_: Some(role_enum),
        },
    )];
    let tracker =
        t.vm.space
            .classes
            .define("WillowGame.MissionTracker", None, tracker_fields)
            .unwrap();
    let mission_fields = vec![f(&mut t, "bRepeatable", PropKind::Bool)];
    let mission =
        t.vm.space
            .classes
            .define("WillowGame.MissionDefinition", None, mission_fields)
            .unwrap();
    let int_fields = vec![f(&mut t, "bRepeatable", PropKind::Int)];
    let int_mission =
        t.vm.space
            .classes
            .define("WillowGame.ToyIntMission", None, int_fields)
            .unwrap();
    let objective_fields = vec![f(&mut t, "bRememberItemsWithinObjective", PropKind::Bool)];
    let objective =
        t.vm.space
            .classes
            .define(
                "WillowGame.MissionObjectiveDefinition",
                None,
                objective_fields,
            )
            .unwrap();
    let six: Vec<PropertyDecl> = [
        "bNotStarted",
        "bActive",
        "bReadyToTurnIn",
        "bComplete",
        "bFailed",
        "bAny",
    ]
    .iter()
    .map(|s| f(&mut t, s, PropKind::Bool))
    .collect();
    let mission_states =
        t.vm.space
            .classes
            .define_struct("WillowGame.ToyMissionStates", None, six)
            .unwrap();
    let three: Vec<PropertyDecl> = ["bComplete", "bActive", "bNotStarted"]
        .iter()
        .map(|s| f(&mut t, s, PropKind::Bool))
        .collect();
    let objective_states =
        t.vm.space
            .classes
            .define_struct("WillowGame.ToyObjectiveStates", None, three)
            .unwrap();
    let condition_fields = vec![
        f(&mut t, "LinkedMission", o.clone()),
        f(&mut t, "LinkedObjective", o.clone()),
        f(
            &mut t,
            "bInstanced",
            if mistyped {
                PropKind::Int
            } else {
                PropKind::Bool
            },
        ),
        f(&mut t, "bIsObjectiveSpecific", PropKind::Bool),
        f(&mut t, "bCreateWaypoint", PropKind::Bool),
        f(
            &mut t,
            "MissionStatesToLinkTo",
            PropKind::Struct(mission_states),
        ),
        f(
            &mut t,
            "ObjectiveStatesToLinkTo",
            PropKind::Struct(objective_states),
        ),
        f(
            &mut t,
            "ObjectiveSetRestrictions",
            PropKind::Array(Box::new(o.clone())),
        ),
    ];
    let condition =
        t.vm.space
            .classes
            .define(
                "WillowGame.BehaviorSequenceEnableByMission",
                None,
                condition_fields,
            )
            .unwrap();
    let handle = t.handle;
    let consumer_fields = vec![
        f(&mut t, "ConsumerHandle", PropKind::Struct(handle)),
        f(&mut t, "WorldInfo", o),
    ];
    let consumer =
        t.vm.space
            .classes
            .define("ToyKernel.ToyWorldPawn", None, consumer_fields)
            .unwrap();
    let answers = Rc::new(Answers::default());
    if service {
        mission_conditions::install(&mut t.vm, answers.clone()).unwrap();
    }
    P2 {
        t,
        condition,
        world,
        gri,
        plain_gri,
        tracker,
        mission,
        int_mission,
        objective,
        consumer,
        net_mode: net_mode.iter().map(|s| s.to_string()).collect(),
        answers,
    }
}

/// The write counters, the class-code loads and the name table's size.
fn counters(vm: &Vm) -> (u64, u64, u64, u64, usize) {
    (
        vm.space.heap.committed_writes(),
        vm.rng.draws(),
        vm.defs.load_transitions(),
        vm.class_code_loads(),
        vm.space.names.len(),
    )
}

/// What a world looks like for the tracker walk.
#[derive(Clone, Copy)]
enum Tracker {
    /// `GRI` None.
    NoGri,
    /// `GRI` not a `WillowGameReplicationInfo`.
    PlainGri,
    /// `MissionTracker` None.
    NoTracker,
    /// A tracker with `Role` `ROLE_Authority` (true) or `ROLE_SimulatedProxy`.
    Role(bool),
}

impl P2 {
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.t.vm.space.heap.get(o).unwrap().class;
        let p = self
            .t
            .vm
            .space
            .prop(class, name)
            .unwrap_or_else(|| panic!("no {name}"));
        *self.t.vm.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = v;
    }

    /// A world with net mode `mode` and the tracker shape `tracker`.
    fn world(&mut self, mode: &str, shape: Tracker) -> (ObjRef, Option<ObjRef>) {
        let world = self.t.object(self.world, "World");
        let m = self.net_mode.iter().position(|v| v == mode).unwrap() as u8;
        self.set(world, "NetMode", Value::Byte(m));
        let mut tracker = None;
        match shape {
            Tracker::NoGri => {}
            Tracker::PlainGri => {
                let g = self.t.object(self.plain_gri, "PlainGri");
                self.set(world, "GRI", Value::Object(Some(g)));
            }
            Tracker::NoTracker | Tracker::Role(_) => {
                let g = self.t.object(self.gri, "Gri");
                self.set(world, "GRI", Value::Object(Some(g)));
                if let Tracker::Role(authority) = shape {
                    let tr = self.t.object(self.tracker, "Tracker");
                    self.set(tr, "Role", Value::Byte(if authority { 3 } else { 1 }));
                    self.set(g, "MissionTracker", Value::Object(Some(tr)));
                    tracker = Some(tr);
                }
            }
        }
        (world, tracker)
    }

    /// A consumer in `world`, registered: its handle.
    fn consumer(&mut self, world: ObjRef, name: &str) -> (ObjRef, Value) {
        let c = self.t.object(self.consumer, name);
        self.set(c, "WorldInfo", Value::Object(Some(world)));
        let h = self
            .t
            .call(REGISTER_CONSUMER, None, vec![Value::Object(Some(c))])
            .unwrap();
        (c, h)
    }

    /// A condition on `mission` (none: None), not objective-specific, the other flags clear.
    fn condition(&mut self, mission: Option<ObjRef>) -> ObjRef {
        let c = self.t.object(self.condition, "Condition");
        self.set(c, "LinkedMission", Value::Object(mission));
        c
    }

    fn mission(&mut self) -> ObjRef {
        self.t.object(self.mission, "Mission")
    }

    /// The fixture definition `provider` (from `file`), its first sequence's condition set to `condition`.
    fn definition(&mut self, file: &str, provider: &str, condition: ObjRef) -> ObjRef {
        let d = self.t.definition(file, provider);
        let class = self.t.vm.space.heap.get(d).unwrap().class;
        let p = self.t.vm.space.prop(class, "BehaviorSequences").unwrap();
        let mut list = self
            .t
            .vm
            .space
            .read(d, PropAddr::scalar(p))
            .unwrap()
            .clone();
        let id = self.t.vm.space.names.find("CustomEnableCondition").unwrap();
        {
            let seq = &mut list.array_mut().unwrap()[0];
            let s = seq.struct_mut().unwrap();
            let layout = self.t.vm.space.classes.struct_layout(s.ty).unwrap();
            let (entry, _) = layout.field(id).unwrap();
            s.fields[entry as usize] = Value::Object(Some(condition));
        }
        *self.t.vm.space.slot_mut(d, PropAddr::scalar(p)).unwrap() = list;
        d
    }

    fn register(&mut self, h: &Value, d: ObjRef) -> Result<Value, VmError> {
        self.t.call(
            INITIALIZE_PROVIDER,
            None,
            vec![h.clone(), Value::Object(Some(d))],
        )
    }

    fn enabled(&mut self, h: &Value, d: ObjRef) -> bool {
        let gated = self.t.name("Gated");
        match self
            .t
            .call(
                IS_SEQUENCE_ENABLED,
                None,
                vec![h.clone(), Value::Object(Some(d)), gated],
            )
            .unwrap()
        {
            Value::Bool(b) => b,
            other => panic!("{other:?}"),
        }
    }

    fn links(&self, condition: ObjRef) -> Vec<Record> {
        let k = kernel(&self.t.vm).unwrap();
        let k = borrow(&k).unwrap();
        k.condition_links
            .get(&condition)
            .cloned()
            .unwrap_or_default()
    }

    /// The conditions stored under `mission`, by its stock path (the service's one identity, review of #775, item 4).
    fn stored(&self, mission: ObjRef) -> Vec<ObjRef> {
        let path = crate::kernel::natives::pass2::mission_key(&self.t.vm, mission).unwrap();
        match self.t.vm.service::<MissionConditions>() {
            Some(cell) => cell.borrow().stored_under(&path),
            None => Vec::new(),
        }
    }

    fn path(&self, mission: ObjRef) -> String {
        crate::kernel::natives::pass2::mission_key(&self.t.vm, mission).unwrap()
    }

    fn verdict(&self, condition: ObjRef, v: bool) {
        self.answers.verdicts.borrow_mut().insert(condition, v);
    }

    /// A refusal of the registration, with nothing written (the counters, the links and the store unchanged).
    fn refused(&mut self, h: &Value, d: ObjRef, condition: ObjRef) -> NotYet {
        let before = counters(&self.t.vm);
        let n = match self.register(h, d) {
            Err(VmError::NotYet(n)) => n,
            other => panic!("expected NotYet, got {other:?}"),
        };
        assert_eq!(counters(&self.t.vm), before, "{n}: nothing written");
        assert!(!ow_core::notyet::is_partial_writes(&n.to_string()), "{n}");
        assert!(self.links(condition).is_empty(), "{n}: nothing linked");
        n
    }
}

/// A world with an authority tracker, a consumer and the `ToyPass2` definition on a condition of a fresh mission.
fn standard(x: &mut P2, provider: (&str, &str)) -> (Value, ObjRef, ObjRef, ObjRef) {
    let (world, _) = x.world("NM_Standalone", Tracker::Role(true));
    let (_, h) = x.consumer(world, "Maya");
    let m = x.mission();
    let c = x.condition(Some(m));
    let d = x.definition(provider.0, provider.1, c);
    (h, d, c, m)
}

const OFF: (&str, &str) = ("TestPass2", "ToyPass2");
const ON: (&str, &str) = ("TestPass2Spawn", "ToyPass2Spawn");

// 1.
#[test]
fn a_false_verdict_links_and_stores_and_fires_nothing() {
    let mut x = p2(true);
    let (h, d, c, m) = standard(&mut x, OFF);
    x.register(&h, d).unwrap();
    assert!(!x.enabled(&h, d));
    assert_eq!(x.links(c).len(), 1, "linked once");
    assert_eq!(x.stored(m), vec![c], "stored once under the mission");
    assert!(x.t.seen().is_empty(), "no event");
}

// 2.
#[test]
fn a_true_verdict_enables_after_pass_one() {
    let mut x = p2(true);
    let (h, d, c, _) = standard(&mut x, OFF);
    x.verdict(c, true);
    x.register(&h, d).unwrap();
    assert!(x.enabled(&h, d));
    assert_eq!(
        x.t.seen(),
        vec!["OnBehaviorSequenceEnabled En consumer=Maya"]
    );
}

// 3.
#[test]
fn enabled_on_spawn_then_a_false_verdict_disables_it() {
    let mut x = p2(true);
    let (h, d, _, _) = standard(&mut x, ON);
    x.register(&h, d).unwrap();
    assert!(!x.enabled(&h, d));
    assert_eq!(
        x.t.seen(),
        vec![
            "OnBehaviorSequenceEnabled En consumer=Maya",
            "OnBehaviorSequenceDisabled Dis consumer=Maya"
        ]
    );
}

// 4.
#[test]
fn an_instanced_condition_is_not_linked() {
    let mut x = p2(true);
    let (h, d, c, m) = standard(&mut x, ON);
    x.set(c, "bInstanced", Value::Bool(true));
    x.register(&h, d).unwrap();
    assert!(x.enabled(&h, d), "as pass 1 left it");
    assert!(x.links(c).is_empty());
    assert!(x.stored(m).is_empty());
}

// 5.
#[test]
fn without_a_tracker_the_sequence_is_only_linked() {
    for shape in [Tracker::NoGri, Tracker::PlainGri, Tracker::NoTracker] {
        let mut x = p2(true);
        let (world, _) = x.world("NM_Standalone", shape);
        let (_, h) = x.consumer(world, "Maya");
        let m = x.mission();
        let c = x.condition(Some(m));
        x.verdict(c, false);
        let d = x.definition(ON.0, ON.1, c);
        x.register(&h, d).unwrap();
        assert!(x.enabled(&h, d), "as pass 1 left it");
        assert_eq!(x.links(c).len(), 1, "linked");
        assert!(x.stored(m).is_empty(), "nothing stored");
    }
}

// 6.
#[test]
fn no_authority_or_no_mission_links_without_registering() {
    let mut x = p2(true);
    let (world, _) = x.world("NM_Standalone", Tracker::Role(false));
    let (_, h) = x.consumer(world, "Maya");
    let m = x.mission();
    let c = x.condition(Some(m));
    let d = x.definition(ON.0, ON.1, c);
    x.register(&h, d).unwrap();
    assert!(x.enabled(&h, d), "no verdict applied");
    assert_eq!(x.links(c).len(), 1);
    assert!(x.stored(m).is_empty());

    let mut y = p2(true);
    let (world, _) = y.world("NM_Standalone", Tracker::Role(true));
    let (_, h) = y.consumer(world, "Maya");
    let c = y.condition(None);
    let d = y.definition(ON.0, ON.1, c);
    y.register(&h, d).unwrap();
    assert!(y.enabled(&h, d));
    assert_eq!(y.links(c).len(), 1);
}

// 7.
#[test]
fn a_second_consumer_re_applies_the_verdict_to_the_first() {
    let mut x = p2(true);
    let (world, _) = x.world("NM_Standalone", Tracker::Role(true));
    let (_, h1) = x.consumer(world, "Maya");
    let (_, h2) = x.consumer(world, "Mordecai");
    let m = x.mission();
    let c = x.condition(Some(m));
    let d = x.definition(OFF.0, OFF.1, c);
    x.register(&h1, d).unwrap();
    x.register(&h2, d).unwrap();
    assert_eq!(x.links(c).len(), 2, "both records in the shared list");
    assert!(
        !x.enabled(&h1, d) && !x.enabled(&h2, d),
        "a false verdict changes nothing"
    );
    assert!(x.t.seen().is_empty());
}

// 8.
#[test]
fn the_refusals_come_before_any_write() {
    // An unloaded condition: the fixtures' ToyCondition, a store handle.
    let mut x = p2(true);
    let (world, _) = x.world("NM_Standalone", Tracker::Role(true));
    let (_, h) = x.consumer(world, "Maya");
    let lazy =
        x.t.vm
            .space
            .alloc_stand_in(
                x.condition,
                "LazyCondition",
                ow_object::heap::Origin::Default { class: x.condition },
            )
            .unwrap();
    let d = x.definition(OFF.0, OFF.1, lazy);
    let n = x.refused(&h, d, lazy);
    assert_eq!(
        (n.missing, &*n.what),
        (Missing::NotRead, CONDITION_NOT_LOADED)
    );

    // Another condition class.
    let mut x = p2(true);
    let (world, _) = x.world("NM_Standalone", Tracker::Role(true));
    let (_, h) = x.consumer(world, "Maya");
    let other = x.t.object(x.plain_gri, "OtherCondition");
    let d = x.definition(OFF.0, OFF.1, other);
    let n = x.refused(&h, d, other);
    assert!(
        n.what
            .contains("a CustomEnableCondition of class Engine.GameReplicationInfo"),
        "{n}"
    );

    // An unloaded LinkedObjective, a per-instance objective, and no service.
    let mut x = p2(true);
    let (h, d, c, _) = standard(&mut x, OFF);
    x.set(c, "bIsObjectiveSpecific", Value::Bool(true));
    let objective = x.t.object(x.objective, "Objective");
    x.set(
        objective,
        "bRememberItemsWithinObjective",
        Value::Bool(true),
    );
    x.set(c, "LinkedObjective", Value::Object(Some(objective)));
    let n = x.refused(&h, d, c);
    assert_eq!((n.missing, &*n.what), (Missing::NotRead, PER_INSTANCE));
    let stand_in =
        x.t.vm
            .space
            .alloc_stand_in(
                x.objective,
                "Lazy",
                ow_object::heap::Origin::Default { class: x.objective },
            )
            .unwrap();
    x.set(c, "LinkedObjective", Value::Object(Some(stand_in)));
    let n = x.refused(&h, d, c);
    assert_eq!(
        (n.missing, &*n.what),
        (Missing::NotRead, OBJECTIVE_NOT_LOADED)
    );

    let mut x = p2(false);
    let (h, d, c, _) = standard(&mut x, OFF);
    let n = x.refused(&h, d, c);
    assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}");
    assert_eq!(&*n.what, NO_ANSWERS);
}

// 8 (the mistyped declaration).
#[test]
fn a_mistyped_condition_field_is_a_script_error() {
    let mut x = p2_with(true, true);
    let (world, _) = x.world("NM_Standalone", Tracker::Role(true));
    let (_, h) = x.consumer(world, "Maya");
    let c = x.condition(None);
    x.set(c, "bInstanced", Value::Int(0));
    let d = x.definition(OFF.0, OFF.1, c);
    let before = counters(&x.t.vm);
    match x.register(&h, d) {
        Err(VmError::Script { message, .. }) => {
            assert!(message.contains("bInstanced"), "{message}")
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(counters(&x.t.vm), before, "nothing written");
}

// 9.
#[test]
fn the_direct_native_skips_an_empty_definition() {
    let mut x = p2(true);
    let (world, _) = x.world("NM_Standalone", Tracker::Role(true));
    let (_, h) = x.consumer(world, "Maya");
    let empty = x.t.definition("TestCreateEmpty", CREATE_EMPTY);
    x.t.vm.defs.ensure_loaded(&mut x.t.vm.space, empty).unwrap();
    x.register(&h, empty).unwrap();
    let pid = x.t.pid_of(&h);
    let k = kernel(&x.t.vm).unwrap();
    assert!(
        borrow(&k)
            .unwrap()
            .processes
            .get(&pid)
            .unwrap()
            .providers
            .is_empty(),
        "nothing registered"
    );
}

// 10.
#[test]
fn a_complete_mission_is_stored_only_when_repeatable() {
    let mut x = p2(true);
    let (h, d, c, m) = standard(&mut x, OFF);
    x.answers.complete.borrow_mut().push(m);
    x.register(&h, d).unwrap();
    assert_eq!(x.links(c).len(), 1, "the verdict applied (linked)");
    assert!(
        x.stored(m).is_empty(),
        "Complete and not repeatable: not stored"
    );

    let mut x = p2(true);
    let (h, d, c, m) = standard(&mut x, OFF);
    x.set(m, "bRepeatable", Value::Bool(true));
    x.answers.complete.borrow_mut().push(m);
    x.register(&h, d).unwrap();
    assert_eq!(x.stored(m), vec![c], "repeatable: stored");

    let mut x = p2(true);
    let (world, _) = x.world("NM_Standalone", Tracker::Role(true));
    let (_, h) = x.consumer(world, "Maya");
    let m = x.t.object(x.int_mission, "IntMission");
    x.answers.complete.borrow_mut().push(m);
    let c = x.condition(Some(m));
    let d = x.definition(OFF.0, OFF.1, c);
    let before = counters(&x.t.vm);
    match x.register(&h, d) {
        Err(VmError::Script { message, .. }) => {
            assert!(message.contains("bRepeatable"), "{message}")
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(counters(&x.t.vm), before, "nothing written");
}

/// Review of #784: a Complete `LinkedMission` the store has not loaded is the mission's not-loaded `NotRead`, nothing
/// written or linked (its text names the mission when the store gives it a path; a run-time stand-in has none, so the
/// text here is the constant).
#[test]
fn a_complete_mission_not_loaded_is_not_read() {
    let mut x = p2(true);
    let (world, _) = x.world("NM_Standalone", Tracker::Role(true));
    let (_, h) = x.consumer(world, "Maya");
    let class = x.mission;
    let lazy = x.t.vm.space.alloc_stand_in(class, "LazyMission", ow_object::heap::Origin::Default { class }).unwrap();
    x.answers.complete.borrow_mut().push(lazy);
    let c = x.condition(Some(lazy));
    let d = x.definition(OFF.0, OFF.1, c);
    let n = x.refused(&h, d, c);
    assert_eq!((n.missing, &*n.what), (Missing::NotRead, MISSION_NOT_LOADED));
}

// 11.
#[test]
fn a_client_world_has_no_pass_two() {
    let mut x = p2(true);
    let (world, _) = x.world("NM_Client", Tracker::Role(true));
    let (_, h) = x.consumer(world, "Maya");
    let m = x.mission();
    let c = x.condition(Some(m));
    let d = x.definition(ON.0, ON.1, c);
    x.register(&h, d).unwrap();
    assert!(x.enabled(&h, d), "as pass 1 left it");
    assert!(x.links(c).is_empty());
    assert!(x.stored(m).is_empty());
}

// 12 (the kernel side): a later notification re-evaluates a stored condition through the same verdict; an
// unloaded objective is the typed refusal.
#[test]
fn a_later_notification_re_evaluates_through_the_verdict() {
    let mut x = p2(true);
    let (h, d, c, m) = standard(&mut x, OFF);
    x.register(&h, d).unwrap();
    assert!(!x.enabled(&h, d));
    x.verdict(c, true);
    let mp = x.path(m);
    reevaluate_conditions(&mut x.t.vm, &mp).unwrap();
    assert!(x.enabled(&h, d), "follows the new verdict");
    assert_eq!(
        x.t.seen(),
        vec!["OnBehaviorSequenceEnabled En consumer=Maya"]
    );
    // Another mission's notification reaches nothing (another stock path: the service's identity).
    let other = x.t.object(x.mission, "OtherMission");
    x.verdict(c, false);
    let op = x.path(other);
    reevaluate_conditions(&mut x.t.vm, &op).unwrap();
    assert!(x.enabled(&h, d));
    // The objective made unloaded: the typed NotRead.
    x.set(c, "bIsObjectiveSpecific", Value::Bool(true));
    let stand_in =
        x.t.vm
            .space
            .alloc_stand_in(
                x.objective,
                "Lazy",
                ow_object::heap::Origin::Default { class: x.objective },
            )
            .unwrap();
    x.set(c, "LinkedObjective", Value::Object(Some(stand_in)));
    match reevaluate_conditions(&mut x.t.vm, &mp) {
        Err(VmError::NotYet(n)) => assert_eq!(
            (n.missing, &*n.what),
            (Missing::NotRead, OBJECTIVE_NOT_LOADED)
        ),
        other => panic!("{other:?}"),
    }
}

// 13.
#[test]
fn the_waypoint_test_reads_another_consumer_s_record() {
    let mut x = p2(true);
    let (world, _) = x.world("NM_Standalone", Tracker::Role(true));
    let (_, h1) = x.consumer(world, "Maya");
    let (_, h2) = x.consumer(world, "Mordecai");
    let m = x.mission();
    let c = x.condition(Some(m));
    x.set(c, "bCreateWaypoint", Value::Bool(true));
    let d_off = x.definition(OFF.0, OFF.1, c);
    x.register(&h1, d_off).unwrap();
    assert!(
        !x.enabled(&h1, d_off),
        "linked and disabled, the verdict unchanged"
    );
    let d_on = x.definition(ON.0, ON.1, c);
    let n = x.refused_after_links(&h2, d_on, c);
    assert_eq!((n.missing, &*n.what), (Missing::NotRead, WAYPOINT));
}

impl P2 {
    /// A refusal with nothing written, the condition's list as it was.
    fn refused_after_links(&mut self, h: &Value, d: ObjRef, condition: ObjRef) -> NotYet {
        let (before, links) = (counters(&self.t.vm), self.links(condition));
        let n = match self.register(h, d) {
            Err(VmError::NotYet(n)) => n,
            other => panic!("expected NotYet, got {other:?}"),
        };
        assert_eq!(counters(&self.t.vm), before, "{n}: nothing written");
        assert_eq!(self.links(condition), links, "{n}: the list unchanged");
        n
    }
}

/// Review of #775, item 3 (the hub's ruling): a refusal from commit step C always carries the partial-writes prefix,
/// since pass 1 has run by then. A true verdict enables the sequence, and its enabled event's first behavior refuses.
/// The rule is called directly, past the native's counter check (which would add the prefix on its own once pass 1's
/// writes moved #518's counters), so only the commit's own marking can put the prefix there (review r2 of #775,
/// item 1).
#[test]
fn a_refusal_from_the_reaction_carries_the_prefix() {
    let mut x = p2(true);
    let (h, d, c, _) = standard(&mut x, ("TestPass2Refuse", "ToyPass2Refuse"));
    x.verdict(c, true);
    let pid = crate::kernel::natives::consumer_pid(&x.t.vm, &h).expect("a kernel handle");
    match crate::kernel::natives::provider_into(&mut x.t.vm, pid, d) {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert!(ow_core::notyet::is_partial_writes(&n.to_string()), "{n}");
            assert!(n.what.contains("an invented OnCreate behavior"), "{n}");
        }
        other => panic!("{other:?}"),
    }
}

/// Review of #775, item 5: a store the service could not take (its cell borrowed) is refused in the plan, before any
/// write, rather than dropped at the commit.
#[test]
fn a_borrowed_service_is_refused_before_any_write() {
    let mut x = p2(true);
    let (h, d, c, _) = standard(&mut x, OFF);
    let cell = x.t.vm.service::<MissionConditions>().unwrap();
    let held = cell.borrow();
    let before = counters(&x.t.vm);
    match x.register(&h, d) {
        Err(VmError::ServiceBorrowed(name)) => assert!(name.ends_with("MissionConditions"), "{name}"),
        other => panic!("{other:?}"),
    }
    drop(held);
    assert_eq!(counters(&x.t.vm), before, "nothing written");
    assert!(x.links(c).is_empty(), "nothing linked");
}
