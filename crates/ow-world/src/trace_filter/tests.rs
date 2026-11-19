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
    let d = bools(&mut space, &["bWorldGeometry", "bProjTarget", "bBlockActors", "bCollideActors"]);
    let actor = space.classes.define("Engine.Actor", Some(object), d).unwrap();
    // The controller chain (NATIVE_SPAWN_TRACE_FILTER.md section 1) and an unread subclass of it.
    let controller = space.classes.define("Engine.Controller", Some(actor), vec![]).unwrap();
    let player = space.classes.define("Engine.PlayerController", Some(controller), vec![]).unwrap();
    let game = space.classes.define("GameFramework.GamePlayerController", Some(player), vec![]).unwrap();
    // A class at the wrong package path takes no row (review r1 of #514, item 1).
    space.classes.define("Engine.GamePlayerController", Some(player), vec![]).unwrap();
    let willow = space.classes.define("WillowGame.WillowPlayerController", Some(game), vec![]).unwrap();
    space.classes.define("Test.SubPlayerController", Some(willow), vec![]).unwrap();
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
    let mut d = bools(&mut space, &["BlockNonZeroExtent", "BlockZeroExtent", "BlockActors", "CollideActors", "BulletListener"]);
    // The component's own owner field (NATIVE_SPAWN_TRACE_FILTER.md section 2).
    d.push(PropertyDecl::new(space.names.intern("Owner"), PropKind::Object { class: actor }));
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
fn the_controller_classes_run_the_actor_row_and_no_subclass_inherits_it() {
    let w = world();
    let row = |p: &str| row_of(&w.space, w.space.classes.by_path(p).unwrap());
    for p in ["Engine.Controller", "Engine.PlayerController", "GameFramework.GamePlayerController", "WillowGame.WillowPlayerController"] {
        assert_eq!(row(p), Some(Row::Actor), "{p}");
    }
    assert_eq!(row("Engine.GamePlayerController"), None, "the installed path is GameFramework's");
    assert_eq!(row("Test.SubPlayerController"), None);
}

/// NATIVE_SPAWN_TRACE_FILTER.md sections 2, 3 and 7: every read test only rejects. A primitive that passes them all
/// is Unsupported on the hash admission, under that note's record; a primitive any one of them rejects is not tested.
#[test]
fn a_candidate_is_not_tested_when_any_read_test_rejects_it() {
    let mut w = world();
    let prim = w.primitive;
    let candidate = w.obj(prim, &["BlockNonZeroExtent", "CollideActors"]);
    let skipped = w.obj(prim, &["BlockNonZeroExtent", "CollideActors", "BulletListener"]);
    let quiet = w.obj(prim, &["BlockNonZeroExtent"]);
    let zero = w.obj(prim, &["BlockZeroExtent", "CollideActors"]);
    let geometry = w.actor("Engine.StaticMeshActor", &["bWorldGeometry", "bCollideActors"]);
    let inert = w.actor("Engine.StaticMeshActor", &["bWorldGeometry"]);
    let plain = w.actor("Engine.StaticMeshActor", &["bCollideActors"]);
    let mover = w.actor("Engine.InterpActor", &["bCollideActors"]);
    let pawn = w.actor("Engine.Pawn", &["bStationary", "bCollideActors"]);
    let box_test = Query::BoxTest { zero_extent: false, complex: false };
    let zero_test = Query::BoxTest { zero_extent: true, complex: false };
    let t = |w: &World, p, o, q| tested(&w.space, p, o, q).unwrap();
    let admission = Candidate::Unsupported(Unread::HashAdmission);
    // Every read test passes: the unread hash admission.
    assert_eq!(t(&w, candidate, geometry, box_test), admission);
    assert_eq!(t(&w, candidate, geometry, Query::BoxSweep), admission);
    assert_eq!(t(&w, zero, geometry, zero_test), admission);
    assert_eq!(t(&w, zero, geometry, Query::PointSweep), admission);
    assert_eq!(Unread::HashAdmission.rule(), HASH_ADMISSION);
    assert_eq!(Unread::HashAdmission.record(), RECORD_TRACE_FILTER);
    // The skip flag (the spawn's sweeps do not set bit 30, so they keep it).
    assert_eq!(t(&w, skipped, geometry, box_test), Candidate::NotTested);
    assert_eq!(t(&w, skipped, geometry, Query::BoxSweep), Candidate::NotTested);
    assert_eq!(Query::BoxSweep.flags() & SKIP_IGNORED, 0);
    // The blocking flag for the query's kind.
    assert_eq!(t(&w, candidate, geometry, zero_test), Candidate::NotTested);
    assert_eq!(t(&w, candidate, geometry, Query::PointSweep), Candidate::NotTested);
    // The per-component test: the primitive's CollideActors, then the owner's bCollideActors.
    assert_eq!(t(&w, quiet, geometry, box_test), Candidate::NotTested);
    assert_eq!(t(&w, candidate, inert, box_test), Candidate::NotTested);
    // The owner's row: an Actor-row owner without bWorldGeometry, a mover in a box test, a pawn.
    assert_eq!(t(&w, candidate, plain, box_test), Candidate::NotTested);
    assert_eq!(t(&w, candidate, mover, box_test), Candidate::NotTested);
    assert_eq!(t(&w, candidate, mover, Query::BoxSweep), admission);
    assert_eq!(t(&w, candidate, pawn, box_test), Candidate::NotTested);
    assert_eq!(t(&w, candidate, pawn, Query::BoxSweep), Candidate::NotTested);
}

