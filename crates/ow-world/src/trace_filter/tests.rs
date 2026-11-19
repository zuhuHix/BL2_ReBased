//! Tests of the trace filters (NATIVE_SPAWN_PLACEMENT.md section 7) on invented classes, objects and hits (AGENTS.md
//! rule 9). The class paths and flag names are the note's; the flag values set on each object are invented.

use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;

use super::*;

#[test]
fn the_flag_sets_are_the_notes() {
    assert_eq!(BOX_TEST, MOVERS | LEVEL_BSP | BLOCKING_ONLY | WORLD_GEOMETRY | STOP_AT_FIRST | TERRAIN);
    assert_eq!(SWEEP, MOVERS | LEVEL_BSP | WORLD_GEOMETRY | TERRAIN);
    assert_eq!(SWEEP_HASH_MASK, PAWNS | MOVERS | VOLUMES | OTHER_ACTORS | WORLD_GEOMETRY | TERRAIN);
    let simple = Query::BoxTest { zero_extent: false, complex: false };
    assert_eq!(simple.flags(), 8902);
    assert_eq!(Query::BoxTest { zero_extent: false, complex: true }.flags(), 8902 + 131072);
    assert_eq!(Query::BoxSweep.flags(), 8326 + 1024);
    assert!(simple.walks_actors() && Query::BoxSweep.walks_actors() && Query::PointSweep.walks_actors());
}

#[test]
fn each_query_needs_its_blocking_flag() {
    assert_eq!(Query::BoxTest { zero_extent: false, complex: false }.blocking_flag(), "BlockNonZeroExtent");
    assert_eq!(Query::BoxSweep.blocking_flag(), "BlockNonZeroExtent");
    assert_eq!(Query::BoxTest { zero_extent: true, complex: true }.blocking_flag(), "BlockZeroExtent");
    assert_eq!(Query::PointSweep.blocking_flag(), "BlockZeroExtent");
}

/// The note's table: (row, owner flags, tested by box tests, tested by sweeps).
#[test]
fn the_spawn_queries_test_the_notes_rows() {
    let o = |f: fn(&mut Owner)| {
        let mut o = Owner::default();
        f(&mut o);
        o
    };
    let world = o(|o| o.world_geometry = true);
    let cases = [
        ("Actor rule with bWorldGeometry", Row::Actor, world, true, true),
        ("Actor rule without", Row::Actor, Owner::default(), false, false),
        ("Volume with bPawnsOnly", Row::Volume, o(|o| (o.pawns_only, o.world_geometry) = (true, true)), false, false),
        ("Volume with bWorldGeometry", Row::Volume, world, true, true),
        ("Volume without either", Row::Volume, Owner::default(), false, false),
        ("mover", Row::Mover, Owner::default(), false, true),
        ("mover with bWorldGeometry (no shortcut)", Row::Mover, world, false, true),
        ("pawn", Row::Pawn, Owner::default(), false, false),
        ("stationary pawn", Row::Pawn, o(|o| o.stationary = true), false, false),
    ];
    for (what, row, owner, by_box, by_sweep) in cases {
        assert_eq!(owner_tested(row, &owner, false, BOX_TEST), by_box, "{what}: box test");
        assert_eq!(owner_tested(row, &owner, false, SWEEP | ONE_RESULT), by_sweep, "{what}: sweep");
    }
}

