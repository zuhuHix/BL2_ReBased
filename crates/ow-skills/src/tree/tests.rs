//! Tests of the tree rules and natives on an invented tree (AGENTS.md rule 9): a root branch holding the action skill
//! (one tier of 1 point) with two children, `A` (three tiers of 5, 5 and 1 points, a layout) and `B` (one tier of four
//! skills, no layout).

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_core::notyet::Missing;
use ow_defs::DefStore;
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, Vm};

use super::oracle::*;
use super::*;
use crate::skills::fixture::Fx;

struct T {
    fx: Fx,
    tree: ObjRef,
    action: ObjRef,
    a: [ObjRef; 4],
    b: [ObjRef; 4],
    branch_a: ObjRef,
    branch_b: ObjRef,
}

fn build() -> T {
    let mut fx = Fx::new();
    let ts = fx.tree_classes();
    let action = fx.skill("Pkg.Action", "Act", "SKILL_TYPE_Action", 1);
    fx.set(action, "bSubjectToGradeRules", Value::Bool(true));
    let a = [0, 1, 2, 3].map(|i| fx.skill(&format!("Pkg.A{i}"), "a", "SKILL_TYPE_Passive", 5));
    let b = [0, 1, 2, 3].map(|i| fx.skill(&format!("Pkg.B{i}"), "b", "SKILL_TYPE_Kill", 5));
    let tree = fx.obj("WillowGame.SkillTreeDefinition", "Pkg.Tree");
    let root = fx.obj("WillowGame.SkillTreeBranchDefinition", "Pkg.Root");
    let branch_a = fx.obj("WillowGame.SkillTreeBranchDefinition", "Pkg.BranchA");
    let branch_b = fx.obj("WillowGame.SkillTreeBranchDefinition", "Pkg.BranchB");
    let layout = fx.obj("WillowGame.SkillTreeBranchLayoutDefinition", "Pkg.LayoutA");
    fx.set(tree, "Root", Value::Object(Some(root)));
    let tiers = fx.array(vec![fx.tier(&ts, &[action], 1)]);
    fx.set(root, "Tiers", tiers);
    let children = fx.objects(&[branch_a, branch_b]);
    fx.set(root, "Children", children);
    let tiers = fx.array(vec![
        fx.tier(&ts, &[a[0], a[1]], 5),
        fx.tier(&ts, &[a[2]], 5),
        fx.tier(&ts, &[a[3]], 1),
    ]);
    fx.set(branch_a, "Tiers", tiers);
    fx.set(branch_a, "Layout", Value::Object(Some(layout)));
    let rows = fx.array(vec![
        fx.row(&ts, [true, false, true]),
        fx.row(&ts, [false, true, false]),
        fx.row(&ts, [false, true, false]),
    ]);
    fx.set(layout, "Tiers", rows);
    let tiers = fx.array(vec![fx.tier(&ts, &b, 5)]);
    fx.set(branch_b, "Tiers", tiers);
    T {
        fx,
        tree,
        action,
        a,
        b,
        branch_a,
        branch_b,
    }
}

fn initialized(t: &mut T) -> SkillTree {
    let mut cx = t.fx.cx();
    let def = read_tree(&mut cx, t.tree).unwrap();
    SkillTree::initialize(&mut cx, def, Some(&[])).unwrap()
}

#[test]
fn the_tree_is_read_with_cumulative_requirements_and_cells() {
    let mut t = build();
    let mut cx = t.fx.cx();
    let def = read_tree(&mut cx, t.tree).unwrap();
    assert_eq!(def.branches.len(), 3);
    assert_eq!(def.branches[0].children, vec![1, 2]);
    let a = &def.branches[1];
    assert_eq!(a.parent, Some(0));
    assert_eq!(
        a.tiers.iter().map(|x| x.cumulative).collect::<Vec<_>>(),
        vec![0, 5, 10]
    );
    assert_eq!(a.unlock_total, 11);
    assert_eq!(
        a.tiers.iter().map(|x| x.cells).collect::<Vec<_>>(),
        vec![2, 1, 1],
        "occupied cells of the layout"
    );
    assert_eq!(
        def.branches[2].tiers[0].cells, DEFAULT_CELLS,
        "no layout: 3 cells"
    );
}

