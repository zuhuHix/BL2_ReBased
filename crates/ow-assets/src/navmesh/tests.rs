//! Synthetic meshes only (AGENTS.md rule 9): two invented triangles in a package built with the testkit.

use super::*;
use ow_package::testkit::PackageBuilder;

/// The parts of an invented tail, changed one at a time by the refusal cases.
#[derive(Clone)]
struct Mesh {
    vertices: Vec<[f32; 3]>,
    polys: Vec<([u16; 3], [u16; 3], u16)>,
    poly_data: Vec<(Vec<[u16; 3]>, i32, [u16; 6])>,
    path_sizes: Vec<(f32, f32, i32)>,
    grid: (i32, i32),
    cell_indices: Vec<i32>,
    cells: Vec<Vec<u16>>,
    obstacles: Vec<(i32, i32)>,
    trailing: usize,
    /// The value of the trailing bytes (H1 of review r1).
    trailing_bytes: [u8; TRAILING_BYTES],
    /// The lookup grid's origin and cell size as written.
    origin: [f32; 3],
    cell_size: [f32; 3],
}

fn square() -> Mesh {
    Mesh {
        vertices: vec![[0.0, 0.0, 10.0], [100.0, 0.0, 10.0], [100.0, 100.0, 10.0], [0.0, 100.0, 10.0]],
        // Triangle 0 (0, 1, 2) meets triangle 1 (2, 3, 0) across its edge 2 (vertex 2 to 0); triangle 1's edge 2
        // (vertex 0 to 2) is that edge.
        polys: vec![([0, 1, 2], [NONE, NONE, 1], 0), ([2, 3, 0], [NONE, NONE, 0], NONE)],
        poly_data: vec![(vec![[1, 0, 7]], 0, [NONE; 6])],
        path_sizes: vec![(22.0, 220.0, 0)],
        grid: (2, 1),
        cell_indices: vec![0, -1],
        cells: vec![vec![0, 1]],
        obstacles: vec![],
        trailing: TRAILING_BYTES,
        trailing_bytes: [0; TRAILING_BYTES],
        origin: [-50.0, -50.0, 10.0],
        cell_size: [100.0, 200.0, 0.0],
    }
}

fn tail(m: &Mesh) -> Vec<u8> {
    let mut t = Vec::new();
    let i32s = |t: &mut Vec<u8>, v: i32| t.extend_from_slice(&v.to_le_bytes());
    let u16s = |t: &mut Vec<u8>, v: u16| t.extend_from_slice(&v.to_le_bytes());
    let f32s = |t: &mut Vec<u8>, v: f32| t.extend_from_slice(&v.to_le_bytes());
    i32s(&mut t, m.vertices.len() as i32);
    for v in &m.vertices {
        v.iter().for_each(|c| f32s(&mut t, *c));
    }
    i32s(&mut t, m.polys.len() as i32);
    for (v, n, d) in &m.polys {
        v.iter().chain(n.iter()).for_each(|x| u16s(&mut t, *x));
        u16s(&mut t, *d);
    }
    i32s(&mut t, m.poly_data.len() as i32);
    for (cross, special, obstacles) in &m.poly_data {
        i32s(&mut t, cross.len() as i32);
        cross.iter().flatten().for_each(|x| u16s(&mut t, *x));
        i32s(&mut t, *special);
        obstacles.iter().for_each(|x| u16s(&mut t, *x));
    }
    i32s(&mut t, m.path_sizes.len() as i32);
    for (radius, height, start) in &m.path_sizes {
        f32s(&mut t, *radius);
        f32s(&mut t, *height);
        t.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        i32s(&mut t, *start);
    }
    i32s(&mut t, m.grid.0);
    i32s(&mut t, m.grid.1);
    m.origin.iter().chain(m.cell_size.iter()).for_each(|c| f32s(&mut t, *c));
    i32s(&mut t, m.cell_indices.len() as i32);
    m.cell_indices.iter().for_each(|c| i32s(&mut t, *c));
    i32s(&mut t, m.cells.len() as i32);
    for cell in &m.cells {
        i32s(&mut t, cell.len() as i32);
        cell.iter().for_each(|x| u16s(&mut t, *x));
    }
    i32s(&mut t, m.obstacles.len() as i32);
    for (o, s) in &m.obstacles {
        i32s(&mut t, *o);
        i32s(&mut t, *s);
    }
    t.extend(m.trailing_bytes.iter().copied().cycle().take(m.trailing));
    t
}

