//! Owner: lane W5-DIALOG. The mover rules on an invented `Engine` package (AGENTS.md rule 9). AI-assisted (Claude
//! Code).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_package::testkit::ints;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{Asm, CPF_OPT, FunctionSpec, Param, ScriptToy};

use super::*;

const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
const NATIVE: u32 = 0x400 | 0x20000;

/// What the invented package varies.
#[derive(Debug, Clone)]
struct Opts {
    /// `InterpolationFinished` also calls a native nothing implements (it logs `UNIMPLEMENTED`).
    broken: bool,
    /// `InterpolationFinished` arms a looping timer.
    looping: bool,
    /// The name `InterpolationFinished` arms (`Callback` names the callback).
    timer: &'static str,
    /// The name of `InterpolationFinished`'s first parameter.
    finished_param: &'static str,
    /// `SeqAct_Interp` declares `bReversePlayback`.
    reverse_prop: bool,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            broken: false,
            looping: false,
            timer: "Callback",
            finished_param: "InterpAction",
            reverse_prop: true,
        }
    }
}

fn opt(kind: &str, name: &str, type_ref: i32) -> Param {
    Param {
        flags: CPF_OPT,
        type_ref,
        ..Param::new(kind, name)
    }
}

fn native(t: &mut ScriptToy, owner: i32, name: &str, params: &[Param]) -> i32 {
    let spec = FunctionSpec::new(NATIVE, name);
    t.function(owner, name, params, None, &[], &spec, |_| None)
        .function
}

fn script(
    t: &mut ScriptToy,
    owner: i32,
    name: &str,
    params: &[Param],
    body: impl FnOnce(&mut Asm),
) -> i32 {
    let spec = FunctionSpec::new(SCRIPT, name);
    t.function(owner, name, params, None, &[], &spec, |_| {
        let mut a = Asm::new();
        body(&mut a);
        a.stmt().return_nothing();
        a.end();
        Some(a)
    })
    .function
}

/// A placed actor's payload: the 26-byte prefix, then `tags`, then `None`.
fn placed(t: &mut ScriptToy, tags: &[Vec<u8>]) -> Vec<u8> {
    let mut payload = vec![0u8; 26];
    for tag in tags {
        payload.extend_from_slice(tag);
    }
    payload.extend_from_slice(&t.none);
    payload
}

/// An ordinary object's payload: one word, then `tags`, then `None`.
fn ordinary(t: &mut ScriptToy, tags: &[Vec<u8>]) -> Vec<u8> {
    let mut payload = ints(&[0]);
    for tag in tags {
        payload.extend_from_slice(tag);
    }
    payload.extend_from_slice(&t.none);
    payload
}