#[test]
fn initialize_unlocks_only_the_roots_first_tier_and_records_the_action_skill() {
    let mut t = build();
    let tree = initialized(&mut t);
    assert_eq!(
        tree.unlocked,
        vec![vec![true], vec![false, false, false], vec![false]]
    );
    assert_eq!(tree.action_skill, Some(t.action));
    assert_eq!(tree.points_spent().unwrap(), 0);
}

#[test]
fn spending_the_action_point_opens_every_child_and_tiers_open_by_cumulative_points() {
    let mut t = build();
    let mut tree = initialized(&mut t);
    assert!(
        !tree.upgrade(&mut t.fx.cx(), t.a[0]).unwrap(),
        "a locked tier refuses"
    );
    assert!(tree.upgrade(&mut t.fx.cx(), t.action).unwrap());
    assert!(tree.unlocked[1][0]);
    assert!(tree.unlocked[2][0]);
    assert!(!tree.upgrade(&mut t.fx.cx(), t.action).unwrap(), "maxed");
    for _ in 0..4 {
        assert!(tree.upgrade(&mut t.fx.cx(), t.a[0]).unwrap());
    }
    assert!(!tree.unlocked[1][1], "4 points: tier 2 needs 5");
    assert!(tree.upgrade(&mut t.fx.cx(), t.a[1]).unwrap());
    assert!(tree.unlocked[1][1], "5 points open the second tier");
    assert!(
        !tree.upgrade(&mut t.fx.cx(), t.a[3]).unwrap(),
        "the third tier stays locked"
    );
}

#[test]
fn at_most_one_tier_opens_per_grade_change() {
    let mut t = build();
    let mut tree = initialized(&mut t);
    tree.upgrade(&mut t.fx.cx(), t.action).unwrap();
    // One grade change of 10 points makes both higher tiers due; one opens per call.
    let i = tree
        .skills
        .iter()
        .position(|s| s.definition == t.a[0])
        .unwrap();
    tree.skills[i].max_grade = 10;
    assert!(tree.set_grade(&mut t.fx.cx(), t.a[0], 10).unwrap());
    assert_eq!(tree.unlocked[1], vec![true, true, false]);
    assert!(tree.set_grade(&mut t.fx.cx(), t.a[1], 1).unwrap());
    assert_eq!(
        tree.unlocked[1],
        vec![true, true, true],
        "the next call opens the next due tier"
    );
    assert!(tree.set_grade(&mut t.fx.cx(), t.a[0], 0).unwrap());
    assert_eq!(
        tree.unlocked[1],
        vec![true, true, true],
        "a decrease never re-locks"
    );
}

#[test]
fn set_grade_clamps_skips_the_tier_check_and_answers_whether_it_changed() {
    let mut t = build();
    let mut tree = initialized(&mut t);
    assert!(
        tree.set_grade(&mut t.fx.cx(), t.a[3], 99).unwrap(),
        "no tier check on the load path"
    );
    assert_eq!(tree.grade(t.a[3]), Some(5));
    assert!(
        !tree.set_grade(&mut t.fx.cx(), t.a[3], 7).unwrap(),
        "unchanged after the clamp"
    );
    assert!(tree.set_grade(&mut t.fx.cx(), t.a[3], -3).unwrap());
    assert_eq!(tree.grade(t.a[3]), Some(0));
    let other = t.fx.skill("Pkg.Other", "o", "SKILL_TYPE_Passive", 1);
    assert!(
        !tree.set_grade(&mut t.fx.cx(), other, 1).unwrap(),
        "not in this tree"
    );
    assert!(!tree.upgrade(&mut t.fx.cx(), other).unwrap());
}

#[test]
fn reset_refunds_before_minus_after_and_restores_default_grades() {
    let mut t = build();
    t.fx.set(t.b[0], "DefaultStartingGrade", Value::Int(1));
    let mut tree = initialized(&mut t);
    assert_eq!(
        tree.grade(t.b[0]),
        Some(1),
        "initialize resets to DefaultStartingGrade"
    );
    tree.upgrade(&mut t.fx.cx(), t.action).unwrap();
    tree.upgrade(&mut t.fx.cx(), t.b[1]).unwrap();
    tree.upgrade(&mut t.fx.cx(), t.b[1]).unwrap();
    assert_eq!(tree.points_spent().unwrap(), 4);
    assert_eq!(
        tree.reset(false).unwrap(),
        3,
        "4 before, 1 after (the default grade)"
    );
    assert_eq!(
        tree.unlocked,
        vec![vec![true], vec![false, false, false], vec![false]]
    );
}

