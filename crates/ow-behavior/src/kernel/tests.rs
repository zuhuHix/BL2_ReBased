//! The kernel rules on invented packages (`super::fixtures`): the cases of `tests/behavior_test.py` sections 2, 3 and
//! 5 (NATIVE_MISSION_DISPATCH.md acceptance tests 1-5) and one test per remaining rule of the module documentation.

use std::cell::{Cell, RefCell};

use ow_core::Missing;

use super::fixtures::{Env, Ev, Seq, Spec, Tag, VLink, Word, beh, ev, rules, toy_provider};
use super::*;
use crate::registry::Handler;

const FIRE: &str = "Things.DamageType.ToyFire";
const ICE: &str = "Things.DamageType.ToyIce";

fn outputs(pairs: &[(&str, &str)]) -> DetMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn event(env: &mut Env, p: &mut Provider, name: &str) {
    p.fire_event(&mut env.vm, name, &DetMap::new(), -1, &[])
        .unwrap();
}

// ------------------------------------------------------------------------------------------------ CompareObject

#[test]
fn compare_object_follows_same_and_different() {
    let mut env = Env::of(&toy_provider(
        "exact",
        "BVAR_InstanceData",
        "BVARLINK_Input",
    ));
    let same = ["OnHit -> Cmp", "OnHit -> Same"];
    let diff = ["OnHit -> Cmp", "OnHit -> Diff"];
    for (given, want) in [(Some(FIRE), &same), (Some(ICE), &diff), (None, &diff)] {
        let mut p = env.provider("ToyProvider");
        let out = given.map_or_else(DetMap::new, |g| outputs(&[("DamageType", g)]));
        p.fire_event(&mut env.vm, "OnHit", &out, -1, &[]).unwrap();
        assert_eq!(p.trace, want.to_vec(), "{given:?}");
        assert!(p.errors.is_empty());
    }
    // The outputs are rewritten per event.
    let mut p = env.provider("ToyProvider");
    p.fire_event(
        &mut env.vm,
        "OnHit",
        &outputs(&[("DamageType", ICE)]),
        -1,
        &[],
    )
    .unwrap();
    p.fire_event(
        &mut env.vm,
        "OnHit",
        &outputs(&[("DamageType", FIRE)]),
        -1,
        &[],
    )
    .unwrap();
    assert_eq!(p.trace, [diff.as_slice(), same.as_slice()].concat());
    assert_eq!(
        p.boundary,
        [
            "OnHit -> WillowGame.Behavior_ToyDifferent:Diff",
            "OnHit -> WillowGame.Behavior_ToySame:Same"
        ]
    );
    // A disabled sequence does not run.
    let mut p = env.provider("ToyProvider");
    assert!(p.set_sequence_enabled(&mut env.vm, "Main", false).unwrap());
    p.fire_event(
        &mut env.vm,
        "OnHit",
        &outputs(&[("DamageType", FIRE)]),
        -1,
        &[],
    )
    .unwrap();
    assert!(p.trace.is_empty());
}

#[test]
fn reading_an_input_from_an_untrusted_block_is_an_error() {
    let mut env = Env::of(&toy_provider(
        "short",
        "BVAR_InstanceData",
        "BVARLINK_Input",
    ));
    let mut p = env.provider("ToyProvider");
    p.fire_event(
        &mut env.vm,
        "OnHit",
        &outputs(&[("DamageType", FIRE)]),
        -1,
        &[],
    )
    .unwrap();
    assert_eq!(p.trace, ["OnHit -> Cmp"]);
    assert_eq!(
        p.errors,
        [
            "variable values not decoded for Cmp.ObjectA in ToyProvider",
            "variable values not decoded for Cmp.ObjectB in ToyProvider"
        ]
    );
}

#[test]
fn a_link_type_neither_input_nor_output_is_an_error() {
    let mut env = Env::of(&toy_provider(
        "exact",
        "BVAR_InstanceData",
        "BVARLINK_Unknown",
    ));
    let mut p = env.provider("ToyProvider");
    p.fire_event(
        &mut env.vm,
        "OnHit",
        &outputs(&[("DamageType", ICE)]),
        -1,
        &[],
    )
    .unwrap();
    assert_eq!(p.trace, ["OnHit -> Cmp"]);
    assert_eq!(
        p.errors,
        ["unexpected variable link type BVARLINK_Unknown for Cmp.ObjectA"]
    );
}

// ------------------------------------------------------------------------------------------------ A1 / A2 rules

fn rules_trace(steps: &[&str]) -> Vec<String> {
    let mut env = Env::of(&rules());
    let mut p = env.provider("ToyRules");
    for step in steps {
        if let Some(rest) = step.strip_prefix("fire:") {
            let (id, name) = rest.split_once(':').unwrap();
            p.fire_event(&mut env.vm, name, &DetMap::new(), id.parse().unwrap(), &[])
                .unwrap();
        } else if let Some(s) = step.strip_prefix("tick:") {
            p.tick(&mut env.vm, s.parse().unwrap()).unwrap();
        } else {
            event(&mut env, &mut p, step);
        }
    }
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    p.trace
}

#[test]
fn a1_the_link_id_filter_selects_links() {
    assert_eq!(rules_trace(&["fire:4:Filter"]), ["Filter -> C4"]);
    assert_eq!(
        rules_trace(&["fire:-1:Filter"]),
        ["Filter -> C4", "Filter -> C5"]
    );
    assert!(rules_trace(&["fire:6:Filter"]).is_empty());
}

#[test]
fn a1_the_event_gates() {
    // MaxTriggerCount 1: the second fire is ignored.
    assert_eq!(rules_trace(&["Once", "Once"]), ["Once -> C4"]);
    // ReTriggerDelay 2 s: 1 s later ignored, 3 s later runs.
    assert_eq!(
        rules_trace(&["Delay2", "tick:1", "Delay2", "tick:2", "Delay2"]),
        ["Delay2 -> C4", "Delay2 -> C4"]
    );
    // Exactly the delay later runs (the gate is "less than").
    assert_eq!(
        rules_trace(&["Delay2", "tick:2", "Delay2"]),
        ["Delay2 -> C4", "Delay2 -> C4"]
    );
    // bEnabled false: never fired.
    assert!(rules_trace(&["Off"]).is_empty());
}

#[test]
fn a2_no_deduplication() {
    assert_eq!(
        rules_trace(&["Twice"]),
        ["Twice -> Twice", "Twice -> Twice"]
    );
}

#[test]
fn a2_recorded_ids_and_the_default_output() {
    // CompareObject selects 1 (ObjectA is ToyFire, ObjectB None); no default output: only the id-1 link.
    assert_eq!(
        rules_trace(&["DefOff"]),
        ["DefOff -> CmpOff", "DefOff -> One"]
    );
    // Default output supported: the id-1 link, then -1. The first selected continues; the other starts first.
    assert_eq!(
        rules_trace(&["DefOn"]),
        ["DefOn -> CmpOn", "DefOn -> Dflt", "DefOn -> One"]
    );
}

