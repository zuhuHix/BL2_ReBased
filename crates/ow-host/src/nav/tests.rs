//! Invented meshes only (AGENTS.md rule 9): squares split into triangles, written as a `GBXNavMesh` tail in a testkit
//! package and read back with `ow_assets::navmesh::read_navmesh`, so the query runs on what the reader gives.

use super::*;
use ow_assets::navmesh::{PLACED_ACTOR_PREFIX, TRAILING_BYTES, read_navmesh};
use ow_package::package::Package;
use ow_package::testkit::PackageBuilder;

/// A mesh to write: vertices, triangles (vertex indices), runs (radius, first triangle) and a lookup grid.
struct Spec {
    vertices: Vec<[f32; 3]>,
    triangles: Vec<[u16; 3]>,
    runs: Vec<(f32, i32)>,
    grid: (i32, i32, [f32; 3], [f32; 3]),
}

/// Unit squares of `size` uu at grid positions `cells` (x, y), at height `z`, each split into two triangles; vertices
/// shared between squares so neighbours meet on shared edges.
fn squares(
    cells: &[(i32, i32)],
    size: f32,
    z: f32,
    base: &mut Vec<[f32; 3]>,
    tris: &mut Vec<[u16; 3]>,
) {
    let vertex = |x: i32, y: i32, base: &mut Vec<[f32; 3]>| -> u16 {
        let p = [x as f32 * size, y as f32 * size, z];
        if let Some(i) = base.iter().position(|q| *q == p) {
            return i as u16;
        }
        base.push(p);
        (base.len() - 1) as u16
    };
    for &(x, y) in cells {
        let a = vertex(x, y, base);
        let b = vertex(x + 1, y, base);
        let c = vertex(x + 1, y + 1, base);
        let d = vertex(x, y + 1, base);
        tris.push([a, b, c]);
        tris.push([c, d, a]);
    }
}

/// Neighbour `k` of triangle `t`: the other triangle with the edge (vertex k, vertex k + 1).
fn neighbours(tris: &[[u16; 3]]) -> Vec<[u16; 3]> {
    tris.iter()
        .enumerate()
        .map(|(i, t)| {
            let mut n = [NONE; 3];
            for k in 0..3 {
                let e = [t[k], t[(k + 1) % 3]];
                if let Some(j) = tris.iter().enumerate().position(|(j, u)| {
                    j != i
                        && (0..3).any(|m| {
                            [u[m], u[(m + 1) % 3]] == [e[1], e[0]] || [u[m], u[(m + 1) % 3]] == e
                        })
                }) {
                    n[k] = j as u16;
                }
            }
            n
        })
        .collect()
}