/// Link 9's case (NATIVE_SPAWN_TRACE_FILTER.md section 6): a `WillowPlayerController` and its cylinder with the class
/// defaults' flags (the actor's `bCollideActors` and `bWorldGeometry` clear; the cylinder's `CollideActors` clear and
/// both blocking flags set) is not tested. A controller every read test accepts would still be Unsupported, and an
/// unread subclass names its class.
#[test]
fn a_controller_s_cylinder_is_not_tested() {
    let mut w = world();
    let prim = w.primitive;
    let cylinder = w.obj(prim, &["BlockZeroExtent", "BlockNonZeroExtent"]);
    let controller = w.actor("WillowGame.WillowPlayerController", &[]);
    let t = |w: &World, p, o, q| tested(&w.space, p, o, q).unwrap();
    for q in [
        Query::BoxTest { zero_extent: false, complex: false },
        Query::BoxTest { zero_extent: true, complex: false },
        Query::BoxSweep,
        Query::PointSweep,
    ] {
        assert_eq!(t(&w, cylinder, controller, q), Candidate::NotTested, "{q:?}");
    }
    let live = w.obj(prim, &["BlockNonZeroExtent", "CollideActors"]);
    let colliding = w.actor("WillowGame.WillowPlayerController", &["bCollideActors"]);
    let box_test = Query::BoxTest { zero_extent: false, complex: false };
    // The Actor row rejects a controller without bWorldGeometry in every spawn query.
    assert_eq!(t(&w, live, colliding, box_test), Candidate::NotTested);
    assert_eq!(t(&w, live, colliding, Query::BoxSweep), Candidate::NotTested);
    let geometry = w.actor("WillowGame.WillowPlayerController", &["bCollideActors", "bWorldGeometry"]);
    assert_eq!(t(&w, live, geometry, box_test), Candidate::Unsupported(Unread::HashAdmission));
    let sub = w.actor("Test.SubPlayerController", &["bCollideActors"]);
    assert_eq!(
        t(&w, live, sub, box_test),
        Candidate::Unsupported(Unread::OwnerClass("Test.SubPlayerController".to_string()))
    );
}

/// The owner (review r1 of #502, note section 7 step 2): the component's own `Owner` field when set, otherwise the
/// walked actor.
#[test]
fn the_owner_is_the_component_s_field_when_set_else_the_walked_actor() {
    let mut w = world();
    let prim = w.primitive;
    let p = w.obj(prim, &["BlockNonZeroExtent", "CollideActors"]);
    let colliding = w.actor("Engine.StaticMeshActor", &["bWorldGeometry", "bCollideActors"]);
    let inert = w.actor("Engine.StaticMeshActor", &["bWorldGeometry"]);
    let box_test = Query::BoxTest { zero_extent: false, complex: false };
    let t = |w: &World, o| tested(&w.space, p, o, box_test).unwrap();
    // No owner field set: the walked actor decides.
    assert_eq!(owner_of(&w.space, p, inert).unwrap(), inert);
    assert_eq!(t(&w, inert), Candidate::NotTested);
    assert_eq!(t(&w, colliding), Candidate::Unsupported(Unread::HashAdmission));
    // The field set: it decides, whatever the walked actor.
    let field = w.space.prop(prim, "Owner").unwrap();
    w.space.write(p, PropAddr::scalar(field), Value::Object(Some(colliding))).unwrap();
    assert_eq!(owner_of(&w.space, p, inert).unwrap(), colliding);
    assert_eq!(t(&w, inert), Candidate::Unsupported(Unread::HashAdmission));
    w.space.write(p, PropAddr::scalar(field), Value::Object(Some(inert))).unwrap();
    assert_eq!(t(&w, colliding), Candidate::NotTested);
}