#[test]
fn a2_threads_run_depth_first() {
    assert_eq!(
        rules_trace(&["Order"]),
        ["Order -> A", "Order -> A2", "Order -> B", "Order -> B2"]
    );
    assert_eq!(
        rules_trace(&["Branch"]),
        [
            "Branch -> P",
            "Branch -> Y",
            "Branch -> Y2",
            "Branch -> X",
            "Branch -> X2"
        ]
    );
}

#[test]
fn event_and_sequence_names_compare_as_fnames() {
    let mut env = Env::of(&rules());
    let mut p = env.provider("ToyRules");
    event(&mut env, &mut p, "twice");
    assert_eq!(p.trace.len(), 2);
    assert!(p.set_sequence_enabled(&mut env.vm, "rules", false).unwrap());
    assert!(!p.sequence_enabled("RULES"));
    assert!(!p.set_sequence_enabled(&mut env.vm, "Nope", true).unwrap());
}

// ------------------------------------------------------------------------------------------------ time

/// One sequence: event `Go` -> A (delay `d0`); A -> B (id -1, delay `d1`) and A -> C (id -1, delay 0).
fn delays(d0: f32, d1: f32) -> Spec {
    Spec::new(
        "P",
        vec![Seq {
            name: "S",
            enabled_on_spawn: Some(true),
            events: vec![ev("Go", (0, 1))],
            behaviors: vec![
                beh("A", "Behavior_ToySame", (1, 2)),
                beh("B", "Behavior_ToySame", (0, 0)),
                beh("C", "Behavior_ToySame", (0, 0)),
            ],
            links: vec![(0, -1, d0), (1, -1, d1), (2, -1, 0.0)],
            ..Seq::default()
        }],
        vec![],
    )
}

#[test]
fn delayed_links_wait_for_their_due_time() {
    let mut env = Env::of(&delays(0.5, 0.25));
    let mut p = env.provider("P");
    let times = Rc::new(RefCell::new(Vec::new()));
    let sink = times.clone();
    p.on_time = Some(Rc::new(move |_: &mut Vm, t: f64| sink.borrow_mut().push(t)));
    event(&mut env, &mut p, "Go");
    assert!(p.trace.is_empty());
    p.tick(&mut env.vm, 0.4).unwrap();
    assert!(p.trace.is_empty());
    // One tick passes through every wake: A at 0.5 (C at once), B at 0.75.
    p.tick(&mut env.vm, 0.5).unwrap();
    assert_eq!(p.trace, ["Go -> A", "Go -> C", "Go -> B"]);
    assert_eq!(*times.borrow(), [0.4, 0.5, 0.75, 0.9]);
    assert!((p.now() - 0.9).abs() < 1e-12);
}

#[test]
fn the_first_selected_link_continues_after_its_delay() {
    // A's first selected link (B) has a delay: C runs at once, B waits.
    let mut env = Env::of(&delays(0.0, 1.0));
    let mut p = env.provider("P");
    event(&mut env, &mut p, "Go");
    assert_eq!(p.trace, ["Go -> A", "Go -> C"]);
    p.tick(&mut env.vm, 1.0).unwrap();
    assert_eq!(p.trace, ["Go -> A", "Go -> C", "Go -> B"]);
}

/// Event `Go` -> L; L links to itself `fan` times (id -1).
fn looping(fan: usize) -> Spec {
    Spec::new(
        "P",
        vec![Seq {
            name: "S",
            enabled_on_spawn: Some(true),
            events: vec![ev("Go", (0, 1))],
            behaviors: vec![beh("L", "Behavior_ToySame", (1, fan as u32))],
            links: std::iter::once((0, -1, 0.0))
                .chain(std::iter::repeat_n((0, -1, 0.0), fan))
                .collect(),
            ..Seq::default()
        }],
        vec![],
    )
}

#[test]
fn a_thread_runs_at_most_60_behaviors_per_call() {
    let mut env = Env::of(&looping(1));
    let mut p = env.provider("P");
    event(&mut env, &mut p, "Go");
    assert_eq!(p.trace.len(), THREAD_BEHAVIOR_CAP);
    // A thread parked at the current time waits for the next call; each call runs 60 more.
    p.tick(&mut env.vm, 0.0).unwrap();
    assert_eq!(p.trace.len(), 2 * THREAD_BEHAVIOR_CAP);
    p.tick(&mut env.vm, 0.0).unwrap();
    assert_eq!(p.trace.len(), 3 * THREAD_BEHAVIOR_CAP);
    assert!(p.errors.is_empty());
}

#[test]
fn the_runaway_guard_ends_the_call_once() {
    let mut env = Env::of(&looping(2));
    let mut p = env.provider("P");
    event(&mut env, &mut p, "Go");
    assert_eq!(p.trace.len(), CALL_BUDGET);
    assert_eq!(p.errors, ["behavior execution limit exceeded in P"]);
    // The waiting threads were dropped; a later call starts with a fresh budget and reports the limit only once.
    p.tick(&mut env.vm, 1.0).unwrap();
    assert_eq!(p.trace.len(), CALL_BUDGET);
    event(&mut env, &mut p, "Go");
    assert_eq!(p.trace.len(), 2 * CALL_BUDGET);
    assert_eq!(p.errors.len(), 1);
}

// ------------------------------------------------------------------------------------------------ latent behaviors

/// Event `Go` -> T (a Behavior_ToyInputs); T -> N (id 0) and T -> D (id -1).
fn latent_spec() -> Spec {
    let mut t = beh("T", "Behavior_ToyInputs", (1, 2));
    t.tags.push(Tag::DefaultOutput(true));
    Spec::new(
        "P",
        vec![Seq {
            name: "S",
            enabled_on_spawn: Some(true),
            events: vec![ev("Go", (0, 1))],
            behaviors: vec![
                t,
                beh("N", "Behavior_ToySame", (0, 0)),
                beh("D", "Behavior_ToySame", (0, 0)),
            ],
            links: vec![(0, -1, 0.0), (1, 0, 0.0), (2, -1, 0.0)],
            ..Seq::default()
        }],
        vec![],
    )
}

/// A handler that asks to wait `wait` on its first run (selecting id 0), keeps a counter in its state, and
/// finishes on the run after.
fn latent_handler(wait: f64, seen: Rc<RefCell<Vec<(bool, bool, u32)>>>) -> Handler {
    Handler::run(move |_vm, p, _id, _event| {
        let run = p.run_info();
        let count = run
            .state
            .as_ref()
            .and_then(|s| s.downcast_ref::<Cell<u32>>())
            .map_or(0, Cell::get);
        seen.borrow_mut()
            .push((run.initial_run, run.has_linked_outputs, count));
        if run.initial_run {
            run.wait = wait;
            run.state = Some(Rc::new(Cell::new(count + 1)));
            Ok(vec![0])
        } else {
            Ok(Vec::new())
        }
    })
}