fn package(spec: &Spec) -> Package {
    let mut t = Vec::new();
    let i32s = |t: &mut Vec<u8>, v: i32| t.extend_from_slice(&v.to_le_bytes());
    let f32s = |t: &mut Vec<u8>, v: f32| t.extend_from_slice(&v.to_le_bytes());
    let u16s = |t: &mut Vec<u8>, v: u16| t.extend_from_slice(&v.to_le_bytes());
    i32s(&mut t, spec.vertices.len() as i32);
    spec.vertices
        .iter()
        .flatten()
        .for_each(|c| f32s(&mut t, *c));
    i32s(&mut t, spec.triangles.len() as i32);
    for (tri, n) in spec.triangles.iter().zip(neighbours(&spec.triangles)) {
        tri.iter().chain(n.iter()).for_each(|x| u16s(&mut t, *x));
        u16s(&mut t, NONE);
    }
    i32s(&mut t, 0); // no per-triangle data
    i32s(&mut t, spec.runs.len() as i32);
    for (radius, start) in &spec.runs {
        f32s(&mut t, *radius);
        f32s(&mut t, 220.0);
        t.extend_from_slice(&[0; 8]);
        i32s(&mut t, *start);
    }
    let (xc, yc, origin, size) = spec.grid;
    i32s(&mut t, xc);
    i32s(&mut t, yc);
    origin
        .iter()
        .chain(size.iter())
        .for_each(|c| f32s(&mut t, *c));
    // Cells in the game's order (y x XCells + x), each listing the triangles whose plan box touches it.
    let mut lists = Vec::new();
    let mut indices = Vec::new();
    for cy in 0..yc {
        for cx in 0..xc {
            let (x0, y0) = (
                origin[0] + cx as f32 * size[0],
                origin[1] + cy as f32 * size[1],
            );
            let (x1, y1) = (x0 + size[0], y0 + size[1]);
            let list: Vec<u16> = spec
                .triangles
                .iter()
                .enumerate()
                .filter(|(_, tri)| {
                    let xs = tri.map(|v| spec.vertices[v as usize][0]);
                    let ys = tri.map(|v| spec.vertices[v as usize][1]);
                    xs.iter().cloned().fold(f32::MIN, f32::max) >= x0
                        && xs.iter().cloned().fold(f32::MAX, f32::min) <= x1
                        && ys.iter().cloned().fold(f32::MIN, f32::max) >= y0
                        && ys.iter().cloned().fold(f32::MAX, f32::min) <= y1
                })
                .map(|(i, _)| i as u16)
                .collect();
            if list.is_empty() {
                indices.push(-1);
            } else {
                indices.push(lists.len() as i32);
                lists.push(list);
            }
        }
    }
    i32s(&mut t, indices.len() as i32);
    indices.iter().for_each(|i| i32s(&mut t, *i));
    i32s(&mut t, lists.len() as i32);
    for l in &lists {
        i32s(&mut t, l.len() as i32);
        l.iter().for_each(|x| u16s(&mut t, *x));
    }
    i32s(&mut t, 0); // no obstacles
    t.extend(std::iter::repeat_n(0u8, TRAILING_BYTES));
    let mut b = PackageBuilder::new_empty();
    b.names(["None"]);
    let mut payload = vec![0u8; PLACED_ACTOR_PREFIX];
    payload.extend(b.none());
    payload.extend(t);
    let cls = b.add_import("Core", "Class", 0, "GBXNavMesh");
    b.add_export(cls, "GBXNavMesh_0", payload);
    Package::from_decoded(Default::default(), "probe".into(), b.build()).unwrap()
}

fn map(spec: &Spec) -> NavMap {
    NavMap::new(vec![(
        "probe".into(),
        read_navmesh(&package(spec), 1).expect("the invented mesh reads"),
    )])
}

/// A 3 x 3 ring of 100 uu squares around a hole (the middle square), one run of radius 22, a 3 x 3 lookup grid.
fn ring() -> Spec {
    let cells: Vec<(i32, i32)> = (0..3)
        .flat_map(|y| (0..3).map(move |x| (x, y)))
        .filter(|&c| c != (1, 1))
        .collect();
    let (mut vertices, mut triangles) = (Vec::new(), Vec::new());
    squares(&cells, 100.0, 0.0, &mut vertices, &mut triangles);
    Spec {
        vertices,
        triangles,
        runs: vec![(22.0, 0)],
        grid: (3, 3, [0.0, 0.0, 0.0], [100.0, 100.0, 0.0]),
    }
}

fn inside_hole(p: V3) -> bool {
    p[0] > 100.0 + 1e-6 && p[0] < 200.0 - 1e-6 && p[1] > 100.0 + 1e-6 && p[1] < 200.0 - 1e-6
}

#[test]
fn a_straight_run_stays_on_its_floor() {
    let (mut vertices, mut triangles) = (Vec::new(), Vec::new());
    squares(
        &[(0, 0), (1, 0), (2, 0)],
        100.0,
        10.0,
        &mut vertices,
        &mut triangles,
    );
    let spec = Spec {
        vertices,
        triangles,
        runs: vec![(22.0, 0)],
        grid: (3, 1, [0.0, 0.0, 10.0], [100.0, 100.0, 0.0]),
    };
    let path = map(&spec)
        .find_path([20.0, 50.0, 90.0], [280.0, 50.0, 90.0], 20.0)
        .unwrap();
    assert_eq!(path.waypoints.first(), Some(&[20.0, 50.0, 10.0]));
    assert_eq!(path.waypoints.last(), Some(&[280.0, 50.0, 10.0]));
    assert!(path.waypoints.iter().all(|p| p[2] == 10.0));
    assert!(path.length >= 260.0);
    assert_eq!(path.run_radius, 22.0);
    assert_eq!(*path.triangles.first().unwrap(), 1, "the start's triangle");
}

