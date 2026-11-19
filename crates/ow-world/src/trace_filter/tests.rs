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
    space.classes.define("Engine.Terrain", Some(actor), vec![]).unwrap();
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
fn only_the_read_classes_have_a_row() {
    let w = world();
    let row = |p: &str| row_of(&w.space, w.space.classes.by_path(p).unwrap());
    assert_eq!(row("Engine.Actor"), Some(Row::Actor));
    assert_eq!(row("Engine.StaticMeshActor"), Some(Row::Actor));
    assert_eq!(row("Engine.Terrain"), Some(Row::Actor));
    assert_eq!(row("Engine.Volume"), Some(Row::Volume));
    assert_eq!(row("Engine.KActor"), Some(Row::Mover));
    assert_eq!(row("Engine.InterpActor"), Some(Row::Mover));
    assert_eq!(row("Engine.Pawn"), Some(Row::Pawn));
    // No row is inherited: a subclass of a read class may override the filter.
    assert_eq!(row("Test.UnreadPawn"), None);
    assert_eq!(row("Engine.DynamicSMActor"), None);
}

#[test]
fn a_blocking_primitive_is_always_unsupported_and_names_its_most_specific_rule() {
    let mut w = world();
    let prim = w.primitive;
    let non_zero = w.obj(prim, &["BlockNonZeroExtent"]);
    let zero = w.obj(prim, &["BlockZeroExtent"]);
    let geometry = w.actor("Engine.StaticMeshActor", &["bWorldGeometry"]);
    let plain = w.actor("Engine.StaticMeshActor", &[]);
    let mover = w.actor("Engine.InterpActor", &[]);
    let pawn = w.actor("Engine.Pawn", &["bStationary"]);
    let box_test = Query::BoxTest { zero_extent: false, complex: false };
    let zero_test = Query::BoxTest { zero_extent: true, complex: false };
    let t = |w: &World, p, o, q| tested(&w.space, p, o, q).unwrap();
    let per = Candidate::Unsupported(Unread::PerComponentTest);
    let skip = Candidate::Unsupported(Unread::SkipFlag);
    assert_eq!(t(&w, non_zero, geometry, box_test), per);
    assert_eq!(t(&w, non_zero, geometry, Query::BoxSweep), per);
    // A blocking primitive the row would drop still meets the unresolved skip flag (review r1 of #477).
    assert_eq!(t(&w, non_zero, plain, box_test), skip);
    // The wrong blocking flag for the query's kind: no candidate, whatever the owner.
    assert_eq!(t(&w, non_zero, geometry, zero_test), Candidate::NotTested);
    assert_eq!(t(&w, non_zero, geometry, Query::PointSweep), Candidate::NotTested);
    assert_eq!(t(&w, zero, geometry, Query::PointSweep), per);
    assert_eq!(t(&w, zero, geometry, zero_test), per);
    // Movers: tested by sweeps only; a box test meets the skip flag. Pawns: never tested, so always the skip flag.
    assert_eq!(t(&w, non_zero, mover, box_test), skip);
    assert_eq!(t(&w, non_zero, mover, Query::BoxSweep), per);
    assert_eq!(t(&w, non_zero, pawn, box_test), skip);
    assert_eq!(t(&w, non_zero, pawn, Query::BoxSweep), skip);
    assert_eq!(Unread::SkipFlag.rule(), SKIP_FLAG);
}