#[test]
fn a_latent_behavior_waits_at_least_a_sixtieth_and_keeps_its_state() {
    let mut env = Env::of(&latent_spec());
    let mut p = env.provider("P");
    let seen = Rc::new(RefCell::new(Vec::new()));
    p.handle(
        "WillowGame.Behavior_ToyInputs",
        latent_handler(0.001, seen.clone()),
    );
    event(&mut env, &mut p, "Go");
    // The selected links (id 0, then the default -1) all start new threads; T waits.
    assert_eq!(p.trace, ["Go -> T", "Go -> N", "Go -> D"]);
    p.tick(&mut env.vm, 0.01).unwrap();
    assert_eq!(
        p.trace.len(),
        3,
        "the 1/60 floor: 0.001 s is raised to 1/60"
    );
    p.tick(&mut env.vm, 0.01).unwrap();
    // Resumed: not the initial run, same state; finished, so only its default output is followed.
    assert_eq!(
        p.trace,
        ["Go -> T", "Go -> N", "Go -> D", "Go -> T", "Go -> D"]
    );
    assert_eq!(*seen.borrow(), [(true, true, 0), (false, true, 1)]);
}

#[test]
fn a_negative_or_nan_wait_means_finished() {
    for wait in [-1.0, f64::NAN] {
        let mut env = Env::of(&latent_spec());
        let mut p = env.provider("P");
        p.handle(
            "WillowGame.Behavior_ToyInputs",
            latent_handler(wait, Rc::default()),
        );
        event(&mut env, &mut p, "Go");
        p.tick(&mut env.vm, 1.0).unwrap();
        // Finished at once: the first selected (N) continued the thread after D started.
        assert_eq!(p.trace, ["Go -> T", "Go -> D", "Go -> N"], "{wait}");
    }
}

#[test]
fn g1_a_latent_thread_of_a_disabled_sequence_ends() {
    let mut env = Env::of(&latent_spec());
    let mut p = env.provider("P");
    p.handle(
        "WillowGame.Behavior_ToyInputs",
        latent_handler(0.5, Rc::default()),
    );
    event(&mut env, &mut p, "Go");
    p.set_sequence_enabled(&mut env.vm, "S", false).unwrap();
    p.tick(&mut env.vm, 1.0).unwrap();
    assert_eq!(p.trace, ["Go -> T", "Go -> N", "Go -> D"]);
}

#[test]
fn a_nested_call_keeps_the_running_behaviors_run_information() {
    let mut env = Env::of(&rules());
    let mut p = env.provider("ToyRules");
    let after = Rc::new(Cell::new(0.0));
    let sink = after.clone();
    p.handle(
        "WillowGame.Behavior_ToySame",
        Handler::run(move |vm, p, id, _| {
            if p.behavior(id).name == "A" {
                p.run_info().wait = -2.0;
                p.fire_event(vm, "Twice", &DetMap::new(), -1, &[])?;
                sink.set(p.run_info().wait);
            }
            Ok(Vec::new())
        }),
    );
    event(&mut env, &mut p, "Order");
    assert_eq!(after.get(), -2.0);
    assert_eq!(
        p.trace,
        [
            "Order -> A",
            "Twice -> Twice",
            "Twice -> Twice",
            "Order -> A2",
            "Order -> B",
            "Order -> B2"
        ]
    );
}

// ------------------------------------------------------------------------------------------------ section C

/// Sequences A (enabled on spawn, mutex), B (disabled, mutex), C (enabled on spawn); each has the enabled and
/// disabled events, each linked to a behavior of its own name.
fn enabling() -> Spec {
    let seq = |name: &'static str, on: bool, mutex: bool| Seq {
        name,
        enabled_on_spawn: Some(on),
        mutex,
        events: vec![
            ev("OnBehaviorSequenceEnabled", (0, 1)),
            ev("OnBehaviorSequenceDisabled", (1, 1)),
        ],
        behaviors: vec![
            beh(if on { "On" } else { "OnB" }, "Behavior_ToySame", (0, 0)),
            beh("Off", "Behavior_ToySame", (0, 0)),
        ],
        links: vec![(0, -1, 0.0), (1, -1, 0.0)],
        ..Seq::default()
    };
    Spec::new(
        "P",
        vec![
            seq("A", true, true),
            seq("B", false, true),
            seq("C", true, false),
        ],
        vec![],
    )
}

#[test]
fn c_enabling_fires_after_the_bit_disabling_before() {
    let mut env = Env::of(&enabling());
    let mut p = env.provider("P");
    let states = Rc::new(RefCell::new(Vec::new()));
    let sink = states.clone();
    p.handle(
        "WillowGame.Behavior_ToySame",
        Handler::run(move |_, p, id, event| {
            let name = p.sequence_name(id).to_string();
            sink.borrow_mut()
                .push(format!("{event}:{name}:{}", p.sequence_enabled(&name)));
            Ok(Vec::new())
        }),
    );
    // Already in that state: nothing fires.
    assert!(p.set_sequence_enabled(&mut env.vm, "C", true).unwrap());
    assert!(states.borrow().is_empty());
    p.set_sequence_enabled(&mut env.vm, "C", false).unwrap();
    p.set_sequence_enabled(&mut env.vm, "C", true).unwrap();
    assert_eq!(
        *states.borrow(),
        [
            "OnBehaviorSequenceDisabled:C:true",
            "OnBehaviorSequenceEnabled:C:true"
        ]
    );
}

#[test]
fn c_enabling_a_mutex_sequence_disables_one_other() {
    let mut env = Env::of(&enabling());
    let mut p = env.provider("P");
    p.set_sequence_enabled(&mut env.vm, "B", true).unwrap();
    assert!(!p.sequence_enabled("A"));
    assert!(p.sequence_enabled("B"));
    assert!(p.sequence_enabled("C"), "C has no mutex");
    // A's disabled event, then B's enabled event.
    assert_eq!(
        p.trace,
        [
            "OnBehaviorSequenceDisabled -> Off",
            "OnBehaviorSequenceEnabled -> OnB"
        ]
    );
}