#[test]
fn the_class_rules_outside_the_spawns_flag_sets() {
    // Bit 1 tests a pawn unless it ignores all traces; bit 16 tests a stationary one; bit 64 needs a source actor.
    let pawn = Owner::default();
    assert!(owner_tested(Row::Pawn, &pawn, false, PAWNS));
    assert!(!owner_tested(Row::Pawn, &Owner { ignore_all_traces: true, ..pawn }, false, PAWNS));
    assert!(owner_tested(Row::Pawn, &Owner { stationary: true, ..pawn }, false, OTHER_ACTORS));
    assert!(!owner_tested(Row::Pawn, &pawn, false, PAWNS | BLOCKING_ONLY));
    // Bit 32 accepts a projectile target, or a blocking actor through a blocking primitive, before bit 64.
    let other = OTHER_ACTORS | PROJ_TARGETS | BLOCKING_ONLY;
    assert!(owner_tested(Row::Actor, &Owner { proj_target: true, ..Owner::default() }, false, other));
    assert!(owner_tested(Row::Actor, &Owner { block_actors: true, ..Owner::default() }, true, other));
    assert!(!owner_tested(Row::Actor, &Owner { block_actors: true, ..Owner::default() }, false, other));
    // A volume uses bit 8 in place of 16.
    assert!(owner_tested(Row::Volume, &Owner::default(), false, VOLUMES));
    assert!(!owner_tested(Row::Volume, &Owner::default(), false, OTHER_ACTORS));
}

struct World {
    space: ObjectSpace,
    primitive: ClassId,
    next: u32,
}

fn bools(space: &mut ObjectSpace, names: &[&str]) -> Vec<PropertyDecl> {
    names.iter().map(|n| PropertyDecl::new(space.names.intern(n), PropKind::Bool)).collect()
}

fn world() -> World {
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let d = bools(&mut space, &["bWorldGeometry", "bProjTarget", "bBlockActors"]);
    let actor = space.classes.define("Engine.Actor", Some(object), d).unwrap();
    let d = bools(&mut space, &["bIgnoreAllTraces", "bStationary"]);
    let pawn = space.classes.define("Engine.Pawn", Some(actor), d).unwrap();
    space.classes.define("Test.UnreadPawn", Some(pawn), vec![]).unwrap();
    let d = bools(&mut space, &["bPawnsOnly"]);
    space.classes.define("Engine.Volume", Some(actor), d).unwrap();
    let dynamic = space.classes.define("Engine.DynamicSMActor", Some(actor), vec![]).unwrap();
    space.classes.define("Engine.InterpActor", Some(dynamic), vec![]).unwrap();
    space.classes.define("Engine.KActor", Some(dynamic), vec![]).unwrap();
    space.classes.define("Engine.StaticMeshActor", Some(actor), vec![]).unwrap();
    let d = bools(&mut space, &["BlockNonZeroExtent", "BlockZeroExtent", "BlockActors"]);
    let primitive = space.classes.define("Engine.PrimitiveComponent", Some(object), d).unwrap();
    World { space, primitive, next: 0 }
}

impl World {
    fn obj(&mut self, class: ClassId, flags: &[&str]) -> ObjRef {
        self.next += 1;
        let o = self.space.alloc(class, &format!("O{}", self.next), None, Origin::Runtime).unwrap();
        for f in flags {
            let p = self.space.prop(class, f).unwrap();
            self.space.write(o, PropAddr::scalar(p), Value::Bool(true)).unwrap();
        }
        o
    }

    fn actor(&mut self, path: &str, flags: &[&str]) -> ObjRef {
        let class = self.space.classes.by_path(path).unwrap();
        self.obj(class, flags)
    }
}

#[test]
fn an_unread_class_takes_its_nearest_read_ancestors_row() {
    let w = world();
    let row = |p: &str| row_of(&w.space, w.space.classes.by_path(p).unwrap());
    assert_eq!(row("Test.UnreadPawn"), Row::Pawn);
    assert_eq!(row("Engine.KActor"), Row::Mover);
    assert_eq!(row("Engine.DynamicSMActor"), Row::Actor);
    assert_eq!(row("Engine.StaticMeshActor"), Row::Actor);
    assert_eq!(row("Engine.Volume"), Row::Volume);
}

