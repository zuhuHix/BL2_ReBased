//! Tests of the level navigation join (lane IMPL-MAPLOAD-NAVLIST) on invented classes with the note's class and field
//! names, defined by hand (AGENTS.md rule 9). AI-assisted (Claude Code).

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_core::notyet::Missing;
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::*;

struct Fx {
    vm: Vm,
    wi: ObjRef,
}

fn fx_mode(parity: ParityMode) -> Fx {
    let mut s = ObjectSpace::new();
    let object = s.classes.define("Core.Object", None, vec![]).unwrap();
    let obj = PropKind::Object { class: object };
    let d = |s: &mut ObjectSpace, n: &str| PropertyDecl::new(s.names.intern(n), obj.clone());
    let actor = s.classes.define("Engine.Actor", Some(object), vec![]).unwrap();
    let decls = vec![d(&mut s, "nextNavigationPoint")];
    let nav = s.classes.define("Engine.NavigationPoint", Some(actor), decls).unwrap();
    let decls = vec![d(&mut s, "NextCoverLink")];
    s.classes.define("Engine.CoverLink", Some(nav), decls).unwrap();
    let decls = vec![d(&mut s, "NextPylon")];
    s.classes.define("Engine.Pylon", Some(nav), decls).unwrap();
    s.classes.define("Engine.PlayerStart", Some(nav), vec![]).unwrap();
    s.classes.define("Engine.Prop", Some(actor), vec![]).unwrap();
    let decls = vec![d(&mut s, "NavigationPointList"), d(&mut s, "CoverList"), d(&mut s, "PylonList")];
    let wi_class = s.classes.define("Engine.WorldInfo", Some(actor), decls).unwrap();
    let wi = s.alloc(wi_class, "WorldInfo_0", None, Origin::Runtime).unwrap();
    let vm = Vm::new(s, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), parity);
    Fx { vm, wi }
}

fn fx() -> Fx {
    fx_mode(ParityMode::Off)
}

impl Fx {
    fn obj(&mut self, class: &str, name: &str) -> ObjRef {
        let c = self.vm.space.classes.by_path(class).unwrap();
        self.vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
    }

    fn set(&mut self, o: ObjRef, field: &str, v: Option<ObjRef>) {
        let c = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(c, field).unwrap_or_else(|| panic!("no {field}"));
        self.vm.space.write(o, PropAddr::scalar(p), Value::Object(v)).unwrap();
    }

    fn get(&self, o: ObjRef, field: &str) -> Option<ObjRef> {
        let c = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(c, field).unwrap();
        match self.vm.space.read(o, PropAddr::scalar(p)).unwrap() {
            Value::Object(r) => *r,
            other => panic!("{other:?}"),
        }
    }

    /// Links `chain` along `kind`'s link, the last entry's link left as it is.
    fn chain(&mut self, kind: ListKind, chain: &[ObjRef]) {
        for w in chain.windows(2) {
            self.set(w[0], kind.link(), Some(w[1]));
        }
    }

    /// The world's list of `kind`, walked (at most 32 entries).
    fn world(&self, kind: ListKind) -> Vec<ObjRef> {
        let mut out = Vec::new();
        let mut cur = self.get(self.wi, kind.head());
        while let Some(o) = cur {
            out.push(o);
            assert!(out.len() <= 32, "a looping list");
            cur = self.get(o, kind.link());
        }
        out
    }

    fn join(&mut self, lists: &LevelLists) -> Result<JoinOutcome, VmError> {
        let wi = self.wi;
        join_level_lists(&mut self.vm, wi, lists)
    }

    /// Every list field of `objects` and the world's heads, to compare before and after a refusal.
    fn snapshot(&self, objects: &[ObjRef]) -> Vec<Option<ObjRef>> {
        let mut out = Vec::new();
        for kind in ListKind::ALL {
            out.push(self.get(self.wi, kind.head()));
            for &o in objects {
                let c = self.vm.space.heap.get(o).unwrap().class;
                if self.vm.space.prop(c, kind.link()).is_some() {
                    out.push(self.get(o, kind.link()));
                }
            }
        }
        out
    }
}

fn pair(start: ObjRef, end: ObjRef) -> ListPair {
    ListPair { start: Some(start), end: Some(end) }
}