fn package(m: &Mesh, class: &str) -> Package {
    let mut b = PackageBuilder::new_empty();
    b.names(["None"]);
    let mut payload = vec![0u8; PLACED_ACTOR_PREFIX];
    payload.extend(b.int_tag("BuildVersion", 4));
    payload.extend(b.none());
    payload.extend(tail(m));
    let cls = b.add_import("Core", "Class", 0, class);
    b.add_export(cls, "GBXNavMesh_0", payload);
    Package::from_decoded(Default::default(), "probe".into(), b.build()).unwrap()
}

#[test]
fn an_invented_mesh_reads_back() {
    let mesh = read_navmesh(&package(&square(), "GBXNavMesh"), 1).unwrap();
    assert_eq!(mesh.path, "GBXNavMesh_0");
    assert_eq!(mesh.vertices.len(), 4);
    assert_eq!(mesh.vertices[2], [100.0, 100.0, 10.0]);
    assert_eq!(mesh.polys[0], NavPoly { verts: [0, 1, 2], neighbours: [NONE, NONE, 1], poly_data: 0 });
    assert_eq!(mesh.poly_data[0].cross_level, vec![CrossLevelLink { edge: 1, mesh: 0, poly: 7 }]);
    assert_eq!(mesh.path_sizes[0].radius, 22.0);
    assert_eq!(mesh.path_sizes[0].draw_color, [1, 2, 3, 4]);
    assert_eq!((mesh.lookup.x_cells, mesh.lookup.y_cells), (2, 1));
    assert_eq!(mesh.lookup.cells, vec![vec![0, 1]]);
    assert_eq!(mesh.lookup.cell_size, [100.0, 200.0, 0.0]);
    assert!(mesh.obstacles.is_empty());
    assert_eq!(mesh.trailing, [0; TRAILING_BYTES]);
}

#[test]
fn each_trailing_byte_must_be_zero() {
    for byte in 0..TRAILING_BYTES {
        let mut m = square();
        m.trailing_bytes[byte] = 1;
        assert!(matches!(
            read_navmesh(&package(&m, "GBXNavMesh"), 1),
            Err(NavMeshError::TrailingNotZero(b)) if b == m.trailing_bytes
        ));
    }
}

#[test]
fn grid_margin_is_planar_and_inclusive() {
    // The invented grid spans x [-50, 150], y [-50, 150]. Each boundary accepts exactly the 1 uu margin.
    for (axis, boundary, direction) in [(0, -50.0, -1.0), (0, 150.0, 1.0), (1, -50.0, -1.0), (1, 150.0, 1.0)] {
        let mut m = square();
        m.vertices[0][axis] = boundary + direction * GRID_TOLERANCE;
        // A lookup grid has no z extent check.
        m.vertices[0][2] = 9000.0;
        read_navmesh(&package(&m, "GBXNavMesh"), 1).unwrap();
        m.vertices[0][axis] += direction * 0.25;
        assert!(matches!(
            read_navmesh(&package(&m, "GBXNavMesh"), 1),
            Err(NavMeshError::VertexOutsideGrid(0))
        ));
    }
}