#[test]
fn c_registering_a_consumer_enables_the_spawn_sequences_once() {
    let mut env = Env::of(&enabling());
    let mut p = env.provider("P");
    assert!(!p.registered());
    p.register_consumer(&mut env.vm).unwrap();
    assert!(p.registered());
    // Every sequence disabled first (no event), then A and C enabled in order.
    assert_eq!(
        p.trace,
        [
            "OnBehaviorSequenceEnabled -> On",
            "OnBehaviorSequenceEnabled -> On"
        ]
    );
    assert!(p.sequence_enabled("A") && !p.sequence_enabled("B") && p.sequence_enabled("C"));
    p.register_consumer(&mut env.vm).unwrap();
    assert_eq!(p.trace.len(), 2);
}

// ------------------------------------------------------------------------------------------------ variables, G2

/// Event `Go` (Output link `Instigator`, connection 1, to variable 0) -> T (Behavior_ToyInputs) with a Context link
/// to variable 1 (a named reference to variable 0), an Input link of `Ctx` to variable 0 and an Input link of
/// `Count` to variable 2 (an Int, 4 in the block); variable 3 is a named Kismet variable `Dummy`.
fn variables_spec(count_type: &'static str) -> Spec {
    let mut t = beh("T", "Behavior_ToyInputs", (0, 0));
    t.vars = (1, 3);
    t.tags.push(Tag::Int("Count", 9));
    t.tags.push(Tag::Float("Scale", -2.75));
    Spec::new(
        "P",
        vec![Seq {
            name: "S",
            enabled_on_spawn: Some(true),
            events: vec![Ev {
                vars: (0, 1),
                ..ev("Go", (0, 1))
            }],
            behaviors: vec![t],
            links: vec![(0, -1, 0.0)],
            variables: vec![
                ("Who", "BVAR_Object"),
                ("Who", "BVAR_NamedVariable"),
                ("N", count_type),
                ("Dummy", "BVAR_NamedKismetVariable"),
            ],
            vlinks: vec![
                VLink("Instigator", "BVARLINK_Output", 1, (0, 1)),
                VLink("Context", "BVARLINK_Context", 0, (1, 1)),
                VLink("Ctx", "BVARLINK_Input", 0, (2, 1)),
                VLink("Count", "BVARLINK_Input", 0, (3, 1)),
            ],
            linked: vec![0, 1, 0, 2],
            ..Seq::default()
        }],
        vec![Word::I(0), Word::I(4)],
    )
}

fn toy_object(env: &mut Env, name: &str) -> ObjRef {
    let c = env.vm.find_class("Core.Object").unwrap();
    env.vm.instantiate(c, name).unwrap()
}

#[test]
fn a_payload_publishes_by_connection_index_and_contexts_follow_it() {
    let mut env = Env::of(&variables_spec("BVAR_Int"));
    let mut p = env.provider("P");
    let pawn = toy_object(&mut env, "Pawn");
    let id = BehaviorId {
        sequence: 0,
        index: 0,
    };
    let own = toy_object(&mut env, "Own");
    // No Context-linked object yet: the list is empty (the behavior would not run).
    assert!(p.contexts(id, own).is_empty());
    p.fire_event(&mut env.vm, "Go", &DetMap::new(), -1, &[None, Some(pawn)])
        .unwrap();
    let who = &p.variables("S")[0];
    assert_eq!(who.live, Some(pawn));
    // A run-time object has no stock path.
    assert_eq!(who.object, "");
    // The Context link names variable 1, a named reference to variable 0.
    assert_eq!(p.contexts(id, own), [pawn]);
    // A payload shorter than the connection index publishes nothing; the old value stays.
    p.fire_event(&mut env.vm, "Go", &DetMap::new(), -1, &[Some(own)])
        .unwrap();
    assert_eq!(p.variables("S")[0].live, Some(pawn));
    // A null element clears it.
    p.fire_event(&mut env.vm, "Go", &DetMap::new(), -1, &[None, None])
        .unwrap();
    assert_eq!(p.variables("S")[0].live, None);
}

#[test]
fn a_behavior_without_a_context_link_runs_on_its_consumer() {
    let mut env = Env::of(&rules());
    let p = env.provider("ToyRules");
    let own = toy_object(&mut env, "Own");
    assert_eq!(
        p.contexts(
            BehaviorId {
                sequence: 0,
                index: 0
            },
            own
        ),
        [own]
    );
}

#[test]
fn named_kismet_variables_resolve_to_the_bound_object() {
    let mut env = Env::of(&variables_spec("BVAR_Int"));
    let mut p = env.provider("P");
    let dummy = toy_object(&mut env, "Dummy");
    let id = BehaviorId {
        sequence: 0,
        index: 0,
    };
    let link = VariableLink {
        property: "Context".into(),
        link_type: "BVARLINK_Context".into(),
        connection: 0,
        variables: vec![3],
    };
    assert!(p.linked_objects(id, &link).is_empty());
    p.bind_kismet_variable("Dummy", Some(dummy));
    assert_eq!(p.linked_objects(id, &link), [dummy]);
    p.bind_kismet_variable("Dummy", None);
    assert!(p.linked_objects(id, &link).is_empty());
}

#[test]
fn g2_context_data_inputs_are_bound_for_the_run_and_cleared_after() {
    let mut env = Env::of(&variables_spec("BVAR_Int"));
    let mut p = env.provider("P");
    let pawn = toy_object(&mut env, "Pawn");
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    let read = |vm: &Vm, o: ObjRef| -> (Value, Value) {
        let ctx = provider::property(&vm.space, o, "Ctx").unwrap().unwrap();
        (
            provider::field(&vm.space, &ctx, "ContextObject")
                .unwrap()
                .clone(),
            provider::field(&vm.space, &ctx, "BehaviorContext")
                .unwrap()
                .clone(),
        )
    };
    p.handle(
        "WillowGame.Behavior_ToyInputs",
        Handler::run(move |vm, p, id, _| {
            sink.borrow_mut().push(read(vm, p.behavior(id).object));
            Ok(Vec::new())
        }),
    );
    p.fire_event(&mut env.vm, "Go", &DetMap::new(), -1, &[None, Some(pawn)])
        .unwrap();
    assert_eq!(
        *seen.borrow(),
        [(Value::Object(Some(pawn)), Value::Byte(USE_CONTEXT_OBJECT))]
    );
    let object = p.behaviors("S")[0].object;
    assert_eq!(
        read(&env.vm, object),
        (Value::Object(None), Value::Byte(USE_CONTEXT_OBJECT))
    );
}