fn package(o: &Opts) -> Vec<u8> {
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
        "StrProperty",
        "ArrayProperty",
        "StructProperty",
    ]);
    let object = t.class("Object", 0);
    // Toy Kismet declarations carrying the property names the executor reads (as the W5-KISMET tests do).
    let s_in = toy_struct(&mut t, object, "SeqOpInputLink", None);
    t.prop("Str", s_in, "LinkDesc", 0, 0);
    t.prop("Bool", s_in, "bDisabled", 0, 0);
    t.prop("Float", s_in, "ActivateDelay", 0, 0);
    let s_link = toy_struct(&mut t, object, "SeqOpOutputInputLink", None);
    t.prop("Object", s_link, "LinkedOp", 0, object);
    t.prop("Int", s_link, "InputLinkIdx", 0, 0);
    let s_out = toy_struct(&mut t, object, "SeqOpOutputLink", None);
    toy_array_of(&mut t, s_out, "Links", "Struct", s_link);
    t.prop("Str", s_out, "LinkDesc", 0, 0);
    t.prop("Bool", s_out, "bDisabled", 0, 0);
    t.prop("Float", s_out, "ActivateDelay", 0, 0);
    let s_var = toy_struct(&mut t, object, "SeqVarLink", None);
    t.prop("Str", s_var, "LinkDesc", 0, 0);
    toy_array_of(&mut t, s_var, "LinkedVariables", "Object", object);
    let op = t.class("SequenceOp", object);
    toy_array_of(&mut t, op, "InputLinks", "Struct", s_in);
    toy_array_of(&mut t, op, "OutputLinks", "Struct", s_out);
    toy_array_of(&mut t, op, "VariableLinks", "Struct", s_var);
    t.prop("Int", op, "ActivateCount", 0, 0);
    let sequence = t.class("Sequence", op);
    let event = t.class("SequenceEvent", op);
    t.prop("Int", event, "TriggerCount", 0, 0);
    t.prop("Object", event, "Originator", 0, object);
    let remote = t.class("SeqEvent_RemoteEvent", event);
    t.prop("Name", remote, "EventName", 0, 0);
    let seq_action = t.class("SequenceAction", op);
    let toggle = t.class("SeqAct_ToyToggle", seq_action); // invented: a world action the host gets
    let var_object = t.class("SeqVar_Object", object);
    t.prop("Object", var_object, "ObjValue", 0, object);
    let mystery = native(&mut t, object, "Mystery", &[]);
    let actor = t.class("Actor", object);
    let set_timer = native(
        &mut t,
        actor,
        "SetTimer",
        &[
            Param::new("Float", "InRate"),
            opt("Bool", "inbLoop", 0),
            opt("Name", "inTimerFunc", 0),
            opt("Object", "inObj", object),
        ],
    );
    let clear_timer = native(
        &mut t,
        actor,
        "ClearTimer",
        &[
            opt("Name", "inTimerFunc", 0),
            opt("Object", "inObj", object),
        ],
    );
    let audio = t.class("AudioComponent", object);
    t.prop("Object", audio, "SoundCue", 0, object);
    native(&mut t, audio, "Stop", &[]);
    let action = t.class("SeqAct_Interp", seq_action);
    if o.reverse_prop {
        t.prop("Bool", action, "bReversePlayback", 0, 0);
    }
    let mover = t.class("InterpActor", actor);
    let checkpoint = t.prop("Bool", mover, "bShouldSaveForCheckpoint", 0, 0);
    let delay = t.prop("Float", mover, "Delay", 0, 0);
    t.prop("Object", mover, "AmbientSoundComponent", 0, audio);
    let callback = t.p.fname("Callback", 0);
    let armed = t.p.fname(o.timer, 0);
    let (broken, looping) = (o.broken, o.looping);
    script(
        &mut t,
        mover,
        "InterpolationStarted",
        &[
            Param {
                type_ref: object,
                ..Param::new("Object", "InterpAction")
            },
            Param {
                type_ref: object,
                ..Param::new("Object", "GroupInst")
            },
        ],
        |a| {
            // bShouldSaveForCheckpoint = true; ClearTimer('Callback');
            a.stmt().raw(&[0x14]).instance(checkpoint).raw(&[0x27]);
            a.stmt().final_function(clear_timer, |a| {
                a.raw(&[0x21]).raw(&callback);
            });
        },
    );
    script(
        &mut t,
        mover,
        "InterpolationFinished",
        &[Param {
            type_ref: object,
            ..Param::new("Object", o.finished_param)
        }],
        |a| {
            // SetTimer(Delay, looping, '<timer>'); [Mystery();]
            a.stmt().final_function(set_timer, |a| {
                a.instance(delay);
                a.raw(&[if looping { 0x27 } else { 0x28 }]);
                a.raw(&[0x21]).raw(&armed);
            });
            if broken {
                a.stmt().final_function(mystery, |_| {});
            }
        },
    );
    script(&mut t, mover, "Callback", &[], |a| {
        // bShouldSaveForCheckpoint = false;
        a.stmt().raw(&[0x14]).instance(checkpoint).raw(&[0x28]);
    });

    let delay_tag = t.p.float_tag("Delay", 0.5);
    let payload = placed(&mut t, &[delay_tag]);
    let placed_mover = t.p.add_export(mover, "PlacedMover", payload);
    let fast = t.p.float_tag("Delay", 0.1);
    let payload = placed(&mut t, &[fast]);
    t.p.add_export(mover, "FastMover", payload);
    let slow = t.p.float_tag("Delay", 5000.0);
    let payload = placed(&mut t, &[slow]);
    t.p.add_export(mover, "SlowMover", payload);
    let payload = ordinary(&mut t, &[]);
    t.p.add_export(action, "PlacedAction", payload);
    // A sequence `Seq` owning the mover's action: remote events OpenDoor (Play, and the lamp on) and CloseDoor
    // (Reverse); the action's Completed turns the lamp off; its Data variable names PlacedMover.
    let base = t.p.next_export();
    let (seq, open, close, inner, lamp, var) =
        (base, base + 1, base + 2, base + 3, base + 4, base + 5);
    let payload = ordinary(&mut t, &[]);
    assert_eq!(t.p.add_export(sequence, "Seq", payload), seq);
    let in_seq = |t: &mut ScriptToy, class: i32, name: &str, tags: &[Vec<u8>]| {
        let payload = ordinary(t, tags);
        let mut e = ow_package::testkit::Export::new(class, name, payload);
        e.outer = seq;
        t.p.add_export_with(e)
    };
    let inputs = |t: &mut ScriptToy, descs: &[&str]| {
        let mut body = Vec::new();
        for d in descs {
            body.extend(t.p.str_tag("LinkDesc", d));
            body.extend(t.none);
        }
        t.p.array_tag("InputLinks", i32::try_from(descs.len()).unwrap(), &body)
    };
    let outputs = |t: &mut ScriptToy, outs: &[(&str, &[(i32, i32)])]| {
        let mut body = Vec::new();
        for (desc, to) in outs {
            let mut targets = Vec::new();
            for &(target, input) in *to {
                targets.extend(t.p.object_tag("LinkedOp", target));
                targets.extend(t.p.int_tag("InputLinkIdx", input));
                targets.extend(t.none);
            }
            body.extend(t.p.array_tag("Links", i32::try_from(to.len()).unwrap(), &targets));
            body.extend(t.p.str_tag("LinkDesc", desc));
            body.extend(t.none);
        }
        t.p.array_tag("OutputLinks", i32::try_from(outs.len()).unwrap(), &body)
    };
    let open_tags = [
        t.p.name_tag("EventName", "OpenDoor"),
        outputs(&mut t, &[("Out", &[(inner, 0), (lamp, 0)])]),
    ];
    assert_eq!(in_seq(&mut t, remote, "OpenEvent", &open_tags), open);
    let close_tags = [
        t.p.name_tag("EventName", "CloseDoor"),
        outputs(&mut t, &[("Out", &[(inner, 1)])]),
    ];
    assert_eq!(in_seq(&mut t, remote, "CloseEvent", &close_tags), close);
    let vars = {
        let mut body = t.p.str_tag("LinkDesc", "Data");
        body.extend(t.p.array_tag("LinkedVariables", 1, &var.to_le_bytes()));
        body.extend(t.none);
        t.p.array_tag("VariableLinks", 1, &body)
    };
    let inner_tags = [
        inputs(&mut t, &["Play", "Reverse", "Stop"]),
        outputs(&mut t, &[("Completed", &[(lamp, 1)]), ("Reversed", &[])]),
        vars,
    ];
    assert_eq!(in_seq(&mut t, action, "InnerAction", &inner_tags), inner);
    let lamp_tags = [inputs(&mut t, &["Turn On", "Turn Off"])];
    assert_eq!(in_seq(&mut t, toggle, "Lamp", &lamp_tags), lamp);
    let var_tags = [t.p.object_tag("ObjValue", placed_mover)];
    assert_eq!(in_seq(&mut t, var_object, "MoverVar", &var_tags), var);
    let payload = placed(&mut t, &[]);
    t.p.add_export(actor, "PlainActor", payload);
    t.p.add_export(mover, "ShortMover", vec![0u8; 25]);
    let payload = ordinary(&mut t, &[]);
    let cue = t.p.add_export(object, "Cue", payload);
    let payload = ordinary(&mut t, &[]);
    let idle = t.p.add_export(audio, "IdleAudio", payload);
    let cue_tag = t.p.object_tag("SoundCue", cue);
    let payload = ordinary(&mut t, &[cue_tag]);
    let loud = t.p.add_export(audio, "LoudAudio", payload);
    for (name, component) in [("QuietMover", idle), ("LoudMover", loud)] {
        let tag = t.p.object_tag("AmbientSoundComponent", component);
        let payload = placed(&mut t, &[tag]);
        t.p.add_export(mover, name, payload);
    }
    t.p.build()
}

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A cooked folder holding the invented `Engine.upk`, removed when dropped.
struct Cooked(PathBuf);

