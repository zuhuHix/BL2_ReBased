//! Invented values and geometry only (AGENTS.md rule 9): no game object is named, and the weapon numbers are made up.

use super::*;
use crate::movement::collide::Brush;

fn wall_at(x: f64) -> CollisionWorld {
    let (lo, hi) = ([x, -1000.0, -1000.0], [x + 50.0, 1000.0, 1000.0]);
    let mut points = Vec::new();
    for i in 0..8 {
        points.push([
            if i & 4 == 0 { lo[0] } else { hi[0] },
            if i & 2 == 0 { lo[1] } else { hi[1] },
            if i & 1 == 0 { lo[2] } else { hi[2] },
        ]);
    }
    CollisionWorld::new(vec![Brush::from_hull(&points, &[], 3).unwrap()])
}

const DT: f64 = 1.0 / 60.0;

#[test]
fn a_weapon_file_reads_its_keys_and_refuses_bad_ones() {
    let spec = parse_spec(
        "# a made-up gun\n\
         definition Pkg.Type_A\n\
         balance Pkg.Bal_A\n\
         manufacturer Pkg.Maker_A\n\
         part Body Pkg.Body_A\n\
         part Barrel Pkg.Barrel_A\n\
         level 7\n",
    )
    .unwrap();
    assert_eq!(spec.definition, "Pkg.Type_A");
    assert_eq!(spec.level, 7);
    assert_eq!(spec.parts.len(), 2);
    assert_eq!(spec.manufacturer.as_deref(), Some("Pkg.Maker_A"));
    let data = definition_data(&spec).unwrap();
    assert_eq!(data.kind, ItemKind::Weapon);
    assert!(data.parts.iter().flatten().count() == 2);
    for bad in [
        "level 7\n",
        "definition A.B\n",
        "definition A.B\nlevel 0\n",
        "definition A.B\nlevel 81\n",
        "definition A.B\ndefinition A.C\nlevel 3\n",
        "definition A.B\nlevel 3\ncolour red\n",
        "definition A.B\nlevel 3\npart NoSuchSlot A.C\n",
        "definition A.B\nlevel 3\npart Body\n",
        "definition A.B\nlevel 3\npart Body A.C\npart Body A.D\n",
        "definition A.B C.D\nlevel 3\n",
    ] {
        assert!(parse_spec(bad).is_err(), "{bad:?} was accepted");
    }
}

#[test]
fn a_held_trigger_fires_at_the_fire_interval() {
    let world = wall_at(500.0);
    // Made-up numbers: 4 shots a second, a 2,000 uu range.
    let mut gun = Gun::new(Ok(WeaponStats::new(12.5, 0.25, Some(1.0), Some(2000.0))));
    let mut shots = Vec::new();
    for step in 1..=60u64 {
        if let Some(s) = gun.step(&world, true, [0.0; 3], Rotator::default(), DT, step) {
            shots.push(s);
        }
    }
    // A second of holding: steps 1, 16, 31 and 46 (0.25 s is 15 steps).
    assert_eq!(shots.iter().map(|s| s.step).collect::<Vec<_>>(), vec![1, 16, 31, 46]);
    let s = &shots[0];
    assert_eq!(s.damage, Ok(12.5));
    assert_eq!(s.range_source, RangeSource::Weapon);
    let hit = s.hit.expect("the wall");
    assert!((hit.point[0] - 500.0).abs() < 0.1 && hit.normal == [-1.0, 0.0, 0.0], "{hit:?}");
    assert!((hit.distance - 500.0).abs() < 0.1);
    // Let go: no banked time; pressing again fires at once.
    assert!(gun.step(&world, false, [0.0; 3], Rotator::default(), DT, 61).is_none());
    for step in 62..70 {
        gun.step(&world, false, [0.0; 3], Rotator::default(), DT, step);
    }
    assert!(gun.step(&world, true, [0.0; 3], Rotator::default(), DT, 70).is_some());
}

#[test]
fn the_range_limits_the_ray_and_the_view_aims_it() {
    let world = wall_at(500.0);
    let mut short = Gun::new(Ok(WeaponStats::new(1.0, 0.1, None, Some(400.0))));
    let s = short.step(&world, true, [0.0; 3], Rotator::default(), DT, 1).unwrap();
    assert!(s.hit.is_none() && s.range == 400.0);
    // Turned a quarter turn left (yaw -16384 faces -Y): the wall is not ahead.
    let mut gun = Gun::new(Ok(WeaponStats::new(1.0, 0.1, None, Some(5000.0))));
    let s = gun.step(&world, true, [0.0; 3], Rotator::new(0, -16384, 0), DT, 1).unwrap();
    assert!(s.hit.is_none() && s.direction[1] < -0.99);
}

#[test]
fn without_a_weapon_each_press_traces_and_the_damage_is_not_yet() {
    let world = wall_at(500.0);
    let mut gun = Gun::new(Err(equipped_weapon()));
    let mut fired = 0;
    for step in 1..=30u64 {
        // Held for 10 steps, let go for 10, held again for 10: two presses.
        let held = !(11..=20).contains(&step);
        if let Some(s) = gun.step(&world, held, [0.0; 3], Rotator::default(), DT, step) {
            fired += 1;
            assert_eq!(s.range_source, RangeSource::Probe);
            assert_eq!(s.range, PROBE_RANGE);
            assert!(s.hit.is_some());
            assert!(matches!(&s.damage, Err(n) if n.record == RECORD));
        }
    }
    assert_eq!(fired, 2);
}

/// Review r1 of PR #366, M1: the notification recorder keeps each property once, in the order first raised, and the
/// weapon line names them.
#[test]
fn notifications_are_recorded_once_and_named() {
    use ow_object::ids::{ClassId, PropId};
    let seen = std::cell::RefCell::new(Vec::new());
    let a = PropId { slot: 7, declared_in: ClassId::new(2) };
    let b = PropId { slot: 9, declared_in: ClassId::new(2) };
    for p in [a, b, a, a, b] {
        record_notification(&seen, p);
    }
    assert_eq!(seen.into_inner(), vec![a, b]);
    let mut stats = WeaponStats::new(1.0, 0.1, None, None);
    assert_eq!(stats.notifications_text(), "no change notification raised");
    stats.notifications_not_applied = vec!["ClipSize".into(), "Other".into()];
    assert_eq!(
        stats.notifications_text(),
        "notifications not applied (NotYet, NATIVE_ATTRIBUTES section 4): ClipSize, Other"
    );
}