#[test]
fn an_unread_owner_class_and_terrain_are_unsupported() {
    let mut w = world();
    let prim = w.primitive;
    let blocking = w.obj(prim, &["BlockNonZeroExtent"]);
    let idle = w.obj(prim, &[]);
    let unread = w.actor("Test.UnreadPawn", &[]);
    let terrain = w.actor("Engine.Terrain", &[]);
    let box_test = Query::BoxTest { zero_extent: false, complex: false };
    let t = |w: &World, p, o, q| tested(&w.space, p, o, q).unwrap();
    assert_eq!(
        t(&w, blocking, unread, box_test),
        Candidate::Unsupported(Unread::OwnerClass("Test.UnreadPawn".to_string()))
    );
    assert_eq!(Unread::OwnerClass("Test.UnreadPawn".into()).rule(), "the trace filter of Test.UnreadPawn");
    // A primitive that does not block the query's kind is no candidate, whatever its owner's class.
    assert_eq!(t(&w, idle, unread, box_test), Candidate::NotTested);
    // A terrain primitive in the region is Unsupported, blocking or not.
    assert_eq!(t(&w, idle, terrain, box_test), Candidate::Unsupported(Unread::TerrainHandling));
    assert_eq!(t(&w, blocking, terrain, Query::BoxSweep), Candidate::Unsupported(Unread::TerrainHandling));
}

fn hit(x: f32, n: f32, fraction: f32) -> Hit {
    Hit { location: [x, 0.0, 0.0], normal: [n, 0.0, 1.0], fraction }
}

#[test]
fn a_sweep_answers_the_bsp_alone_or_the_actors_alone() {
    // A 100-unit sweep with the BSP at 0.5: actors are walked up to 0.55.
    let bsp = hit(50.0, 0.0, 0.5);
    assert_eq!(choose_sweep_hit(Some(bsp), &[], 100.0), Ok(Some(bsp)));
    assert_eq!(choose_sweep_hit(Some(bsp), &[hit(60.0, 1.0, 0.6)], 100.0), Ok(Some(bsp)));
    // A BSP hit and an actor hit within reach: the comparator decides, unread.
    assert_eq!(choose_sweep_hit(Some(bsp), &[hit(40.0, 1.0, 0.4)], 100.0), Err(UnreadChoice::Comparator));
    assert_eq!(choose_sweep_hit(Some(bsp), &[hit(52.0, 1.0, 0.52)], 100.0), Err(UnreadChoice::Comparator));
    // Without a BSP hit, the whole sweep is walked and the smallest actor fraction kept.
    let actors = [hit(80.0, 1.0, 0.8), hit(30.0, 1.0, 0.3)];
    assert_eq!(choose_sweep_hit(None, &actors, 100.0), Ok(Some(hit(30.0, 1.0, 0.3))));
    assert_eq!(choose_sweep_hit(None, &[], 100.0), Ok(None));
}

#[test]
fn the_reach_past_the_bsp_is_capped_at_the_end_point() {
    // A 10-unit sweep, the BSP at 0.8: 5 units past it is 1.3, capped at 1, so a hit beyond the end is not walked.
    let bsp = hit(8.0, 0.0, 0.8);
    assert_eq!(choose_sweep_hit(Some(bsp), &[hit(12.0, 1.0, 1.2)], 10.0), Ok(Some(bsp)));
    assert_eq!(choose_sweep_hit(Some(bsp), &[hit(9.5, 1.0, 0.95)], 10.0), Err(UnreadChoice::Comparator));
}

#[test]
fn every_tied_actor_hit_at_the_minimum_is_kept() {
    let a = hit(20.0, 1.0, 0.2);
    assert_eq!(choose_sweep_hit(None, &[a, a], 100.0), Ok(Some(a)));
    assert_eq!(choose_sweep_hit(None, &[a, hit(21.0, 1.0, 0.2)], 100.0), Err(UnreadChoice::TieOrder));
    assert_eq!(choose_sweep_hit(None, &[a, hit(20.0, 0.0, 0.2)], 100.0), Err(UnreadChoice::TieOrder));
    // Three hits at the minimum, the third differing: still a tie that differs.
    assert_eq!(choose_sweep_hit(None, &[a, a, hit(22.0, 1.0, 0.2)], 100.0), Err(UnreadChoice::TieOrder));
    // A tie above a smaller hit does not matter.
    let actors = [a, hit(21.0, 1.0, 0.2), hit(10.0, 1.0, 0.1)];
    assert_eq!(choose_sweep_hit(None, &actors, 100.0), Ok(Some(hit(10.0, 1.0, 0.1))));
}