/// The refusal is `NotYet { NotRead }` naming `what`.
fn refused(r: Result<JoinOutcome, VmError>, what: &str) {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            assert!(n.what.contains(what), "{n}");
            assert!(!n.what.starts_with("stopped after partial writes: "), "{n}");
        }
        other => panic!("expected NotRead {what:?}, got {other:?}"),
    }
}

/// A level whose navigation chain is two pylons that are also its pylon chain, and one cover link, joined onto a world
/// with one navigation point: each chain goes in front of the world's list, in its own order; one actor sits in two
/// kinds.
#[test]
fn a_level_joins_in_front_of_the_worlds_lists() {
    let mut x = fx();
    let old = x.obj("Engine.PlayerStart", "OldStart");
    let wi = x.wi;
    x.set(wi, "NavigationPointList", Some(old));
    let (p1, p2) = (x.obj("Engine.Pylon", "Pylon_1"), x.obj("Engine.Pylon", "Pylon_2"));
    let c1 = x.obj("Engine.CoverLink", "CoverLink_1");
    x.chain(ListKind::Navigation, &[p1, p2]);
    x.chain(ListKind::Pylon, &[p2, p1]);
    let lists = LevelLists { navigation: pair(p1, p2), cover: pair(c1, c1), pylon: pair(p2, p1) };
    let out = x.join(&lists).unwrap();
    assert_eq!(
        out,
        JoinOutcome { navigation: KindOutcome::Joined, cover: KindOutcome::Joined, pylon: KindOutcome::Joined }
    );
    assert_eq!(x.world(ListKind::Navigation), vec![p1, p2, old]);
    assert_eq!(x.world(ListKind::Cover), vec![c1]);
    assert_eq!(x.world(ListKind::Pylon), vec![p2, p1]);
    // A second level goes in front of the first.
    let s = x.obj("Engine.PlayerStart", "Start");
    let lists = LevelLists { navigation: pair(s, s), ..LevelLists::default() };
    let out = x.join(&lists).unwrap();
    assert_eq!(out.navigation, KindOutcome::Joined);
    assert_eq!((out.cover, out.pylon), (KindOutcome::Empty, KindOutcome::Empty));
    assert_eq!(x.world(ListKind::Navigation), vec![s, p1, p2, old]);
}

/// Null pairs are no-ops, each on its own; a level with none writes nothing.
#[test]
fn null_pairs_are_no_ops() {
    let mut x = fx();
    let old = x.obj("Engine.PlayerStart", "OldStart");
    let wi = x.wi;
    x.set(wi, "NavigationPointList", Some(old));
    let before = x.snapshot(&[old]);
    let out = x.join(&LevelLists::default()).unwrap();
    assert_eq!(out, JoinOutcome { navigation: KindOutcome::Empty, cover: KindOutcome::Empty, pylon: KindOutcome::Empty });
    assert_eq!(x.snapshot(&[old]), before);
}

/// A second join of the same level is a no-op (the port's answer), and a level only partly in the world's lists is
/// refused.
#[test]
fn a_second_join_is_a_no_op_and_a_partial_one_is_refused() {
    let mut x = fx();
    let (p1, p2) = (x.obj("Engine.Pylon", "Pylon_1"), x.obj("Engine.Pylon", "Pylon_2"));
    x.chain(ListKind::Navigation, &[p1, p2]);
    x.chain(ListKind::Pylon, &[p1, p2]);
    let lists = LevelLists { navigation: pair(p1, p2), pylon: pair(p1, p2), ..LevelLists::default() };
    x.join(&lists).unwrap();
    let before = x.snapshot(&[p1, p2]);
    let out = x.join(&lists).unwrap();
    assert_eq!((out.navigation, out.pylon), (KindOutcome::AlreadyJoined, KindOutcome::AlreadyJoined));
    assert_eq!(x.snapshot(&[p1, p2]), before, "nothing written");
    // The pylon list taken out again by hand: navigation is joined, pylon is not.
    let wi = x.wi;
    x.set(wi, "PylonList", None);
    let before = x.snapshot(&[p1, p2]);
    refused(x.join(&lists), PARTLY_JOINED);
    assert_eq!(x.snapshot(&[p1, p2]), before);
}