#[test]
fn int_inputs_read_the_variable_or_the_own_property() {
    let mut env = Env::of(&variables_spec("BVAR_Int"));
    let mut p = env.provider("P");
    let id = BehaviorId {
        sequence: 0,
        index: 0,
    };
    assert_eq!(p.int_input(&env.vm, id, "Count").unwrap(), (Some(4), true));
    // No link: the behavior's own property, a float truncated toward zero (the C++ int64_t cast); absent: 0.
    assert_eq!(
        p.int_input(&env.vm, id, "Scale").unwrap(),
        (Some(-2), false)
    );
    assert_eq!(
        p.int_input(&env.vm, id, "Missing").unwrap(),
        (Some(0), false)
    );
    assert!(p.errors.is_empty());
    // Linked to a variable of another type: an error, no value.
    let mut env = Env::of(&variables_spec("BVAR_Float"));
    let mut p = env.provider("P");
    assert_eq!(p.int_input(&env.vm, id, "Count").unwrap(), (None, true));
    assert_eq!(
        p.errors,
        ["int input T.Count is linked to a BVAR_Float variable"]
    );
    // An object input linked to a non-object variable.
    let mut env = Env::of(&variables_spec("BVAR_Int"));
    let mut p = env.provider("P");
    assert_eq!(
        p.object_input(&env.vm, id, "Ctx").unwrap(),
        Some(String::new())
    );
    assert_eq!(p.object_input(&env.vm, id, "Count").unwrap(), None);
    assert_eq!(
        p.errors,
        ["object input T.Count is linked to a BVAR_Int variable"]
    );
}

#[test]
fn an_unlinked_object_input_reads_the_behaviors_own_reference() {
    let mut env = Env::of(&rules());
    let mut p = env.provider("ToyRules");
    let cmp = BehaviorId {
        sequence: 0,
        index: 12,
    };
    assert_eq!(p.behavior(cmp).name, "CmpOff");
    // The resolved export's path in the file that holds it.
    assert_eq!(
        p.object_input(&env.vm, cmp, "ObjectA").unwrap(),
        Some("DamageType.ToyFire".into())
    );
    assert_eq!(
        p.object_input(&env.vm, cmp, "ObjectB").unwrap(),
        Some(String::new())
    );
}

// ------------------------------------------------------------------------------------------------ registry, boundary

#[test]
fn an_unregistered_class_is_not_yet_and_stops_its_branch() {
    let mut env = Env::of(&rules());
    let definition = env.definition("TestBpd", "ToyRules");
    let mut p = Provider::new(&mut env.vm, definition).unwrap();
    event(&mut env, &mut p, "Order");
    assert_eq!(p.trace, ["Order -> A", "Order -> B"]);
    assert_eq!(
        p.errors,
        [
            "unsupported behavior class WillowGame.Behavior_ToySame (A)",
            "unsupported behavior class WillowGame.Behavior_ToySame (B)"
        ]
    );
    assert_eq!(p.not_yet.len(), 2);
    assert_eq!(p.not_yet[0].missing, Missing::NotPorted);
    assert!(p.not_yet[0].what.contains("WillowGame.Behavior_ToySame"));
}

#[test]
fn the_vm_registry_runs_registered_classes_and_the_provider_may_override() {
    let mut env = Env::of(&rules());
    crate::registry::register(
        &mut env.vm,
        "WillowGame.Behavior_ToySame",
        Handler::run(|_, _, _, _| Ok(Vec::new())),
    )
    .unwrap();
    let definition = env.definition("TestBpd", "ToyRules");
    let mut p = Provider::new(&mut env.vm, definition).unwrap();
    event(&mut env, &mut p, "DefOn");
    assert!(p.errors.is_empty());
    assert_eq!(p.trace, ["DefOn -> CmpOn", "DefOn -> Dflt", "DefOn -> One"]);
    // A provider handler wins over the registry's CompareObject: select 0 instead.
    p.handle(
        crate::handlers::COMPARE_OBJECT,
        Handler::run(|_, _, _, _| Ok(vec![0])),
    );
    p.trace.clear();
    event(&mut env, &mut p, "DefOn");
    assert_eq!(
        p.trace,
        ["DefOn -> CmpOn", "DefOn -> Dflt", "DefOn -> Zero"]
    );
}

#[test]
fn boundary_behaviors_are_listed_with_their_fields_and_outputs() {
    let mut env = Env::of(&rules());
    let mut p = env.provider("ToyRules");
    p.report_at_boundary(
        crate::handlers::COMPARE_OBJECT,
        Some(Rc::new(|vm: &mut Vm, p: &mut Provider, id: BehaviorId| {
            let a = p.object_input(vm, id, "ObjectA")?.unwrap_or_default();
            Ok([("ObjectA".to_string(), a)].into_iter().collect())
        })),
        vec![0],
    );
    event(&mut env, &mut p, "DefOff");
    assert_eq!(p.trace, ["DefOff -> CmpOff", "DefOff -> Zero"]);
    assert_eq!(
        p.boundary,
        [
            "DefOff -> WillowGame.Behavior_CompareObject:CmpOff",
            "DefOff -> WillowGame.Behavior_ToySame:Zero"
        ]
    );
    let call = &p.boundary_calls[0];
    assert_eq!(
        (
            call.event.as_str(),
            call.sequence.as_str(),
            call.name.as_str()
        ),
        ("DefOff", "Rules", "CmpOff")
    );
    assert_eq!(
        call.fields.get("ObjectA").map(String::as_str),
        Some("DamageType.ToyFire")
    );
    assert!(p.boundary_calls[1].fields.is_empty());
}

#[test]
fn the_graph_prints_every_link_with_its_id_and_delay() {
    let mut env = Env::of(&delays(0.5, 0.25));
    let p = env.provider("P");
    assert_eq!(
        p.graph("s"),
        [
            "event Go -> A id=-1 delay=0.500000",
            "WillowGame.Behavior_ToySame:A -> B id=-1 delay=0.250000",
            "WillowGame.Behavior_ToySame:A -> C id=-1 delay=0.000000"
        ]
    );
    assert!(p.has_event("go"));
    assert!(!p.has_event("Stop"));
    assert!(p.graph("Nope").is_empty());
}

#[test]
fn sequences_are_addressed_by_their_first_name() {
    // Two sequences named S: lookups read the first, enabling changes both.
    let first = Seq {
        name: "S",
        enabled_on_spawn: Some(true),
        variables: vec![("X", "BVAR_Int")],
        ..Seq::default()
    };
    let second = Seq {
        name: "S",
        enabled_on_spawn: Some(false),
        ..Seq::default()
    };
    let mut env = Env::of(&Spec::new("P", vec![first, second], vec![Word::I(1)]));
    let mut p = env.provider("P");
    assert!(p.sequence_enabled("S"));
    assert_eq!(p.variables("S").len(), 1);
    assert_eq!(p.sequence_names(), ["S", "S"]);
    p.set_sequence_enabled(&mut env.vm, "S", false).unwrap();
    assert!(!p.sequence_enabled("S"));
}

// ------------------------------------------------------------------------------------------------ review r1