#[test]
fn what_is_not_proven_is_refused() {
    let base = square();
    let mut cases: Vec<(Mesh, fn(&NavMeshError) -> bool)> = Vec::new();
    let mut m = base.clone();
    m.polys[0].0[2] = 4;
    cases.push((m, |e| matches!(e, NavMeshError::BadIndex("triangle vertex", 4))));
    let mut m = base.clone();
    m.polys[1].1[0] = 2;
    cases.push((m, |e| matches!(e, NavMeshError::BadIndex("triangle neighbour", 2))));
    let mut m = base.clone();
    m.poly_data[0].1 = 1;
    cases.push((m, |e| matches!(e, NavMeshError::SpecialMovesNotRead(1))));
    let mut m = base.clone();
    m.poly_data[0].0[0][0] = 3;
    cases.push((m, |e| matches!(e, NavMeshError::BadIndex("cross-level edge", 3))));
    let mut m = base.clone();
    m.poly_data[0].2[0] = 0;
    cases.push((m, |e| matches!(e, NavMeshError::BadIndex("triangle obstacle", 0))));
    let mut m = base.clone();
    m.cell_indices = vec![0];
    cases.push((m, |e| matches!(e, NavMeshError::BadLookup(_))));
    let mut m = base.clone();
    m.cell_indices = vec![0, 0];
    cases.push((m, |e| matches!(e, NavMeshError::BadLookup(_))));
    let mut m = base.clone();
    m.cell_indices = vec![-1, -1];
    cases.push((m, |e| matches!(e, NavMeshError::BadLookup(_))));
    let mut m = base.clone();
    m.cells[0].push(2);
    cases.push((m, |e| matches!(e, NavMeshError::BadIndex("lookup cell triangle", 2))));
    let mut m = base.clone();
    m.path_sizes[0].2 = 3;
    cases.push((m, |e| matches!(e, NavMeshError::BadIndex("path size start", 3))));
    let mut m = base.clone();
    m.trailing = 4;
    cases.push((m, |e| matches!(e, NavMeshError::UnexpectedEnd(4))));
    let mut m = base.clone();
    m.trailing = 12;
    cases.push((m, |e| matches!(e, NavMeshError::UnexpectedEnd(12))));
    let mut m = base.clone();
    m.vertices[1][0] = f32::NAN;
    cases.push((m, |e| matches!(e, NavMeshError::NonFinite("vertex"))));
    // Review r1 of PR #373: the checks the record listed and the reader lacked (H1, M1), and more refusals (L1, L2).
    let mut m = base.clone();
    m.trailing_bytes = [1; TRAILING_BYTES];
    cases.push((m, |e| matches!(e, NavMeshError::TrailingNotZero([1, 1, 1, 1, 1, 1, 1, 1]))));
    let mut m = base.clone();
    m.poly_data.push((vec![], 0, [NONE; 6]));
    cases.push((m, |e| matches!(e, NavMeshError::UnusedPolyData(1))));
    let mut m = base.clone();
    m.cells = vec![vec![0]];
    cases.push((m, |e| matches!(e, NavMeshError::UncoveredTriangle(1))));
    let mut m = base.clone();
    m.vertices[3] = [0.0, 400.0, 10.0];
    cases.push((m, |e| matches!(e, NavMeshError::VertexOutsideGrid(3))));
    let mut m = base.clone();
    // Triangle 0's neighbour 1 (edge 1 to 2) shares vertices 0 and 2 with triangle 1: not that edge.
    m.polys[0].1 = [NONE, 1, NONE];
    m.polys[1].1 = [NONE, NONE, 0];
    cases.push((m, |e| matches!(e, NavMeshError::NeighbourNotOnEdge(0, 1))));
    let mut m = base.clone();
    // Repeated endpoints cannot stand for the two distinct shared vertices, even when both occur in the neighbour.
    m.polys[0].0 = [0, 0, 2];
    m.polys[0].1 = [1, NONE, NONE];
    cases.push((m, |e| matches!(e, NavMeshError::NeighbourNotOnEdge(0, 0))));
    let mut m = base.clone();
    m.polys[0].2 = 1;
    cases.push((m, |e| matches!(e, NavMeshError::BadIndex("triangle data", 1))));
    let mut m = base.clone();
    m.cell_indices = vec![1, -1];
    cases.push((m, |e| matches!(e, NavMeshError::BadIndex("lookup cell index", 1))));
    let mut m = base.clone();
    m.cell_indices = vec![0, -2];
    cases.push((m, |e| matches!(e, NavMeshError::BadIndex("lookup cell index", -2))));
    let mut m = base.clone();
    m.grid = (-2, 1);
    cases.push((m, |e| matches!(e, NavMeshError::BadLookup(_))));
    let mut m = base.clone();
    m.grid = (2, -1);
    cases.push((m, |e| matches!(e, NavMeshError::BadLookup(_))));
    let mut m = base.clone();
    m.path_sizes[0].0 = f32::INFINITY;
    cases.push((m, |e| matches!(e, NavMeshError::NonFinite("path size radius"))));
    let mut m = base.clone();
    m.origin[0] = f32::NAN;
    cases.push((m, |e| matches!(e, NavMeshError::NonFinite("lookup origin"))));
    let mut m = base.clone();
    m.cell_size[1] = f32::NEG_INFINITY;
    cases.push((m, |e| matches!(e, NavMeshError::NonFinite("lookup cell size"))));
    let mut m = base.clone();
    m.obstacles = vec![(99, 0)];
    m.poly_data[0].2[0] = 0;
    cases.push((m, |e| matches!(e, NavMeshError::Package(_))));
    let mut m = base.clone();
    m.poly_data[0].1 = -1;
    cases.push((m, |e| matches!(e, NavMeshError::BadCount("special moves", -1))));
    for (i, (mesh, expected)) in cases.into_iter().enumerate() {
        let got = read_navmesh(&package(&mesh, "GBXNavMesh"), 1);
        match got {
            Err(e) => assert!(expected(&e), "case {i}: {e:?}"),
            Ok(_) => panic!("case {i} was accepted"),
        }
    }
    // A count larger than the bytes left is refused before anything is allocated.
    let mut bytes = tail(&base);
    bytes[0..4].copy_from_slice(&1_000_000i32.to_le_bytes());
    let mut b = PackageBuilder::new_empty();
    b.names(["None"]);
    let mut payload = vec![0u8; PLACED_ACTOR_PREFIX];
    payload.extend(b.none());
    payload.extend(bytes);
    let cls = b.add_import("Core", "Class", 0, "GBXNavMesh");
    b.add_export(cls, "GBXNavMesh_0", payload);
    let p = Package::from_decoded(Default::default(), "probe".into(), b.build()).unwrap();
    assert!(matches!(read_navmesh(&p, 1), Err(NavMeshError::BadCount("vertices", 1_000_000))));
    // An oversize nested count (a lookup cell's triangle list) is refused the same way (L1).
    let mut nested = base.clone();
    nested.cells = vec![vec![0, 1]];
    let mut bytes = tail(&nested);
    // The cell's count sits after the cell-list count, 2 u16 and the obstacle count and trailing bytes from the end.
    let at = bytes.len() - TRAILING_BYTES - 4 - 2 * 2 - 4;
    bytes[at..at + 4].copy_from_slice(&500_000i32.to_le_bytes());
    let mut b = PackageBuilder::new_empty();
    b.names(["None"]);
    let mut payload = vec![0u8; PLACED_ACTOR_PREFIX];
    payload.extend(b.none());
    payload.extend(bytes);
    let cls = b.add_import("Core", "Class", 0, "GBXNavMesh");
    b.add_export(cls, "GBXNavMesh_0", payload);
    let p = Package::from_decoded(Default::default(), "probe".into(), b.build()).unwrap();
    assert!(matches!(read_navmesh(&p, 1), Err(NavMeshError::BadCount("lookup cell triangles", 500_000))));
    // Another class.
    assert!(matches!(
        read_navmesh(&package(&base, "Pylon"), 1),
        Err(NavMeshError::NotNavMesh(c)) if c == "Pylon"
    ));
}