#[test]
fn tier_points_count_only_the_cells_and_branch_points_every_skill() {
    let mut t = build();
    let mut tree = initialized(&mut t);
    for (i, s) in t.b.iter().enumerate() {
        tree.set_grade(&mut t.fx.cx(), *s, i32::try_from(i).unwrap() + 1)
            .unwrap();
    }
    assert_eq!(
        tree.tier_state(t.branch_b, 0).unwrap().unwrap().points,
        1 + 2 + 3,
        "three cells without a layout"
    );
    let branch = tree.branch_state(t.branch_b).unwrap().unwrap();
    assert_eq!(branch.points, 1 + 2 + 3 + 4);
    assert_eq!(branch.max_points, 20);
    assert!(!branch.unlocked);
    assert_eq!(tree.tier_state(t.branch_a, 7).unwrap(), None);
    let state = tree.skill_state(t.b[3]).unwrap().unwrap();
    assert_eq!(
        (state.branch, state.tier, state.grade, state.unlocked),
        (2, 0, 4, false)
    );
}

#[test]
fn has_trained_a_skill_of_type_needs_a_grade_and_an_unlocked_tier() {
    let mut t = build();
    let mut tree = initialized(&mut t);
    tree.set_grade(&mut t.fx.cx(), t.b[0], 2).unwrap();
    assert!(
        !tree.unlocked[2][0],
        "the load path does not open a child's first tier"
    );
    assert!(
        !tree.has_trained_of_type(SkillType::Kill).unwrap(),
        "its tier is locked"
    );
    tree.upgrade(&mut t.fx.cx(), t.action).unwrap();
    assert!(tree.has_trained_of_type(SkillType::Kill).unwrap());
    assert!(tree.has_trained_of_type(SkillType::Action).unwrap());
    assert!(!tree.has_trained_of_type(SkillType::Passive).unwrap());
}

#[test]
fn a_proficiency_outside_every_tier_is_not_yet() {
    let mut t = build();
    let p = t.fx.skill("Pkg.Prof", "p", "SKILL_TYPE_Proficiency", 5);
    let mut cx = t.fx.cx();
    let def = read_tree(&mut cx, t.tree).unwrap();
    let mut tree = SkillTree::initialize(&mut cx, def, Some(&[p])).unwrap();
    assert!(matches!(
        tree.upgrade(&mut t.fx.cx(), p),
        Err(SkillError::NotYet(_))
    ));
    assert!(matches!(tree.skill_state(p), Err(SkillError::NotYet(_))));
    assert!(
        tree.set_grade(&mut t.fx.cx(), p, 2).unwrap(),
        "the load path sets it"
    );
    assert_eq!(
        tree.points_spent().unwrap(),
        0,
        "proficiencies are not spent points"
    );
    assert_eq!(tree.reset(true).unwrap(), 0);
    assert_eq!(tree.grade(p), Some(2), "kept when ignoring proficiencies");
}

#[test]
fn the_ui_check_and_the_server_spend_in_their_order() {
    let mut t = build();
    let mut tree = initialized(&mut t);
    let other = t.fx.skill("Pkg.Other", "o", "SKILL_TYPE_Passive", 1);
    assert_eq!(
        can_upgrade(&tree, other, 10, 3).unwrap(),
        UpgradeFailure::DataIssue
    );
    assert_eq!(
        can_upgrade(&tree, t.action, 4, 3).unwrap(),
        UpgradeFailure::SkillLocked,
        "below level 5"
    );
    assert_eq!(
        can_upgrade(&tree, t.a[0], 10, 3).unwrap(),
        UpgradeFailure::SkillLocked,
        "a locked tier"
    );
    assert_eq!(
        can_upgrade(&tree, t.action, 5, 0).unwrap(),
        UpgradeFailure::NoSkillPoints
    );
    assert_eq!(
        can_upgrade(&tree, t.action, 5, 1).unwrap(),
        UpgradeFailure::NoFailure
    );
    let mut points = 1;
    assert_eq!(
        server_upgrade(&mut t.fx.cx(), &mut tree, t.action, &mut points).unwrap(),
        Some(1)
    );
    assert_eq!(points, 0);
    assert_eq!(
        can_upgrade(&tree, t.action, 5, 1).unwrap(),
        UpgradeFailure::SkillMaxed
    );
    assert_eq!(
        server_upgrade(&mut t.fx.cx(), &mut tree, t.a[0], &mut points).unwrap(),
        None,
        "no point"
    );
    assert_eq!(tree.grade(t.a[0]), Some(0));
    let mut points = 2;
    assert_eq!(
        server_upgrade(&mut t.fx.cx(), &mut tree, t.a[3], &mut points).unwrap(),
        None,
        "a locked tier costs nothing"
    );
    assert_eq!(points, 2);
    assert_eq!(upgrade_cost(SkillType::Proficiency), 0);
    assert_eq!(upgrade_cost(SkillType::Kill), 1);
}