#[test]
fn a_primitive_is_tested_by_its_blocking_flag_and_its_owners_row() {
    let mut w = world();
    let prim = w.primitive;
    let non_zero = w.obj(prim, &["BlockNonZeroExtent"]);
    let zero = w.obj(prim, &["BlockZeroExtent"]);
    let geometry = w.actor("Engine.StaticMeshActor", &["bWorldGeometry"]);
    let mover = w.actor("Engine.InterpActor", &[]);
    let pawn = w.actor("Test.UnreadPawn", &["bStationary"]);
    let box_test = Query::BoxTest { zero_extent: false, complex: false };
    let zero_test = Query::BoxTest { zero_extent: true, complex: false };
    let t = |w: &World, p, o, q| tested(&w.space, p, o, q).unwrap();
    assert_eq!(t(&w, non_zero, geometry, box_test), Candidate::Tested);
    assert_eq!(t(&w, non_zero, geometry, Query::BoxSweep), Candidate::Tested);
    // The wrong blocking flag for the query's kind.
    assert_eq!(t(&w, non_zero, geometry, zero_test), Candidate::NotTested);
    assert_eq!(t(&w, non_zero, geometry, Query::PointSweep), Candidate::NotTested);
    assert_eq!(t(&w, zero, geometry, Query::PointSweep), Candidate::Tested);
    assert_eq!(t(&w, zero, geometry, zero_test), Candidate::Tested);
    // Movers: sweeps only. Pawns: never.
    assert_eq!(t(&w, non_zero, mover, box_test), Candidate::NotTested);
    assert_eq!(t(&w, non_zero, mover, Query::BoxSweep), Candidate::Tested);
    assert_eq!(t(&w, non_zero, pawn, box_test), Candidate::NotTested);
    assert_eq!(t(&w, non_zero, pawn, Query::BoxSweep), Candidate::NotTested);
}

fn hit(x: f32, n: f32, fraction: f32) -> Hit {
    Hit { location: [x, 0.0, 0.0], normal: [n, 0.0, 1.0], fraction }
}

#[test]
fn a_sweep_returns_the_smaller_fraction_and_walks_actors_to_five_units_past_the_bsp() {
    // A 100-unit sweep: the BSP at 0.5; actors are walked up to 0.55.
    let bsp = hit(50.0, 0.0, 0.5);
    assert_eq!(choose_sweep_hit(Some(bsp), &[hit(40.0, 1.0, 0.4)], 100.0), Ok(Some(hit(40.0, 1.0, 0.4))));
    assert_eq!(choose_sweep_hit(Some(bsp), &[hit(60.0, 1.0, 0.6)], 100.0), Ok(Some(bsp)));
    assert_eq!(choose_sweep_hit(Some(bsp), &[hit(52.0, 1.0, 0.52)], 100.0), Ok(Some(bsp)));
    // Without a BSP hit, the whole sweep is walked and the smallest actor fraction kept.
    let actors = [hit(80.0, 1.0, 0.8), hit(30.0, 1.0, 0.3)];
    assert_eq!(choose_sweep_hit(None, &actors, 100.0), Ok(Some(hit(30.0, 1.0, 0.3))));
    assert_eq!(choose_sweep_hit(None, &[], 100.0), Ok(None));
}

#[test]
fn an_exact_tie_answers_only_when_the_tied_hits_agree() {
    let bsp = hit(50.0, 0.0, 0.5);
    assert_eq!(choose_sweep_hit(Some(bsp), &[bsp], 100.0), Ok(Some(bsp)));
    assert_eq!(choose_sweep_hit(Some(bsp), &[hit(50.0, 1.0, 0.5)], 100.0), Err(UnreadTieOrder));
    assert_eq!(choose_sweep_hit(None, &[hit(20.0, 1.0, 0.2), hit(21.0, 1.0, 0.2)], 100.0), Err(UnreadTieOrder));
    // A tie above a smaller hit does not matter.
    let actors = [hit(20.0, 1.0, 0.2), hit(21.0, 1.0, 0.2), hit(10.0, 1.0, 0.1)];
    assert_eq!(choose_sweep_hit(None, &actors, 100.0), Ok(Some(hit(10.0, 1.0, 0.1))));
}