/// The installed Sanctuary meshes (local only): `OW_BL2_ROOT` names the install; the counts go to stdout, nothing is
/// written. `cargo test -p ow-assets --locked -- --ignored sanctuary_navmeshes_on_the_installed_game --nocapture`.
#[test]
#[ignore = "reads the installed game (OW_BL2_ROOT)"]
fn sanctuary_navmeshes_on_the_installed_game() {
    let root = std::env::var_os("OW_BL2_ROOT").expect("set OW_BL2_ROOT to the Borderlands 2 install folder");
    let cooked = std::path::Path::new(&root).join("WillowGame").join("CookedPCConsole");
    let mut store = ow_package::store::PackageStore::open(cooked).expect("the store");
    for level in ["Sanctuary_P", "Sanctuary_Land", "Sanctuary_Light", "Sanctuary_Dynamic"] {
        let id = store.load(&ow_core::PackageName::parse(level).unwrap()).expect("the level");
        let pkg = store.get(id);
        let index = (1..=pkg.exports.len() as i32)
            .find(|&i| pkg.object(i).ok().and_then(|o| pkg.object(o.class).ok()).is_some_and(|c| c.name == "GBXNavMesh"))
            .expect("a GBXNavMesh");
        let m = read_navmesh(pkg, index).expect("the mesh reads");
        let links = m.poly_data.iter().map(|d| d.cross_level.len()).sum::<usize>();
        println!(
            "{level}: vertices {}, triangles {}, poly data {} (cross-level {links}), path sizes {}, grid {}x{} with {} cells, obstacles {}, trailing {:?}",
            m.vertices.len(),
            m.polys.len(),
            m.poly_data.len(),
            m.path_sizes.len(),
            m.lookup.x_cells,
            m.lookup.y_cells,
            m.lookup.cells.len(),
            m.obstacles.len(),
            m.trailing
        );
    }
}