#[test]
fn a_path_goes_around_a_hole_and_never_through_it() {
    let m = map(&ring());
    let path = m
        .find_path([50.0, 50.0, 0.0], [250.0, 250.0, 0.0], 22.0)
        .unwrap();
    assert!(path.waypoints.len() >= 3, "{:?}", path.waypoints);
    for w in path.waypoints.windows(2) {
        for i in 0..=100 {
            let t = f64::from(i) / 100.0;
            let p = [
                w[0][0] + (w[1][0] - w[0][0]) * t,
                w[0][1] + (w[1][1] - w[0][1]) * t,
                0.0,
            ];
            assert!(!inside_hole(p), "segment {w:?} crosses the hole at {p:?}");
        }
    }
    let straight = dist([50.0, 50.0, 0.0], [250.0, 250.0, 0.0]);
    assert!(
        path.length > straight && path.length < 2.0 * straight,
        "{}",
        path.length
    );
    // The same query gives the same path.
    assert_eq!(
        m.find_path([50.0, 50.0, 0.0], [250.0, 250.0, 0.0], 22.0)
            .unwrap(),
        path
    );
}

#[test]
fn points_off_the_mesh_and_unfit_pawns_are_errors() {
    let m = map(&ring());
    assert_eq!(
        m.find_path([150.0, 150.0, 0.0], [250.0, 250.0, 0.0], 22.0),
        Err(NavError::OffMesh(Which::Start, [150.0, 150.0, 0.0]))
    );
    assert_eq!(
        m.find_path([50.0, 50.0, 0.0], [350.0, 50.0, 0.0], 22.0),
        Err(NavError::OffMesh(Which::End, [350.0, 50.0, 0.0]))
    );
    // Too far above or below the floor.
    assert!(matches!(
        m.find_path([50.0, 50.0, 300.0], [250.0, 250.0, 0.0], 22.0),
        Err(NavError::OffMesh(Which::Start, _))
    ));
    assert!(matches!(
        m.find_path([50.0, 50.0, -50.0], [250.0, 250.0, 0.0], 22.0),
        Err(NavError::OffMesh(Which::Start, _))
    ));
    assert_eq!(
        m.find_path([50.0, 50.0, 0.0], [250.0, 250.0, 0.0], 30.0),
        Err(NavError::NoRunFits(30.0))
    );
}

#[test]
fn islands_have_no_path() {
    let (mut vertices, mut triangles) = (Vec::new(), Vec::new());
    squares(&[(0, 0), (2, 0)], 100.0, 0.0, &mut vertices, &mut triangles);
    let spec = Spec {
        vertices,
        triangles,
        runs: vec![(22.0, 0)],
        grid: (3, 1, [0.0, 0.0, 0.0], [100.0, 100.0, 0.0]),
    };
    assert_eq!(
        map(&spec).find_path([50.0, 50.0, 0.0], [250.0, 50.0, 0.0], 22.0),
        Err(NavError::NoPath)
    );
}