impl Drop for Cooked {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn cooked(o: &Opts) -> Cooked {
    let dir = std::env::temp_dir().join(format!(
        "ow-mover-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Engine.upk"), package(o)).unwrap();
    Cooked(dir)
}

/// Runs `f` on a thread with the VM's stack.
fn on_stack(f: impl FnOnce() + Send) {
    ow_vm::interp::on_vm_stack(f).unwrap();
}

fn bind(dir: &Cooked, actor: &str, action: &str) -> Result<Mover, MoverError> {
    Mover::new(
        dir.0.clone(),
        "Engine",
        actor,
        action,
        ParityMode::Off,
        StubPolicy::LogAndZero,
    )
}

fn mover(dir: &Cooked, actor: &str) -> Mover {
    bind(dir, actor, "PlacedAction").unwrap()
}

fn text(r: &MoverResult) -> String {
    r.error
        .as_ref()
        .map(MoverError::cpp_text)
        .unwrap_or_default()
}

fn reverse_flag(m: &Mover) -> bool {
    let class = m.vm.space.heap.get(m.action).unwrap().class;
    let p = m.vm.space.prop(class, "bReversePlayback").unwrap();
    m.vm.space.read(m.action, PropAddr::scalar(p)).unwrap() == &Value::Bool(true)
}

#[test]
fn the_probe_sequence_runs_the_installed_notifications_and_their_timer() {
    let dir = cooked(&Opts::default());
    on_stack(|| {
        let mut m = mover(&dir, "PlacedMover");
        assert!(m.loading_diagnostics().is_empty());
        for reverse in [false, true] {
            let start = m.notify(false, reverse);
            assert_eq!((start.error.clone(), start.checkpoint), (None, true));
            assert!(start.steps > 0);
            assert_eq!(reverse_flag(&m), reverse);
            let finish = m.notify(true, reverse);
            assert_eq!((finish.error.clone(), finish.checkpoint), (None, true));
            // InterpolationFinished armed 'Callback' at the clock plus Delay (0.5 s).
            let due = m.clock() + 0.5;
            assert_eq!(m.timers(), vec![("callback".to_string(), due)]);
            let timers = m.advance(10.0);
            assert_eq!((timers.error.clone(), timers.checkpoint), (None, false));
            assert!(timers.steps > 0);
            assert!(m.timers().is_empty());
        }
        assert_eq!(m.clock(), 20.0);
    });
}

#[test]
fn a_call_that_logs_fails_and_is_rolled_back() {
    let dir = cooked(&Opts {
        broken: true,
        ..Opts::default()
    });
    on_stack(|| {
        let mut m = mover(&dir, "PlacedMover");
        assert_eq!(m.notify(false, false).error, None);
        let finish = m.notify(true, false);
        assert!(text(&finish).starts_with("UNIMPLEMENTED "), "{finish:?}");
        assert!(matches!(finish.error, Some(MoverError::Logged(_))));
        assert!(!finish.checkpoint);
        assert!(finish.steps > 0);
        // The timer it armed is rolled back with it: nothing runs, the checkpoint flag is the start's.
        assert!(m.timers().is_empty());
        let timers = m.advance(10.0);
        assert_eq!(
            (timers.error, timers.steps, timers.checkpoint),
            (None, 0, true)
        );
    });
}

#[test]
fn under_a_refusing_stub_policy_an_unbound_native_fails_the_call_and_is_rolled_back() {
    let dir = cooked(&Opts {
        broken: true,
        ..Opts::default()
    });
    on_stack(|| {
        let mut m = Mover::new(
            dir.0.clone(),
            "Engine",
            "PlacedMover",
            "PlacedAction",
            ParityMode::Off,
            StubPolicy::refuse_all(),
        )
        .unwrap();
        let finish = m.notify(true, false);
        assert!(matches!(
            finish.error,
            Some(MoverError::Vm(VmError::UnboundNative(_)))
        ));
        assert!(m.timers().is_empty());
        assert!(m.vm().log.is_empty(), "refused, not logged");
    });
}

#[test]
fn a_failed_reverse_restores_the_action_flag() {
    let dir = cooked(&Opts {
        broken: true,
        ..Opts::default()
    });
    on_stack(|| {
        let mut m = mover(&dir, "PlacedMover");
        assert!(m.notify(true, true).error.is_some());
        assert!(!reverse_flag(&m));
    });
}

#[test]
fn the_binding_refuses_what_the_cxx_refuses() {
    let dir = cooked(&Opts::default());
    on_stack(|| {
        let err = |actor: &str, action: &str| bind(&dir, actor, action).unwrap_err().cpp_text();
        // Swapped: the action is too short for a placed actor's prefix.
        assert_eq!(
            err("PlacedAction", "PlacedMover"),
            "object property prefix outside export"
        );
        // 25 bytes: shorter than the 26-byte prefix.
        assert_eq!(
            err("ShortMover", "PlacedAction"),
            "object property prefix outside export"
        );
        assert_eq!(
            err("NoSuchMover", "PlacedAction"),
            "unsupported exported object request"
        );
        // An actor that is not exactly an InterpActor.
        assert_eq!(
            err("PlainActor", "PlacedAction"),
            "mover binding has unsupported classes"
        );
    });
    let dir = cooked(&Opts {
        finished_param: "Action",
        ..Opts::default()
    });
    on_stack(|| {
        let e = bind(&dir, "PlacedMover", "PlacedAction").unwrap_err();
        assert_eq!(e.cpp_text(), "unsupported mover notification signature");
    });
    let dir = cooked(&Opts {
        reverse_prop: false,
        ..Opts::default()
    });
    on_stack(|| {
        let e = bind(&dir, "PlacedMover", "PlacedAction").unwrap_err();
        assert_eq!(e.cpp_text(), "missing mover lifecycle properties");
    });
}

#[test]
fn set_timer_refuses_a_rate_past_an_hour_and_a_name_with_no_method() {
    let dir = cooked(&Opts::default());
    on_stack(|| {
        let mut m = mover(&dir, "SlowMover");
        let finish = m.notify(true, false);
        assert_eq!(text(&finish), "unsupported mover timer");
        assert!(m.timers().is_empty());
    });
    let dir = cooked(&Opts {
        timer: "NoSuchCallback",
        ..Opts::default()
    });
    on_stack(|| {
        let mut m = mover(&dir, "PlacedMover");
        assert_eq!(text(&m.notify(true, false)), "unsupported mover timer");
    });
}

#[test]
fn a_looping_timer_stops_at_the_callback_limit_and_is_rolled_back() {
    let dir = cooked(&Opts {
        looping: true,
        ..Opts::default()
    });
    on_stack(|| {
        let mut m = mover(&dir, "FastMover");
        assert_eq!(m.notify(true, false).error, None);
        let before = m.timers();
        // 0.1 s apart, 10 s: about a hundred callbacks are due, more than 32.
        let r = m.advance(10.0);
        assert_eq!(text(&r), "mover timer callback limit exceeded");
        assert!(r.steps > 0, "the last callback's steps");
        assert_eq!(m.timers(), before);
        assert_eq!(m.clock(), 0.0);
        // A step that reaches three callbacks runs them, re-armed each time.
        let r = m.advance(0.3);
        assert_eq!(r.error, None);
        assert_eq!(m.timers().len(), 1);
    });
}

#[test]
fn advance_refuses_a_bad_delta() {
    let dir = cooked(&Opts::default());
    on_stack(|| {
        let mut m = mover(&dir, "PlacedMover");
        for bad in [-1.0, 60.5, f64::NAN, f64::INFINITY] {
            let r = m.advance(bad);
            assert_eq!(text(&r), "invalid mover timer delta");
            assert_eq!(r.steps, 0);
        }
        assert_eq!(m.advance(60.0).error, None);
        assert_eq!(m.clock(), 60.0);
    });
}

#[test]
fn the_natives_refuse_targets_outside_the_binding() {
    let dir = cooked(&Opts::default());
    on_stack(|| {
        let mut m = mover(&dir, "QuietMover");
        let action = m.action;
        let set = m.vm.find_function("Engine.Actor.SetTimer").unwrap();
        let clear = m.vm.find_function("Engine.Actor.ClearTimer").unwrap();
        let stop = m.vm.find_function("Engine.AudioComponent.Stop").unwrap();
        let message = |e: VmError| MoverError::from(e).cpp_text();
        let e = m.vm.call(clear, Some(action), vec![]).unwrap_err();
        assert_eq!(message(e), "mover ClearTimer target is outside binding");
        let e =
            m.vm.call(set, Some(action), vec![Value::Float(1.0)])
                .unwrap_err();
        assert_eq!(message(e), "mover SetTimer target is outside binding");
        let actor = m.actor;
        let e =
            m.vm.call(
                set,
                Some(actor),
                vec![
                    Value::Float(1.0),
                    Value::Bool(false),
                    Value::None,
                    Value::Object(Some(action)),
                ],
            )
            .unwrap_err();
        assert_eq!(message(e), "mover SetTimer target is outside binding");
        let e = m.vm.call(stop, Some(actor), vec![]).unwrap_err();
        assert_eq!(message(e), "mover audio target outside binding");
        // The ambient component is the loaded export; stopping it while idle is valid.
        let class = m.vm.space.heap.get(actor).unwrap().class;
        let p = m.vm.space.prop(class, "AmbientSoundComponent").unwrap();
        let Value::Object(Some(audio)) =
            m.vm.space.read(actor, PropAddr::scalar(p)).unwrap().clone()
        else {
            panic!("no ambient component");
        };
        assert!(m.vm.call(stop, Some(audio), vec![]).is_ok());
        // A timer with no name is 'timer', which the class has no method for.
        let e =
            m.vm.call(set, Some(actor), vec![Value::Float(1.0)])
                .unwrap_err();
        assert_eq!(message(e), "unsupported mover timer");
    });
    let dir = cooked(&Opts::default());
    on_stack(|| {
        let mut m = mover(&dir, "LoudMover");
        let stop = m.vm.find_function("Engine.AudioComponent.Stop").unwrap();
        let actor = m.actor;
        let class = m.vm.space.heap.get(actor).unwrap().class;
        let p = m.vm.space.prop(class, "AmbientSoundComponent").unwrap();
        let Value::Object(Some(audio)) =
            m.vm.space.read(actor, PropAddr::scalar(p)).unwrap().clone()
        else {
            panic!("no ambient component");
        };
        let e = m.vm.call(stop, Some(audio), vec![]).unwrap_err();
        assert_eq!(
            MoverError::from(e).cpp_text(),
            "mover sound playback is unsupported"
        );
    });
}

#[test]
fn an_action_with_no_owning_sequence_is_the_cxx_dispatch_error() {
    let dir = cooked(&Opts::default());
    on_stack(|| {
        let mut m = mover(&dir, "PlacedMover");
        let d = m.remote_event("OpenDoor");
        assert_eq!(
            (d.matched, d.motion, d.errors.clone()),
            (
                0,
                0,
                vec!["mover action path has no owning sequence".to_string()]
            )
        );
        assert!(d.trace.is_empty() && d.host_boundary.is_empty() && d.requests.is_empty());
        for d in [
            m.motion_finished(true),
            m.mission_event("M", "E"),
            m.sequence_event("Op"),
            m.originator_event("O"),
            m.output("Op", "Out"),
            m.advance_sequence(-1.0),
        ] {
            assert_eq!(d.errors, ["mover action path has no owning sequence"]);
        }
        // `variables` does not catch, as the C++.
        let e = m.variables("Op", "Target").unwrap_err();
        assert_eq!(e.cpp_text(), "mover action path has no owning sequence");
    });
}

/// The sequence calls of a mover bound to `Seq.InnerAction`.
fn sequenced(dir: &Cooked) -> Mover {
    bind(dir, "PlacedMover", "Seq.InnerAction").unwrap()
}

#[test]
fn a_remote_event_requests_motion_and_lists_the_other_actions_at_the_host_boundary() {
    let dir = cooked(&Opts::default());
    on_stack(|| {
        let mut m = sequenced(&dir);
        let d = m.remote_event("OpenDoor");
        assert_eq!((d.matched, d.motion), (1, 1));
        assert!(d.errors.is_empty(), "{:?}", d.errors);
        assert_eq!(d.host_boundary, ["Engine.SeqAct_ToyToggle:Lamp <- Turn On"]);
        assert_eq!(d.requests.len(), 1);
        assert_eq!(
            (
                d.requests[0].class.as_str(),
                d.requests[0].op.as_str(),
                d.requests[0].input.as_str()
            ),
            ("Engine.SeqAct_ToyToggle", "Lamp", "Turn On")
        );
        assert_eq!(d.trace[0], "event OpenEvent");
        // The per-call state is cleared: Reverse is -1 and the lamp is not listed again.
        let d = m.remote_event("CloseDoor");
        assert_eq!((d.matched, d.motion), (1, -1));
        assert!(d.host_boundary.is_empty());
        let d = m.remote_event("Nobody");
        assert_eq!((d.matched, d.motion), (0, 0));
        // The lifecycle calls are unaffected by the sequence.
        assert_eq!(m.notify(false, false).error, None);
    });
}

#[test]
fn the_motion_end_fires_completed_or_reversed() {
    let dir = cooked(&Opts::default());
    on_stack(|| {
        let mut m = sequenced(&dir);
        let d = m.motion_finished(false);
        assert_eq!(d.matched, 1);
        assert_eq!(
            d.host_boundary,
            ["Engine.SeqAct_ToyToggle:Lamp <- Turn Off"]
        );
        let d = m.motion_finished(true);
        assert_eq!(d.matched, 1);
        assert!(d.host_boundary.is_empty() && d.errors.is_empty(), "{d:?}");
    });
}

#[test]
fn event_output_and_time_entry_points() {
    let dir = cooked(&Opts::default());
    on_stack(|| {
        let mut m = sequenced(&dir);
        let d = m.sequence_event("CloseEvent");
        assert_eq!(
            (d.matched, d.motion, d.entered.clone()),
            (1, -1, vec!["CloseEvent".to_string()])
        );
        let d = m.sequence_event("Lamp");
        assert_eq!(
            (d.matched, d.errors.clone()),
            (0, vec!["no sequence event named Lamp".to_string()])
        );
        let d = m.output("InnerAction", "Completed");
        assert_eq!(d.matched, 1);
        assert_eq!(
            d.host_boundary,
            ["Engine.SeqAct_ToyToggle:Lamp <- Turn Off"]
        );
        // An op with no such output does nothing; a name that is no op is an error line.
        let d = m.output("Lamp", "Out");
        assert_eq!((d.matched, d.errors.len()), (0, 0));
        let d = m.output("Nope", "Out");
        assert_eq!(d.errors, ["no sequence op named Nope"]);
        let d = m.advance_sequence(-1.0);
        assert_eq!(d.errors, ["invalid sequence time delta"]);
        assert!(m.advance_sequence(1.0).errors.is_empty());
        // No event has the mover as its originator.
        assert_eq!(m.originator_event("PlacedMover").matched, 0);
        assert_eq!(m.mission_event("M", "E").matched, 0);
    });
}

#[test]
fn the_action_s_variables_name_the_objects_they_hold() {
    let dir = cooked(&Opts::default());
    on_stack(|| {
        let mut m = sequenced(&dir);
        let v = m.variables("InnerAction", "Data").unwrap();
        assert_eq!(
            v,
            [Variable {
                name: "MoverVar".to_string(),
                object: "PlacedMover".to_string()
            }]
        );
        assert!(m.variables("Nope", "Data").unwrap().is_empty());
        assert!(m.variables("InnerAction", "Other").unwrap().is_empty());
    });
}

#[test]
fn the_next_due_timer_is_the_earliest_and_a_tie_goes_to_the_first_name() {
    let timer = |name: &str, due: f64| {
        (
            name.to_string(),
            Timer {
                due,
                looping: false,
                rate: 1.0,
                name: name.to_string(),
            },
        )
    };
    let timers: BTreeMap<String, Timer> = [
        timer("b", 1.0),
        timer("a", 1.0),
        timer("c", 0.5),
        timer("d", 3.0),
    ]
    .into_iter()
    .collect();
    assert_eq!(next_due(&timers, 2.0).map(|t| t.name.as_str()), Some("c"));
    let timers: BTreeMap<String, Timer> = [timer("b", 1.0), timer("a", 1.0)].into_iter().collect();
    assert_eq!(next_due(&timers, 1.0).map(|t| t.name.as_str()), Some("a"));
    assert!(next_due(&timers, 0.99).is_none());
}