/// Each preflight refusal of one kind stops the whole join: a valid navigation chain is not spliced either.
#[test]
fn every_refusal_comes_before_any_splice() {
    let mut x = fx();
    let old = x.obj("Engine.PlayerStart", "OldStart");
    let wi = x.wi;
    x.set(wi, "NavigationPointList", Some(old));
    let (a, b) = (x.obj("Engine.PlayerStart", "A"), x.obj("Engine.PlayerStart", "B"));
    x.chain(ListKind::Navigation, &[a, b]);
    let nav = pair(a, b);
    let (c1, c2, c3) =
        (x.obj("Engine.CoverLink", "Cover_1"), x.obj("Engine.CoverLink", "Cover_2"), x.obj("Engine.CoverLink", "Cover_3"));
    let all = [old, a, b, c1, c2, c3];
    let case = |x: &mut Fx, cover: ListPair, links: &[(ObjRef, Option<ObjRef>)], what: &str| {
        for &(o, next) in links {
            x.set(o, "NextCoverLink", next);
        }
        let before = x.snapshot(&all);
        refused(x.join(&LevelLists { navigation: nav, cover, ..LevelLists::default() }), what);
        assert_eq!(x.snapshot(&all), before, "{what}: nothing spliced");
        assert_eq!(x.world(ListKind::Navigation), vec![old], "{what}");
    };
    // A half-null pair, either way round.
    case(&mut x, ListPair { start: Some(c1), end: None }, &[], HALF);
    case(&mut x, ListPair { start: None, end: Some(c1) }, &[], HALF);
    // The walk misses its end: c1 -> c2 -> None, the end c3.
    case(&mut x, pair(c1, c3), &[(c1, Some(c2)), (c2, None)], MISSES_END);
    // The end links on: c1 -> c2 -> c3, the end c2.
    case(&mut x, pair(c1, c2), &[(c1, Some(c2)), (c2, Some(c3)), (c3, None)], PAST_END);
    // A loop: c1 -> c2 -> c1, the end c3 never met.
    case(&mut x, pair(c1, c3), &[(c1, Some(c2)), (c2, Some(c1))], LOOP);
    // An entry not of the kind's class: a navigation point in a cover chain.
    case(&mut x, pair(c1, a), &[(c1, Some(a))], NOT_OF_KIND);
    // Through an entry of the world's cover list: the world holds c2, the level's chain c1 -> c2 -> c3.
    x.set(wi, "CoverList", Some(c2));
    case(&mut x, pair(c1, c3), &[(c1, Some(c2)), (c2, None)], SHARED);
    // A world list that loops.
    case(&mut x, pair(c1, c1), &[(c2, Some(c3)), (c3, Some(c2)), (c1, None)], WORLD_LOOP);
    // A world list holding an entry of another class.
    x.set(wi, "CoverList", Some(a));
    case(&mut x, pair(c1, c1), &[(c1, None)], NOT_OF_KIND);
}

/// The level's pairs are not decoded: reading them, and so `join_level`, is `NotRead` and writes nothing.
#[test]
fn the_level_pairs_are_not_read() {
    let mut x = fx();
    let level = x.obj("Engine.Prop", "PersistentLevel");
    match level_list_pairs(&x.vm, level) {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert!(n.what.contains(PAIRS), "{n}");
        }
        other => panic!("{other:?}"),
    }
    let wi = x.wi;
    refused(join_level(&mut x.vm, wi, level), PAIRS);
    assert_eq!(x.snapshot(&[]), vec![None, None, None]);
}

/// Under Reference the join is `NotPorted` (the C++ reference has none), and writes nothing.
#[test]
fn reference_has_no_join() {
    let mut x = fx_mode(ParityMode::Reference);
    let s = x.obj("Engine.PlayerStart", "Start");
    match x.join(&LevelLists { navigation: pair(s, s), ..LevelLists::default() }) {
        Err(VmError::NotYet(n)) => assert_eq!(n.missing, Missing::NotPorted, "{n}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(x.world(ListKind::Navigation), vec![]);
}

/// Malformed shapes are script errors: a receiver that is not a `WorldInfo`.
#[test]
fn the_receiver_must_be_a_world_info() {
    let mut x = fx();
    let prop = x.obj("Engine.Prop", "Prop");
    match join_level_lists(&mut x.vm, prop, &LevelLists::default()) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("is not a WorldInfo"), "{message}"),
        other => panic!("{other:?}"),
    }
}