#[test]
fn a_failed_run_leaves_the_shared_definition_as_it_was() {
    let mut env = Env::of(&variables_spec("BVAR_Int"));
    let mut p = env.provider("P");
    let pawn = toy_object(&mut env, "Pawn");
    let object = p.behaviors("S")[0].object;
    let class = env.vm.space.heap.get(object).unwrap().class;
    let ctx = PropAddr::scalar(env.vm.space.prop(class, "Ctx").unwrap());
    let cooked = env.vm.space.read(object, ctx).unwrap().clone();
    assert_eq!(
        env.vm.defs.origin(object, ctx),
        ow_defs::overlay::ValueOrigin::Cooked
    );
    let seen = Rc::new(Cell::new(false));
    let sink = seen.clone();
    p.handle(
        "WillowGame.Behavior_ToyInputs",
        Handler::run(move |vm, p, id, _| {
            // The bind happened: the context object is in the definition while the handler runs.
            let o = p.behavior(id).object;
            let ctx = provider::property(&vm.space, o, "Ctx")?.unwrap();
            sink.set(
                provider::field(&vm.space, &ctx, "ContextObject") != Some(&Value::Object(None)),
            );
            Err(BehaviorError::Data("boom".into()))
        }),
    );
    let e = p
        .fire_event(&mut env.vm, "Go", &DetMap::new(), -1, &[None, Some(pawn)])
        .unwrap_err();
    assert_eq!(e.to_string(), "boom");
    assert!(seen.get(), "the handler ran with the input bound");
    // Back to the cooked value (selector included) and still a cooked element.
    assert_eq!(env.vm.space.read(object, ctx).unwrap(), &cooked);
    assert_eq!(
        env.vm.defs.origin(object, ctx),
        ow_defs::overlay::ValueOrigin::Cooked
    );

    // A boundary description that fails: the same.
    p.report_at_boundary(
        "WillowGame.Behavior_ToyInputs",
        Some(Rc::new(|_: &mut Vm, _: &mut Provider, _: BehaviorId| {
            Err(BehaviorError::Data("no fields".into()))
        })),
        Vec::new(),
    );
    let e = p
        .fire_event(&mut env.vm, "Go", &DetMap::new(), -1, &[None, Some(pawn)])
        .unwrap_err();
    assert_eq!(e.to_string(), "no fields");
    assert_eq!(env.vm.space.read(object, ctx).unwrap(), &cooked);
    // The successful path still leaves the C++ state: ContextObject None, the selector 4.
    p.report_at_boundary("WillowGame.Behavior_ToyInputs", None, Vec::new());
    p.fire_event(&mut env.vm, "Go", &DetMap::new(), -1, &[None, Some(pawn)])
        .unwrap();
    let after = env.vm.space.read(object, ctx).unwrap().clone();
    assert_eq!(
        provider::field(&env.vm.space, &after, "ContextObject"),
        Some(&Value::Object(None))
    );
    assert_eq!(
        provider::field(&env.vm.space, &after, "BehaviorContext"),
        Some(&Value::Byte(USE_CONTEXT_OBJECT))
    );
}

#[test]
fn a_failed_disabled_event_leaves_the_sequence_enabled() {
    let mut env = Env::of(&enabling());
    let mut p = env.provider("P");
    p.handle(
        "WillowGame.Behavior_ToySame",
        Handler::run(|_, _, _, _| Err(BehaviorError::Data("refused".into()))),
    );
    let e = p.set_sequence_enabled(&mut env.vm, "C", false).unwrap_err();
    assert_eq!(e.to_string(), "refused");
    assert!(p.sequence_enabled("C"));
}

#[test]
fn a_latent_wait_and_its_own_delayed_link_due_together_run_link_first() {
    // T selects its id-0 link to N (delay 0.5) and waits 0.5: N is parked before T, so it runs first.
    let spec = Spec::new(
        "P",
        vec![Seq {
            name: "S",
            enabled_on_spawn: Some(true),
            events: vec![ev("Go", (0, 1))],
            behaviors: vec![
                beh("T", "Behavior_ToyInputs", (1, 1)),
                beh("N", "Behavior_ToySame", (0, 0)),
            ],
            links: vec![(0, -1, 0.0), (1, 0, 0.5)],
            ..Seq::default()
        }],
        vec![],
    );
    let mut env = Env::of(&spec);
    let mut p = env.provider("P");
    let seen = Rc::new(RefCell::new(Vec::new()));
    p.handle(
        "WillowGame.Behavior_ToyInputs",
        latent_handler(0.5, seen.clone()),
    );
    event(&mut env, &mut p, "Go");
    assert_eq!(p.trace, ["Go -> T"]);
    p.tick(&mut env.vm, 0.5).unwrap();
    assert_eq!(p.trace, ["Go -> T", "Go -> N", "Go -> T"]);
    assert_eq!(*seen.borrow(), [(true, true, 0), (false, true, 1)]);
}

// Invented handles and properties only; no native/gameplay parity claim.
fn explicit_context(env: &mut Env, p: &Provider, id: BehaviorId, value: Option<ObjRef>) {
    Provider::write_context_field(
        &mut env.vm,
        p.behavior(id).object,
        "Context",
        "ContextObject",
        Value::Object(value),
    )
    .unwrap();
}

#[test]
fn runner_context_explicit_object_without_link_and_reference_unchanged() {
    let mut env = Env::of(&rules());
    let p = env.provider("ToyRules");
    let id = BehaviorId { sequence: 0, index: 12 };
    let own = toy_object(&mut env, "Own");
    let other = toy_object(&mut env, "Explicit");
    explicit_context(&mut env, &p, id, Some(other));
    // Selector Self (zero) must not replace runner context selection.
    assert_eq!(p.contexts_for(&env.vm, id, own).unwrap(), [other]);
    env.vm.parity = ow_core::ParityMode::Reference;
    assert_eq!(p.contexts_for(&env.vm, id, own).unwrap(), [own]);
}

#[test]
fn runner_context_link_wins_even_when_empty() {
    let mut env = Env::of(&variables_spec("BVAR_Int"));
    let mut p = env.provider("P");
    let id = BehaviorId {
        sequence: 0,
        index: 0,
    };
    let own = toy_object(&mut env, "Own");
    let explicit = toy_object(&mut env, "Explicit");
    let linked = toy_object(&mut env, "Linked");
    explicit_context(&mut env, &p, id, Some(explicit));
    assert!(p.contexts_for(&env.vm, id, own).unwrap().is_empty());
    p.fire_event(&mut env.vm, "Go", &DetMap::new(), -1, &[None, Some(linked)])
        .unwrap();
    assert_eq!(p.contexts_for(&env.vm, id, own).unwrap(), [linked]);
}

#[test]
fn runner_context_null_and_missing_fall_back_to_own() {
    let mut env = Env::of(&rules());
    let p = env.provider("ToyRules");
    let own = toy_object(&mut env, "Own");
    for index in [12, 1] {
        assert_eq!(
            p.contexts_for(&env.vm, BehaviorId { sequence: 0, index }, own)
                .unwrap(),
            [own]
        );
    }
}