#[test]
fn an_unread_owner_class_is_unsupported_past_the_read_tests_and_terrain_ahead_of_them() {
    let mut w = world();
    let prim = w.primitive;
    let blocking = w.obj(prim, &["BlockNonZeroExtent", "CollideActors"]);
    let idle = w.obj(prim, &[]);
    let unread = w.actor("Test.UnreadPawn", &["bCollideActors"]);
    let terrain = w.actor("Engine.Terrain", &["bCollideActors"]);
    let box_test = Query::BoxTest { zero_extent: false, complex: false };
    let t = |w: &World, p, o, q| tested(&w.space, p, o, q).unwrap();
    assert_eq!(
        t(&w, blocking, unread, box_test),
        Candidate::Unsupported(Unread::OwnerClass("Test.UnreadPawn".to_string()))
    );
    assert_eq!(Unread::OwnerClass("Test.UnreadPawn".into()).rule(), "the trace filter of Test.UnreadPawn");
    assert_eq!(Unread::OwnerClass("Test.UnreadPawn".into()).record(), "NATIVE_SPAWN_PLACEMENT.md");
    // A primitive a read test rejects is not tested, whatever its owner's class; terrain comes ahead of the tests.
    assert_eq!(t(&w, idle, unread, box_test), Candidate::NotTested);
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

/// Review r1 of #514, item 1: a primitive whose owner is exactly `GameFramework.GamePlayerController` takes the
/// `Actor` row (rejected without `bWorldGeometry`, the hash admission with it), never the unread-class refusal.
#[test]
fn the_game_framework_controller_path_takes_the_actor_row() {
    let mut w = world();
    let prim = w.primitive;
    let live = w.obj(prim, &["BlockNonZeroExtent", "CollideActors"]);
    let box_test = Query::BoxTest { zero_extent: false, complex: false };
    let plain = w.actor("GameFramework.GamePlayerController", &["bCollideActors"]);
    assert_eq!(tested(&w.space, live, plain, box_test).unwrap(), Candidate::NotTested);
    let geometry = w.actor("GameFramework.GamePlayerController", &["bCollideActors", "bWorldGeometry"]);
    assert_eq!(tested(&w.space, live, geometry, box_test).unwrap(), Candidate::Unsupported(Unread::HashAdmission));
}

/// Review r1 of #514, item 2: the walked terrain refuses before any owner is resolved, so a controller set as the
/// component's owner cannot discard it; and a terrain set as the owner of a controller's primitive refuses too.
#[test]
fn a_terrain_on_either_side_of_the_owner_refuses() {
    let mut w = world();
    let prim = w.primitive;
    let terrain = w.actor("Engine.Terrain", &["bCollideActors"]);
    let controller = w.actor("WillowGame.WillowPlayerController", &[]);
    let field = w.space.prop(prim, "Owner").unwrap();
    let box_test = Query::BoxTest { zero_extent: false, complex: false };
    // The walked actor is the terrain; the component names the controller as its owner.
    let p = w.obj(prim, &["BlockNonZeroExtent", "CollideActors"]);
    w.space.write(p, PropAddr::scalar(field), Value::Object(Some(controller))).unwrap();
    assert_eq!(tested(&w.space, p, terrain, box_test).unwrap(), Candidate::Unsupported(Unread::TerrainHandling));
    // The walked actor is the controller; the component names the terrain as its owner.
    let q = w.obj(prim, &["BlockNonZeroExtent", "CollideActors"]);
    w.space.write(q, PropAddr::scalar(field), Value::Object(Some(terrain))).unwrap();
    assert_eq!(tested(&w.space, q, controller, box_test).unwrap(), Candidate::Unsupported(Unread::TerrainHandling));
}

/// Review r2 of #514: a walked `Terrain` actor whose primitive names a stale explicit `Owner` (a handle that no longer
/// resolves) refuses on the terrain, not with a lookup error: the walked actor is checked before the owner is resolved.
/// The same stale owner on a non-terrain walked actor shows the lookup does fail once it is reached.
#[test]
fn a_walked_terrain_refuses_before_a_stale_owner_is_resolved() {
    let mut w = world();
    let prim = w.primitive;
    let terrain = w.actor("Engine.Terrain", &["bCollideActors"]);
    let stale = w.actor("Engine.StaticMeshActor", &[]);
    let field = w.space.prop(prim, "Owner").unwrap();
    let p = w.obj(prim, &["BlockNonZeroExtent", "CollideActors"]);
    // The free clears every reference held in the heap, so the stale handle is put back in the slot directly: the
    // primitive's Owner then names a handle that no longer resolves.
    w.space.free(stale).unwrap();
    *w.space.slot_mut(p, PropAddr::scalar(field)).unwrap() = Value::Object(Some(stale));
    let box_test = Query::BoxTest { zero_extent: false, complex: false };
    assert_eq!(tested(&w.space, p, terrain, box_test).unwrap(), Candidate::Unsupported(Unread::TerrainHandling));
    let walked = w.actor("Engine.StaticMeshActor", &["bWorldGeometry", "bCollideActors"]);
    assert!(tested(&w.space, p, walked, box_test).is_err(), "the stale owner fails its lookup once reached");
}