#[test]
fn a_branch_reached_twice_is_refused() {
    let mut t = build();
    let root = t.fx.at("Pkg.Root");
    let children = t.fx.objects(&[t.branch_a, root]);
    t.fx.set(t.branch_a, "Children", children);
    let mut cx = t.fx.cx();
    assert!(matches!(
        read_tree(&mut cx, t.tree),
        Err(SkillError::BadData { .. })
    ));
}

// ---------------------------------------------------------------------------------------------------------------
// The natives
// ---------------------------------------------------------------------------------------------------------------

fn declare(
    vm: &mut Vm,
    path: &str,
    params: Vec<PropKind>,
    result: Option<PropKind>,
) -> ow_object::ids::FuncId {
    let class = vm.find_class("WillowGame.PlayerSkillTree").unwrap();
    let leaf = path.rsplit('.').next().unwrap();
    let key = format!("PlayerSkillTree.{leaf}(invented)");
    let params = params
        .into_iter()
        .enumerate()
        .map(|(i, k)| PropertyDecl::new(vm.space.names.intern(&format!("P{i}")), k))
        .collect();
    let mut decl = FunctionDecl::native(ow_core::ObjectPath::parse(path).unwrap(), key)
        .with_owner(class)
        .with_params(params);
    if let Some(r) = result {
        decl = decl.with_result(PropertyDecl::new(vm.space.names.intern("ReturnValue"), r));
    }
    vm.declare_function(decl).unwrap()
}

#[test]
fn the_tree_natives_build_change_and_read_the_objects_tree() {
    let mut t = build();
    let space = std::mem::take(&mut t.fx.w.space);
    let mut vm = Vm::new(
        space,
        DefStore::new_empty(),
        Rc::new(AttributeSystem::standard().unwrap()),
        ParityMode::Off,
    );
    let object = vm.find_class("Core.Object").unwrap();
    let obj = PropKind::Object { class: object };
    let [k_init, k_up, k_set, k_spent, k_action, k_trained] = NATIVE_PATHS;
    let init = declare(&mut vm, k_init, vec![obj.clone()], None);
    let up = declare(&mut vm, k_up, vec![obj.clone()], Some(PropKind::Bool));
    let set = declare(
        &mut vm,
        k_set,
        vec![obj.clone(), PropKind::Int],
        Some(PropKind::Bool),
    );
    let spent = declare(&mut vm, k_spent, vec![], Some(PropKind::Int));
    let action = declare(&mut vm, k_action, vec![], Some(obj.clone()));
    let trained = declare(
        &mut vm,
        k_trained,
        vec![PropKind::Byte { enum_: None }],
        Some(PropKind::Bool),
    );
    Pack.register(&mut vm).unwrap();
    let class = vm.find_class("WillowGame.PlayerSkillTree").unwrap();
    let tree_obj = vm.instantiate(class, "PlayerSkillTree_0").unwrap();
    let e = vm
        .call(up, Some(tree_obj), vec![Value::Object(Some(t.action))])
        .unwrap_err();
    assert!(
        matches!(e, VmError::NotYet(ref n) if n.missing == Missing::NotRead),
        "{e:?}"
    );
    vm.call(init, Some(tree_obj), vec![Value::Object(Some(t.tree))])
        .unwrap();
    assert_eq!(
        vm.call(action, Some(tree_obj), vec![]).unwrap().ret,
        Value::Object(Some(t.action))
    );
    assert_eq!(
        vm.call(up, Some(tree_obj), vec![Value::Object(Some(t.a[0]))])
            .unwrap()
            .ret,
        Value::Bool(false)
    );
    assert_eq!(
        vm.call(up, Some(tree_obj), vec![Value::Object(Some(t.action))])
            .unwrap()
            .ret,
        Value::Bool(true)
    );
    assert_eq!(
        vm.call(
            set,
            Some(tree_obj),
            vec![Value::Object(Some(t.b[0])), Value::Int(3)]
        )
        .unwrap()
        .ret,
        Value::Bool(true)
    );
    assert_eq!(
        vm.call(spent, Some(tree_obj), vec![]).unwrap().ret,
        Value::Int(4)
    );
    assert_eq!(
        vm.call(trained, Some(tree_obj), vec![Value::Byte(3)])
            .unwrap()
            .ret,
        Value::Bool(true)
    );
    assert!(
        vm.call(trained, Some(tree_obj), vec![Value::Byte(9)])
            .is_err(),
        "outside ESkillType"
    );
    // A second Initialize leaves the tree as it is.
    vm.call(init, Some(tree_obj), vec![Value::Object(Some(t.tree))])
        .unwrap();
    assert_eq!(
        vm.call(spent, Some(tree_obj), vec![]).unwrap().ret,
        Value::Int(4)
    );
}