#[test]
fn the_run_is_the_narrowest_that_fits_and_floors_stack() {
    // Run 0 (radius 22): a strip at z 0. Run 1 (radius 40): the same strip at z 300, over it.
    let (mut vertices, mut triangles) = (Vec::new(), Vec::new());
    squares(&[(0, 0), (1, 0)], 100.0, 0.0, &mut vertices, &mut triangles);
    let first_wide = triangles.len() as i32;
    squares(
        &[(0, 0), (1, 0)],
        100.0,
        300.0,
        &mut vertices,
        &mut triangles,
    );
    let spec = Spec {
        vertices,
        triangles,
        runs: vec![(22.0, 0), (40.0, first_wide)],
        grid: (2, 1, [0.0, 0.0, 0.0], [100.0, 100.0, 0.0]),
    };
    let m = map(&spec);
    // A narrow pawn uses the 22 run on the lower floor; a pawn of 30 needs the 40 run, on the upper floor.
    let low = m
        .find_path([20.0, 50.0, 80.0], [180.0, 50.0, 80.0], 10.0)
        .unwrap();
    assert_eq!((low.run_radius, low.waypoints[0][2]), (22.0, 0.0));
    let high = m
        .find_path([20.0, 50.0, 380.0], [180.0, 50.0, 380.0], 30.0)
        .unwrap();
    assert_eq!((high.run_radius, high.waypoints[0][2]), (40.0, 300.0));
    assert!(high.triangles.iter().all(|&t| t >= first_wide as u32));
    // Within reach of both floors, the higher one at most STEP_UP above the point is taken.
    let placed = m.locate([20.0, 50.0, 280.0], 30.0).unwrap();
    assert_eq!(placed.on_floor[2], 300.0);
    assert_eq!(
        m.find_path([20.0, 50.0, 80.0], [180.0, 50.0, 80.0], 50.0),
        Err(NavError::NoRunFits(50.0))
    );
}

#[test]
fn arrival_is_within_the_run_radius() {
    let path = NavPath {
        mesh: 0,
        run_radius: 22.0,
        triangles: vec![0],
        waypoints: vec![[0.0; 3], [100.0, 0.0, 0.0]],
        length: 100.0,
    };
    assert!(arrived(&path, [90.0, 10.0, 0.0]));
    assert!(!arrived(&path, [70.0, 0.0, 0.0]));
    assert!(!arrived(&path, [100.0, 0.0, 300.0]));
}

#[test]
fn invalid_queries_are_typed_errors() {
    let m = map(&ring());
    for radius in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            m.find_path([50.0; 3], [50.0; 3], radius),
            Err(NavError::InvalidInput)
        );
    }
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            m.find_path([value, 50.0, 0.0], [50.0; 3], 22.0),
            Err(NavError::InvalidInput)
        );
        assert_eq!(
            m.find_path([50.0; 3], [50.0, value, 0.0], 22.0),
            Err(NavError::InvalidInput)
        );
    }
}

#[test]
fn changing_floor_height_is_preserved_between_waypoints() {
    let mut spec = ring();
    // Non-coplanar adjoining triangles: a shortcut in XY would cut through or float over this floor.
    for v in &mut spec.vertices {
        v[2] = if v[0] == 100.0 { 25.0 } else { 0.0 };
    }
    let m = map(&spec);
    let path = m
        .find_path([50.0, 50.0, 20.0], [250.0, 250.0, 20.0], 22.0)
        .unwrap();
    let mesh = &m.meshes()[0].1;
    for w in path.waypoints.windows(2) {
        for k in 0..=10 {
            let t = k as f64 / 10.0;
            let p = [
                w[0][0] + t * (w[1][0] - w[0][0]),
                w[0][1] + t * (w[1][1] - w[0][1]),
                w[0][2] + t * (w[1][2] - w[0][2]),
            ];
            assert!(path.triangles.iter().any(|&i| {
                NavMap::corners(mesh, i as usize)
                    .and_then(|c| floor_at(c, p[0], p[1]))
                    .is_some_and(|z| (z - p[2]).abs() < 1e-6)
            }));
        }
    }
}

#[test]
fn disconnected_geometric_links_are_not_portals() {
    let mut m = map(&ring());
    let mesh = &mut m.meshes[0].1;
    for p in &mut mesh.polys {
        p.neighbours = [NONE; 3];
    }
    mesh.polys[0].neighbours[0] = 15;
    assert!(NavMap::portal(mesh, 0, 15).is_none());
    assert_eq!(
        NavMap::search(mesh, 0, mesh.polys.len(), 0, 15),
        Err(NavError::NoPath)
    );
}

