//! Tests of the death natives and the world a kill needs, on the invented packages of `crate::run::fixture`
//! (AGENTS.md rule 9).

use ow_core::ParityMode;
use ow_object::value::Value;

use super::*;
use crate::run::tests::{def, fresh, make};

fn native(
    vm: &mut Vm,
    path: &str,
    this: Option<ObjRef>,
    args: Vec<Value>,
) -> Result<Value, VmError> {
    let f = vm.find_function(path)?;
    Ok(vm.call(f, this, args)?.ret)
}

#[test]
fn deaths_give_the_world_a_game_and_a_tracker_once() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    let world = with_state(vm, |s| s.world).unwrap().unwrap();
    let game = object(vm, world, "Game").unwrap().unwrap();
    assert!(crate::hit::access::is_a(vm, game, "WillowGame.WillowGameInfo").unwrap());
    assert_eq!(get(vm, game, "Role").unwrap(), Some(Value::Byte(3)));
    let gri = object(vm, world, "GRI").unwrap().unwrap();
    assert!(object(vm, gri, "MissionTracker").unwrap().is_some());
    assert!(with_state(vm, |s| s.deaths).unwrap());
    enable_deaths(vm).unwrap();
    assert_eq!(
        object(vm, world, "Game").unwrap(),
        Some(game),
        "a second call changes nothing"
    );
}

#[test]
fn a_pawn_prepared_for_death_gets_a_consumer_handle_from_100_and_walking_physics() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    run.apply("spawn:ToyPawns/Pawn_Toy:4");
    run.apply("spawn:ToyPawns/Pawn_Doomed:4");
    let (one, two) = (run.enemy(0).unwrap().pawn, run.enemy(1).unwrap().pawn);
    let vm = &mut run.world().vm;
    for (pawn, pid) in [(one, 100), (two, 101)] {
        let handle = get(vm, pawn, "ConsumerHandle").unwrap().unwrap();
        assert_eq!(field(vm, &handle, "PID"), Some(Value::Int(pid)));
        assert_eq!(
            get(vm, pawn, "Physics").unwrap(),
            Some(Value::Byte(1)),
            "PHYS_Walking"
        );
    }
    let consumers = with_state(vm, |s| s.consumers.clone()).unwrap();
    assert_eq!(consumers.get(&101), Some(&two));
    let handle = native(
        vm,
        "WillowGame.WillowPawn.GetBehaviorConsumerHandle",
        Some(two),
        Vec::new(),
    );
    // The fixture does not declare GetBehaviorConsumerHandle: the call is refused, nothing is guessed.
    assert!(handle.is_err());
}

#[test]
fn a_pawn_with_a_body_class_gets_its_death_definition() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    let pawn = make(vm, "WillowGame.WillowArmoredPawn");
    let body = def(vm, "BodyClass_Toy");
    set(vm, pawn, "BodyClass", Value::Object(Some(body))).unwrap();
    prepare_for_death(vm, pawn).unwrap();
    // BodyClass_Toy names no DeathDef: MyDeathDef stays None.
    assert_eq!(object(vm, pawn, "MyDeathDef").unwrap(), None);
}

#[test]
fn the_expression_trees_combine_by_their_operator() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let (mut run, _dir) = fresh(mode);
        let vm = &mut run.world().vm;
        let walking = make(vm, "WillowGame.WillowAIPawn");
        set(vm, walking, "Physics", Value::Byte(1)).unwrap();
        let standing = make(vm, "WillowGame.WillowAIPawn");
        let and = def(vm, "Tree_And");
        let or = def(vm, "Tree_Or");
        let xor = def(vm, "Tree_Xor");
        let eval = |vm: &mut Vm, tree, pawn| {
            native(
                vm,
                "Engine.ExpressionTree.Evaluate",
                Some(tree),
                vec![Value::Object(Some(pawn))],
            )
        };
        assert_eq!(
            eval(vm, and, walking).unwrap(),
            Value::Bool(true),
            "walking AND not none"
        );
        assert_eq!(eval(vm, and, standing).unwrap(), Value::Bool(false));
        assert_eq!(
            eval(vm, or, standing).unwrap(),
            Value::Bool(true),
            "none OR walking"
        );
        assert_eq!(eval(vm, or, walking).unwrap(), Value::Bool(true));
        match mode {
            ParityMode::Off => assert!(matches!(eval(vm, xor, walking), Err(VmError::NotYet(_)))),
            ParityMode::Reference => {
                assert_eq!(eval(vm, xor, walking).unwrap(), Value::Bool(false));
                assert!(vm.log.iter().any(|l| l == "UNIMPLEMENTED CompoundExpressionEvaluator.Evaluate: operator ceop_xor (not implemented)"));
            }
        }
        let empty = make(vm, "Engine.ExpressionTree");
        assert_eq!(
            eval(vm, empty, walking).unwrap(),
            Value::Bool(true),
            "a tree with no root holds"
        );
    }
}

#[test]
fn an_output_link_is_appended_to_the_running_behaviors_list() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    let f = vm
        .find_function("GearboxFramework.BehaviorKernel.ActivateBehaviorOutputLink")
        .unwrap();
    // No running behavior: nothing kept (the next run starts empty).
    vm.call(f, None, vec![Value::Object(None), Value::Int(4)])
        .unwrap();
    // The links go to the behavior kernel pack's shared collector, which this crate's pack registers.
    let ((), ids) = ow_behavior::kernel::natives::collect(vm, |vm| {
        for id in [2, 0, 2] {
            vm.call(f, None, vec![Value::Object(None), Value::Int(id)])
                .unwrap();
        }
    })
    .unwrap();
    assert_eq!(ids, vec![2, 0, 2], "call order, duplicates kept");
}

#[test]
fn the_tracker_native_logs_the_killed_stat_and_needs_no_objective() {
    let (text, failed) = crate::run::tests::run_steps(
        ParityMode::Off,
        &[
            "spawn:ToyPawns/Pawn_Doomed:4",
            "player:8",
            "hit:0:10::Bullet",
        ],
    );
    assert!(!failed, "{text}");
    assert!(
        text.contains(
            "NotifyPawnDied killed_stat= (no objective is registered with the VM tracker for it)"
        ),
        "{text}"
    );
    assert!(text.contains("\"stubs\":{}"), "{text}");
}