#[test]
fn runner_context_refuses_stale_consumer_behavior_and_link() {
    use ow_object::heap::HeapError;
    use ow_vm::error::VmError;
    let mut env = Env::of(&variables_spec("BVAR_Int"));
    let mut p = env.provider("P");
    let id = BehaviorId {
        sequence: 0,
        index: 0,
    };
    let gone = toy_object(&mut env, "Gone");
    env.vm.space.free(gone).unwrap();
    let own = toy_object(&mut env, "Replacement");
    p.data.sequences[0].variables[0].live = Some(gone);
    assert_eq!(
        p.contexts_for(&env.vm, id, own),
        Err(VmError::Heap(HeapError::Stale(gone)))
    );
    p.data.sequences[0].behaviors[0].variables.clear();
    assert_eq!(
        p.contexts_for(&env.vm, id, gone),
        Err(VmError::Heap(HeapError::Stale(gone)))
    );
    let behavior = p.behavior(id).object;
    env.vm.space.free(behavior).unwrap();
    assert_eq!(
        p.contexts_for(&env.vm, id, own),
        Err(VmError::Heap(HeapError::Stale(behavior)))
    );
}

#[test]
fn runner_context_refuses_unchecked_stale_explicit_object() {
    use ow_object::heap::HeapError;
    use ow_vm::error::VmError;
    let mut env = Env::of(&rules());
    let p = env.provider("ToyRules");
    let id = BehaviorId {
        sequence: 0,
        index: 12,
    };
    let own = toy_object(&mut env, "Own");
    let gone = toy_object(&mut env, "Gone");
    explicit_context(&mut env, &p, id, Some(gone));
    let behavior = p.behavior(id).object;
    let class = env.vm.space.heap.get(behavior).unwrap().class;
    let addr = PropAddr::scalar(env.vm.space.prop(class, "Context").unwrap());
    let stale_value = env.vm.space.read(behavior, addr).unwrap().clone();
    env.vm.space.free(gone).unwrap();
    // Simulate unchecked storage corruption; normal writes refuse a stale reference.
    *env.vm.space.heap.slot_mut(&env.vm.space.classes, behavior, addr).unwrap() = stale_value;
    assert_eq!(
        p.contexts_for(&env.vm, id, own),
        Err(VmError::Heap(HeapError::Stale(gone)))
    );
}

// ------------------------------------------------------------------------------------------ certain first behavior

/// The behavior a firing runs before any other (review r2 of #456): the first link without a delay in firing order,
/// and only when it has no variable links; a closed event gives none. A NaN delay starts at once in the firing and in
/// the scan alike ([`starts_now`]), and a parked link is skipped by both.
#[test]
fn the_certain_first_behavior_is_the_one_the_firing_runs_first() {
    let id = |index| BehaviorId { sequence: 0, index };
    let mut env = Env::of(&rules());
    let p = env.provider("ToyRules");
    assert_eq!(p.certain_first_behavior("Order", -1), Some(id(0)), "A, ahead of B");
    assert_eq!(p.certain_first_behavior("Filter", 5), Some(id(10)), "the link id filter: C5");
    assert_eq!(p.certain_first_behavior("Off", -1), None, "a disabled event runs nothing");

    let spec = Spec::new(
        "Delays",
        vec![Seq {
            name: "S",
            enabled_on_spawn: Some(true),
            events: vec![ev("Go", (0, 3))],
            behaviors: vec![beh("A", "Behavior_ToySame", (0, 0)), beh("B", "Behavior_ToySame", (0, 0)),
                beh("C", "Behavior_ToySame", (0, 0))],
            links: vec![(0, -1, 5.0), (1, -1, f32::NAN), (2, -1, 0.0)],
            ..Seq::default()
        }],
        vec![],
    );
    let mut env = Env::of(&spec);
    let mut p = env.provider("Delays");
    assert_eq!(p.certain_first_behavior("Go", -1), Some(id(1)), "A is parked; B's NaN delay starts at once");
    event(&mut env, &mut p, "Go");
    assert_eq!(p.trace, ["Go -> B", "Go -> C"], "the firing agrees");
    assert!(starts_now(f64::NAN) && starts_now(0.0) && starts_now(-1.0) && !starts_now(0.5));

    // T has a Context link and input links: its context is bound by the firing, so it is not certain.
    let mut env = Env::of(&variables_spec("BVAR_Int"));
    let p = env.provider("P");
    assert_eq!(p.certain_first_behavior("Go", -1), None);
}

// ------------------------------------------------------------------------- script Parameters (IMPL-ACTIVATE-EVENT-PARAMETERS)

/// Event `OnSelectedTarget` of the enabled sequence `S` with four Output links: connection 0 to the object variable
/// `Siren`, 1 to the object variable `Target`, 2 to the Int variable `N`, 3 to the named Kismet variable `Dummy`.
/// Sequence `Later` (disabled) has the same event with an Output link of connection 0 to the Int variable `N2`.
fn parameters_spec() -> Spec {
    Spec::new(
        "P",
        vec![
            Seq {
                name: "S",
                enabled_on_spawn: Some(true),
                events: vec![Ev { vars: (0, 4), ..ev("OnSelectedTarget", (0, 0)) }],
                variables: vec![
                    ("Siren", "BVAR_Object"),
                    ("Target", "BVAR_Object"),
                    ("N", "BVAR_Int"),
                    ("Dummy", "BVAR_NamedKismetVariable"),
                ],
                vlinks: vec![
                    VLink("SkillInstigator", "BVARLINK_Output", 0, (0, 1)),
                    VLink("SkillTarget", "BVARLINK_Output", 1, (1, 1)),
                    VLink("Count", "BVARLINK_Output", 2, (2, 1)),
                    VLink("Kis", "BVARLINK_Output", 3, (3, 1)),
                ],
                linked: vec![0, 1, 2, 3],
                ..Seq::default()
            },
            Seq {
                name: "Later",
                enabled_on_spawn: Some(false),
                events: vec![Ev { vars: (0, 1), ..ev("OnSelectedTarget", (0, 0)) }],
                variables: vec![("N2", "BVAR_Int")],
                vlinks: vec![VLink("Count", "BVARLINK_Output", 0, (0, 1))],
                linked: vec![0],
                ..Seq::default()
            },
        ],
        vec![Word::I(0), Word::I(0), Word::I(0), Word::I(0)],
    )
}

fn typed(var_type: &str) -> PayloadElement {
    PayloadElement { var_type: var_type.to_string(), object: None }
}