fn box_brush(lo: V3, hi: V3) -> crate::movement::Brush {
    let points: Vec<_> = (0..8)
        .map(|i| {
            [
                if i & 1 == 0 { lo[0] } else { hi[0] },
                if i & 2 == 0 { lo[1] } else { hi[1] },
                if i & 4 == 0 { lo[2] } else { hi[2] },
            ]
        })
        .collect();
    crate::movement::Brush::from_hull(&points, &[], 0).unwrap()
}

#[test]
fn collision_check_requires_support_and_clearance() {
    use crate::movement::CollisionWorld;
    let p = map(&ring())
        .find_path([50.0, 50.0, 0.0], [250.0, 50.0, 0.0], 22.0)
        .unwrap();
    let floor = || box_brush([-100.0, -100.0, -100.0], [400.0, 400.0, 0.0]);
    assert!(
        p.check_collision(&CollisionWorld::new(vec![floor()]), 22.0, 80.0)
            .is_ok()
    );
    assert!(matches!(
        p.check_collision(&CollisionWorld::new(vec![]), 22.0, 80.0),
        Err(NavError::CollisionUnsupported(_))
    ));
    let wall = box_brush([140.0, -100.0, 0.0], [150.0, 400.0, 200.0]);
    assert!(matches!(
        p.check_collision(&CollisionWorld::new(vec![floor(), wall]), 22.0, 80.0),
        Err(NavError::CollisionUnsupported(_))
    ));
    let enclosing = box_brush([-100.0, -100.0, 0.0], [400.0, 400.0, 200.0]);
    assert!(matches!(
        p.check_collision(&CollisionWorld::new(vec![floor(), enclosing]), 22.0, 80.0),
        Err(NavError::CollisionUnsupported(_))
    ));
}

#[test]
fn locomotion_keeps_and_clears_paths_and_refuses_reference() {
    use ow_core::ParityMode;
    use ow_object::heap::{ObjectSpace, Origin};
    use ow_object::layout::{PropKind, PropertyDecl};
    use ow_object::value::Value;
    let mut space = ObjectSpace::new();
    let fields = ["X", "Y", "Z"].map(|n| PropertyDecl::new(space.names.intern(n), PropKind::Float));
    let vector = space
        .classes
        .define_struct("Toy.Vector", None, fields.to_vec())
        .unwrap();
    let location = PropertyDecl::new(space.names.intern("Location"), PropKind::Struct(vector));
    let class = space
        .classes
        .define("Toy.Pawn", None, vec![location])
        .unwrap();
    let pawn = space.alloc(class, "Pawn", None, Origin::Runtime).unwrap();
    let handle = space.alloc(class, "Handle", None, Origin::Runtime).unwrap();
    let mut vm = Vm::new(
        space,
        ow_defs::DefStore::new_empty(),
        Rc::new(ow_attr::AttributeSystem::new_empty()),
        ParityMode::Off,
    );
    let loc = |p: [f32; 3]| Value::structure(vector, p.into_iter().map(Value::Float).collect());
    ow_combat::hit::access::set(&mut vm, pawn, "Location", loc([50.0, 50.0, 80.0])).unwrap();
    let mut moves = NavLocomotion::new(Rc::new(map(&ring())));
    assert!(matches!(
        moves.create_path(&mut vm, handle, [250.0, 50.0, 0.0]),
        Err(VmError::NotYet(_))
    ));
    moves.register(handle, pawn, 22.0);
    assert!(
        moves
            .create_path(&mut vm, handle, [250.0, 50.0, 0.0])
            .unwrap()
    );
    assert!(moves.path_of(pawn).is_some());
    assert!(moves.path_is_valid(&mut vm, pawn).unwrap());
    assert!(!moves.reached_destination(&mut vm, pawn).unwrap());
    ow_combat::hit::access::set(&mut vm, pawn, "Location", loc([250.0, 50.0, 80.0])).unwrap();
    assert!(moves.reached_destination(&mut vm, pawn).unwrap());
    assert!(
        !moves
            .create_path(&mut vm, handle, [150.0, 150.0, 0.0])
            .unwrap()
    );
    assert!(
        moves.path_of(pawn).is_none(),
        "failed replacement clears an old route"
    );
    assert!(
        moves
            .create_path(&mut vm, handle, [50.0, 50.0, 0.0])
            .unwrap()
    );
    assert!(moves.clear_path(&mut vm, handle, true).unwrap());
    assert!(!moves.path_is_valid(&mut vm, pawn).unwrap());
    moves.set_movement_speed(&mut vm, pawn, 3).unwrap();
    assert_eq!(moves.speed_of(pawn), Some(3));
    vm.parity = ParityMode::Reference;
    assert!(matches!(
        moves.create_path(&mut vm, handle, [50.0, 50.0, 0.0]),
        Err(VmError::NotYet(_))
    ));
}