/// On a world's query service, the player-skill engine query answers `NotYet { NotPorted }` naming the contract gap
/// (a grade is not an object the resolver can read), not the "layer absent" of a world without this pack, and never a
/// guessed grade.
#[test]
fn the_player_skill_query_names_the_contract_gap() {
    use ow_attr::api::{AttrError, AttrWorld, EngineQuery};
    let mut t = build();
    let space = std::mem::take(&mut t.fx.w.space);
    let mut vm = Vm::new(
        space,
        DefStore::new_empty(),
        Rc::new(AttributeSystem::standard().unwrap()),
        ParityMode::Off,
    );
    ow_world::query::install(&mut vm).unwrap();
    let q = EngineQuery::PlayerSkill {
        definition: t.action,
    };
    let before = vm.engine_query(q, t.action).unwrap_err();
    assert!(
        matches!(before, AttrError::NotYet(ref n) if matches!(n.missing, Missing::LayerAbsent(_))),
        "{before:?}"
    );
    Pack.register(&mut vm).unwrap();
    let e = vm.engine_query(q, t.action).unwrap_err();
    assert!(
        matches!(e, AttrError::NotYet(ref n) if n.missing == Missing::NotPorted),
        "{e:?}"
    );
    Pack.register(&mut vm).unwrap();
}

/// Review r1 of PR #342, item 2: a tree built without the globals' proficiencies (the native's case) answers `NotYet`
/// for the Proficiency type and for an unknown proficiency, so a saved grade is not dropped silently; an unknown skill
/// of another type is still "not in this tree".
#[test]
fn a_tree_without_its_proficiencies_is_not_yet_for_them() {
    let mut t = build();
    let p = t.fx.skill("Pkg.Prof", "p", "SKILL_TYPE_Proficiency", 5);
    let other = t.fx.skill("Pkg.Other", "o", "SKILL_TYPE_Passive", 1);
    let mut cx = t.fx.cx();
    let def = read_tree(&mut cx, t.tree).unwrap();
    let mut tree = SkillTree::initialize(&mut cx, def, None).unwrap();
    let not_read = |r: Result<bool, SkillError>| matches!(r, Err(SkillError::NotYet(ref n)) if n.missing == Missing::NotRead);
    assert!(not_read(tree.has_trained_of_type(SkillType::Proficiency)));
    assert!(not_read(tree.set_grade(&mut cx, p, 3)));
    assert!(not_read(tree.upgrade(&mut cx, p)));
    assert!(!tree.set_grade(&mut cx, other, 1).unwrap());
    assert!(!tree.upgrade(&mut cx, other).unwrap());
    assert!(!tree.has_trained_of_type(SkillType::Kill).unwrap());
}

/// Review r1 of PR #342, item 8: a `DefaultStartingGrade` outside `0..=MaxGrade` is refused (no clamp), and an
/// unknown branch is bad data, not a panic.
#[test]
fn a_bad_default_grade_and_an_unknown_branch_are_refused() {
    let mut t = build();
    t.fx.set(t.b[0], "DefaultStartingGrade", Value::Int(9));
    let mut cx = t.fx.cx();
    let def = read_tree(&mut cx, t.tree).unwrap();
    assert!(matches!(
        SkillTree::initialize(&mut cx, def, Some(&[])),
        Err(SkillError::BadData { .. })
    ));
    let mut t = build();
    let mut tree = initialized(&mut t);
    assert!(matches!(
        tree.run_unlock(99),
        Err(SkillError::BadData { .. })
    ));
    assert_eq!(tree.unlocked(99, 0), None);
    assert_eq!(tree.unlocked(0, 0), Some(true));
}