fn fire_with(p: &mut Provider, env: &mut Env, payload: Option<&[PayloadElement]>) {
    if let Some(payload) = payload {
        p.check_payload("OnSelectedTarget", payload).unwrap();
    }
    p.fire_event_with(&mut env.vm, "OnSelectedTarget", &DetMap::new(), -1, payload).unwrap();
}

/// Link 9's shape: two `BVAR_Object` elements bind by connection index into the two object variables, after the
/// trigger count update.
#[test]
fn object_parameters_bind_by_connection_index() {
    let mut env = Env::of(&parameters_spec());
    let mut p = env.provider("P");
    let (maya, target) = (toy_object(&mut env, "Maya"), toy_object(&mut env, "Target"));
    fire_with(&mut p, &mut env, Some(&[PayloadElement::object(Some(maya)), PayloadElement::object(Some(target))]));
    assert_eq!(p.variables("S")[0].live, Some(maya));
    assert_eq!(p.variables("S")[1].live, Some(target));
    assert_eq!(p.event_trigger_count("OnSelectedTarget"), 1);
}

/// Section 3: a `BVAR_None` element, a short payload and an empty payload bind nothing (the variables keep their
/// values); an object element holding None binds None; an element of another type than the variable's binds nothing.
/// Without Parameters the port keeps its existing behavior (the object outputs are cleared): the recorded divergence.
#[test]
fn parameters_that_bind_nothing_keep_the_variables() {
    let mut env = Env::of(&parameters_spec());
    let mut p = env.provider("P");
    let (maya, target) = (toy_object(&mut env, "Maya"), toy_object(&mut env, "Target"));
    fire_with(&mut p, &mut env, Some(&[PayloadElement::object(Some(maya)), PayloadElement::object(Some(target))]));
    fire_with(&mut p, &mut env, Some(&[PayloadElement::object(Some(maya)), typed("BVAR_None")]));
    assert_eq!(p.variables("S")[1].live, Some(target), "a BVAR_None element binds nothing");
    fire_with(&mut p, &mut env, Some(&[PayloadElement::object(Some(maya))]));
    assert_eq!(p.variables("S")[1].live, Some(target), "a link past the payload is skipped");
    fire_with(&mut p, &mut env, Some(&[typed("BVAR_Float")]));
    assert_eq!(p.variables("S")[0].live, Some(maya), "a Float element does not bind an object variable");
    fire_with(&mut p, &mut env, Some(&[]));
    assert_eq!(p.variables("S")[0].live, Some(maya), "an empty payload binds nothing");
    fire_with(&mut p, &mut env, Some(&[PayloadElement::object(Some(maya)), PayloadElement::object(None)]));
    assert_eq!(p.variables("S")[1].live, None, "an object element holding None binds None");
    assert_eq!(p.event_trigger_count("OnSelectedTarget"), 6);
    // No Parameters: the port's existing route clears the object outputs (out of scope, the hub's ruling).
    fire_with(&mut p, &mut env, None);
    assert_eq!(p.variables("S")[0].live, None);
}

/// The preflight refuses the bindings the port does not do, before anything fires: a same-typed Int (`NotPorted`),
/// also in a sequence that is disabled now (the hub's ruling on #549: a behavior could enable it during the firing),
/// and a named Kismet variable target (`NotRead`).
#[test]
fn the_preflight_refuses_before_any_firing() {
    let mut env = Env::of(&parameters_spec());
    let p = env.provider("P");
    let maya = toy_object(&mut env, "Maya");
    let o = PayloadElement::object(Some(maya));
    let missing = |r: Result<(), BehaviorError>| r.unwrap_err().not_yet().map(|n| (n.missing, n.what.to_string())).unwrap();
    let (m, what) = missing(p.check_payload("OnSelectedTarget", &[o.clone(), o.clone(), typed("BVAR_Int")]));
    assert_eq!(m, Missing::NotPorted, "{what}");
    assert!(what.contains("binding a BVAR_Int Parameters entry"), "{what}");
    let (m, what) = missing(p.check_payload("OnSelectedTarget", &[typed("BVAR_Int")]));
    assert_eq!(m, Missing::NotPorted, "the disabled sequence's Int variable: {what}");
    let (m, what) = missing(p.check_payload("OnSelectedTarget", &[o.clone(), o.clone(), typed("BVAR_None"), typed("BVAR_None")]));
    assert_eq!(m, Missing::NotRead, "{what}");
    assert!(what.contains("into a Kismet variable"), "{what}");
    assert!(!what.starts_with("stopped after partial writes: "), "{what}");
    assert_eq!(p.event_trigger_count("OnSelectedTarget"), 0, "nothing fired");
    // Another event's links are not looked at.
    p.check_payload("Elsewhere", &[typed("BVAR_Int")]).unwrap();
}

/// The binding rule's table (section 2, item 3, and section 4): attribute and direction targets are `NotRead`,
/// same-typed bool, int, float and vector `NotPorted`, a mismatch nothing.
#[test]
fn the_binding_rule_table() {
    assert_eq!(payload_binding("BVAR_Object", "BVAR_Object"), Ok(true));
    assert_eq!(payload_binding("BVAR_Object", "BVAR_Int"), Ok(false));
    assert_eq!(payload_binding("BVAR_None", "BVAR_Object"), Ok(false));
    assert_eq!(payload_binding("BVAR_Int", "BVAR_Attribute"), Ok(false));
    for t in ["BVAR_Bool", "BVAR_Int", "BVAR_Float", "BVAR_Vector"] {
        assert_eq!(payload_binding(t, t).unwrap_err().missing, Missing::NotPorted, "{t}");
    }
    assert_eq!(payload_binding("BVAR_Float", "BVAR_Attribute").unwrap_err().missing, Missing::NotRead);
    assert_eq!(payload_binding("BVAR_Vector", "BVAR_DirectionVector").unwrap_err().missing, Missing::NotRead);
    assert_eq!(payload_binding("BVAR_Object", "BVAR_NamedKismetVariable").unwrap_err().missing, Missing::NotRead);
}

/// Review r1 of #555, item 1 (the hub's ruling): the existing object-payload route (`fire_event`) keeps its rule in both
/// modes. An element binds into the object variables its link targets, and an Int or named Kismet target is skipped
/// silently, with no refusal.
#[test]
fn the_object_payload_route_still_skips_other_targets() {
    let mut env = Env::of(&parameters_spec());
    let mut p = env.provider("P");
    let (maya, other) = (toy_object(&mut env, "Maya"), toy_object(&mut env, "Other"));
    p.fire_event(&mut env.vm, "OnSelectedTarget", &DetMap::new(), -1, &[Some(maya), None, Some(other), Some(other)])
        .unwrap();
    assert_eq!(p.variables("S")[0].live, Some(maya));
    assert_eq!(p.variables("S")[1].live, None);
    assert_eq!(p.event_trigger_count("OnSelectedTarget"), 1);
}