/// Sanctuary (local only, `OW_BL2_ROOT`): a few paths on the installed meshes, each checked against H4's collision.
/// Nothing is written; the numbers go to stdout.
/// `cargo test -p ow-host --release --locked -- --ignored sanctuary_paths_on_the_installed_game --nocapture`.
#[test]
#[ignore = "reads the installed game (OW_BL2_ROOT)"]
fn sanctuary_paths_on_the_installed_game() {
    let root = std::env::var_os("OW_BL2_ROOT").expect("set OW_BL2_ROOT");
    let install = crate::find_install(std::path::Path::new(&root)).expect("an install");
    let loaded = crate::movement::sanctuary::load(&install, "Sanctuary_P").expect("Sanctuary");
    println!("{}", loaded.text);
    let (map, problems) = NavMap::from_scene(&loaded.scene, &loaded.defs);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(map.meshes().len(), 4);
    let world = &loaded.collision.world;
    let mut passed = 0;
    let (mut queries, mut no_path, mut unsupported, mut collision_refused) = (0, 0, 0, 0);
    // Bounded local evidence only: centroid pairs up to six neighbour hops on the small Land mesh. Stop at three
    // supported routes. Do not pretend the old cross-town routes had a proven H4 floor.
    let mi = map
        .meshes()
        .iter()
        .position(|(n, _)| n == "Sanctuary_Land")
        .expect("Land mesh");
    let mesh = &map.meshes()[mi].1;
    'pairs: for i in 0..mesh.polys.len() {
        let mut frontier = vec![i];
        let mut seen = std::collections::BTreeSet::from([i]);
        for _ in 0..6 {
            let mut next = Vec::new();
            for &t in &frontier {
                for &nb in &mesh.polys[t].neighbours {
                    let j = usize::from(nb);
                    if nb != NONE && j < mesh.polys.len() && seen.insert(j) {
                        next.push(j);
                    }
                }
            }
            frontier = next;
        }
        for j in frontier {
            if j <= i {
                continue;
            }
            queries += 1;
            match map.find_path(NavMap::centroid(mesh, i), NavMap::centroid(mesh, j), 22.0) {
                Ok(path) => match path.check_collision(world, 22.0, 80.0) {
                    Ok(()) => {
                        passed += 1;
                        println!(
                            "supported route {passed}: mesh={} start_triangle={i} end_triangle={j} triangles={} waypoints={} length={} uu; floor sampling + pawn sweeps PASS",
                            map.meshes()[path.mesh].0,
                            path.triangles.len(),
                            path.waypoints.len(),
                            ow_core::fmt::fmt_f(path.length, 1)
                        );
                        if passed == 3 {
                            break 'pairs;
                        }
                    }
                    Err(NavError::CollisionUnsupported(_)) => collision_refused += 1,
                    Err(e) => panic!("unexpected collision input: {e}"),
                },
                Err(NavError::NoPath) => no_path += 1,
                Err(NavError::NotYet(_)) => unsupported += 1,
                Err(e) => panic!("centroid query refused: {e}"),
            }
        }
    }
    println!(
        "installed summary: {queries} queries, {passed} supported, {no_path} no-path, {unsupported} unsupported mesh cases, {collision_refused} H4 collision refusals; no live-game parity claim"
    );
    assert_eq!(
        passed, 3,
        "three bounded routes must have floor samples and clear cylinder sweeps"
    );
}
