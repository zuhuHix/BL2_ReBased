//! Owner: lane NAV-MESH (research, ow-assets addition). Gearbox's navigation mesh: an `Engine.GBXNavMesh` actor's
//! native data, the walkable triangles, their neighbours, the per-triangle data, the path sizes, the lookup grid and
//! the obstacle list. Read only; nothing here finds a path.
//!
//! AI-assisted (Claude Code; review r1 fixes: Codex / GPT-6.1 Sol). Record: `docs/verification/NAVMESH_FORMAT.md`; decision
//! `docs/decisions/2026-11/2026-11-16-75-*`.
//!
//! **Where the data is.** A `GBXNavMesh` is a placed actor (its outer is the level), so its tags start 26 bytes in, the
//! placed-actor prefix the scene loader and the definition store use. Its tags hold only `BuildVersion`, `BuildGUID`,
//! `MeshID`, `ConnectedMeshes`, `RenderComponent`, `Tag` and `EditorQuat`; the navigation data is the untagged tail
//! after them. The class (`Engine.upk`) declares that data as native arrays (`Vertices`, `Polys`, `PolyData`,
//! `PathSizes`, `Obstacles`, `SpecialMovers`, flag `CPF_Native`) and a native `PolyLookup` struct, with the constants
//! `NUM_EDGES` 3, `WORDSIZE` 2, `NUM_EDGE_BYTES` 6, `MAX_POLY_OBSTACLES` 6 and `MAX_POLY_OBSTACLE_BYTES` 12.
//!
//! **The tail, in order** (each array an `i32` count then its elements; every field as the class declares it, in
//! declaration order, which is the reverse of export order):
//! 1. vertices: 3 `f32` each (taken as world uu, one point checked; UNVERIFIED: the record);
//! 2. triangles: `Verts` 3 `u16`, `ConnectedPolys` 3 `u16`, `PolyDataIdx` `u16` (14 bytes); `0xFFFF` is "none";
//!    neighbour `k` is across the edge from vertex `k` to vertex `k + 1` (every shared-edge link in Sanctuary);
//! 3. per-triangle data: `CrossLevelConnections` (count, then `EdgeIdx`, `ConnectedNavmeshIdx`, `PolyIdx`, 3 `u16`),
//!    `SpecialMoves` (count; no Sanctuary mesh has one, so its element layout is not known and a non-zero count is
//!    `NotYet`), `Obstacles` 6 `u16` (`0xFFFF` empty);
//! 4. path sizes: `Radius` `f32`, `Height` `f32`, `DrawColor` and `ObstacleDrawColor` 4 bytes each as stored, and
//!    `StartIndex` `i32` (20 bytes);
//! 5. the lookup grid: `XCells`, `YCells` `i32`, `Origin` and `CellSize` 3 `f32` each, `CellIndices` (count, `i32`
//!    each, -1 for an empty cell), `Cells` (count, then per cell a count and its triangles as `u16`);
//! 6. obstacles: count, then an object reference and `MinAffectedPathSize`, `i32` each;
//! 7. eight bytes, zero in all four Sanctuary meshes, whose meaning is not assigned ([`NavMesh::trailing`]); any
//!    other value is refused ([`NavMeshError::TrailingNotZero`]).
//!
//! `SpecialMovers` (declared native) is not located; the 8 trailing bytes are a candidate (an empty array's count is
//! four zero bytes), UNVERIFIED. Nothing in this reader assigns them.
//!
//! The order and sizes are fitted to the installed packages and checked by structural oracles (record), and the
//! reader enforces these checks, refusing a mesh that fails any:
//! - the parts meet exactly and end with eight zero bytes;
//! - local indices are in range (vertices, neighbours, per-triangle data, cross-level edges, obstacle slots, grid cells
//!   and their triangles, path-size starts);
//! - every per-triangle data entry is used by some triangle;
//! - the grid's index count is `XCells x YCells`, every cell is listed exactly once, and every triangle is in some cell;
//! - every vertex lies inside the grid's extent in plan, within [`GRID_TOLERANCE`];
//! - wherever two triangles linked as neighbours share two vertex indices, those are the slot's edge (vertex `k` to
//!   `k + 1`).
//!
//! **UNVERIFIED** beyond the Sanctuary meshes.

use std::fmt;

use ow_package::package::{Package, PackageError};
use ow_package::props::{self, NoArrayTypes, PropError};
use ow_package::reader::{ReadError, Reader};

/// Where a placed actor's tags start (the placed-actor prefix, decision 2026-11-16-36).
pub const PLACED_ACTOR_PREFIX: usize = 26;
/// `GBXNavMesh.NUM_EDGES`: every polygon is a triangle.
pub const NUM_EDGES: usize = 3;
/// `GBXNavMesh.MAX_POLY_OBSTACLES`.
pub const MAX_POLY_OBSTACLES: usize = 6;
/// The index that means "none" in a 16-bit field.
pub const NONE: u16 = 0xFFFF;
/// Bytes after the obstacle list, zero in every mesh read so far.
pub const TRAILING_BYTES: usize = 8;
/// Inclusive margin outside the lookup grid's extent in x/y (uu). This is a reader tolerance for float rounding,
/// not a game value; z is not checked against this planar lookup grid.
pub const GRID_TOLERANCE: f32 = 1.0;

/// A walkable triangle.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavPoly {
    /// Vertex indices.
    pub verts: [u16; NUM_EDGES],
    /// The triangle across edge `k` (vertex `k` to `k + 1`), or [`NONE`].
    pub neighbours: [u16; NUM_EDGES],
    /// Index into [`NavMesh::poly_data`], or [`NONE`].
    pub poly_data: u16,
}

/// A link from a triangle's edge to a triangle of another navigation mesh.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrossLevelLink {
    pub edge: u16,
    /// An index of the mesh's `ConnectedMeshes` (tagged; not read here). Fitted, UNVERIFIED.
    pub mesh: u16,
    pub poly: u16,
}

/// The data some triangles carry.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavPolyData {
    pub cross_level: Vec<CrossLevelLink>,
    /// Indices into [`NavMesh::obstacles`], [`NONE`] for an empty slot.
    pub obstacles: [u16; MAX_POLY_OBSTACLES],
}

/// One pawn size the mesh was built for.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavPathSize {
    pub radius: f32,
    pub height: f32,
    /// The two editor colours, the four bytes as stored.
    pub draw_color: [u8; 4],
    pub obstacle_draw_color: [u8; 4],
    /// The first triangle of this size's run (the record: the sizes' runs follow each other). Fitted, UNVERIFIED.
    pub start_index: i32,
}

/// The grid that finds the triangles near a point.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct NavLookup {
    pub x_cells: i32,
    pub y_cells: i32,
    pub origin: [f32; 3],
    pub cell_size: [f32; 3],
    /// Per grid cell (`x_cells x y_cells` of them), an index into [`NavLookup::cells`] or -1.
    pub cell_indices: Vec<i32>,
    /// Per listed cell, the triangles that touch it.
    pub cells: Vec<Vec<u16>>,
}

/// An obstacle the mesh knows.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavObstacle {
    /// A reference into the mesh's package (0 none). Fitted, UNVERIFIED.
    pub obstacle: i32,
    pub min_affected_path_size: i32,
}

/// A `GBXNavMesh`'s native data.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct NavMesh {
    pub path: String,
    pub vertices: Vec<[f32; 3]>,
    pub polys: Vec<NavPoly>,
    pub poly_data: Vec<NavPolyData>,
    pub path_sizes: Vec<NavPathSize>,
    pub lookup: NavLookup,
    pub obstacles: Vec<NavObstacle>,
    /// The last eight bytes, kept as read.
    pub trailing: [u8; TRAILING_BYTES],
}

/// Why a mesh could not be read.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavMeshError {
    Package(PackageError),
    Props(PropError),
    Read(ReadError),
    /// The export is not a `GBXNavMesh` (its class name).
    NotNavMesh(String),
    /// A count is negative or larger than the bytes left could hold (the array, the count).
    BadCount(&'static str, i32),
    /// A vertex or a float field is not finite.
    NonFinite(&'static str),
    /// An index points outside what it indexes (what, the value).
    BadIndex(&'static str, i64),
    /// The lookup grid's index count is not `XCells x YCells`, or a cell is listed twice or never.
    BadLookup(&'static str),
    /// A triangle carries a special move: its element layout was never seen, so it is not read.
    SpecialMovesNotRead(i32),
    /// The tail does not end eight bytes after the obstacle list (the bytes left).
    UnexpectedEnd(usize),
    /// The eight trailing bytes are not all zero (as read).
    TrailingNotZero([u8; TRAILING_BYTES]),
    /// A per-triangle data entry no triangle uses (its index).
    UnusedPolyData(usize),
    /// A triangle in no lookup cell (its index).
    UncoveredTriangle(usize),
    /// A vertex outside the lookup grid's extent (its index).
    VertexOutsideGrid(usize),
    /// Two neighbours share two vertex indices that are not the slot's edge (the triangle, the slot).
    NeighbourNotOnEdge(usize, usize),
}

impl fmt::Display for NavMeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NavMeshError::Package(e) => write!(f, "{e}"),
            NavMeshError::Props(e) => write!(f, "{e}"),
            NavMeshError::Read(e) => write!(f, "{e}"),
            NavMeshError::NotNavMesh(c) => write!(f, "export is {c}, not GBXNavMesh"),
            NavMeshError::BadCount(what, n) => write!(f, "navmesh {what}: bad count {n}"),
            NavMeshError::NonFinite(what) => write!(f, "navmesh {what}: a value is not finite"),
            NavMeshError::BadIndex(what, v) => write!(f, "navmesh {what}: index {v} out of range"),
            NavMeshError::BadLookup(what) => write!(f, "navmesh lookup grid: {what}"),
            NavMeshError::SpecialMovesNotRead(n) => write!(
                f,
                "navmesh: a triangle has {n} special moves, whose layout is not read yet (NotYet; docs/verification/NAVMESH_FORMAT.md)"
            ),
            NavMeshError::UnexpectedEnd(left) => {
                write!(f, "navmesh: {left} bytes after the obstacle list, {TRAILING_BYTES} expected")
            }
            NavMeshError::TrailingNotZero(b) => write!(f, "navmesh: the trailing bytes are not zero: {b:?}"),
            NavMeshError::UnusedPolyData(i) => write!(f, "navmesh: per-triangle data {i} is used by no triangle"),
            NavMeshError::UncoveredTriangle(t) => write!(f, "navmesh: triangle {t} is in no lookup cell"),
            NavMeshError::VertexOutsideGrid(v) => write!(f, "navmesh: vertex {v} lies outside the lookup grid"),
            NavMeshError::NeighbourNotOnEdge(t, k) => {
                write!(f, "navmesh: triangle {t}'s neighbour {k} shares two vertices that are not its edge {k}")
            }
        }
    }
}

impl std::error::Error for NavMeshError {}

impl From<PackageError> for NavMeshError {
    fn from(e: PackageError) -> Self {
        NavMeshError::Package(e)
    }
}

impl From<PropError> for NavMeshError {
    fn from(e: PropError) -> Self {
        NavMeshError::Props(e)
    }
}

impl From<ReadError> for NavMeshError {
    fn from(e: ReadError) -> Self {
        NavMeshError::Read(e)
    }
}

/// A count of elements of `size` bytes, refused when negative or larger than the bytes left.
fn count(r: &mut Reader<'_>, what: &'static str, size: usize) -> Result<usize, NavMeshError> {
    let n = r.i32()?;
    let left = r.limit().saturating_sub(r.pos());
    match usize::try_from(n) {
        Ok(k) if k.checked_mul(size.max(1)).is_some_and(|b| b <= left) => Ok(k),
        _ => Err(NavMeshError::BadCount(what, n)),
    }
}

fn u16_at(r: &mut Reader<'_>) -> Result<u16, NavMeshError> {
    let b = r.take(2)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

fn finite(r: &mut Reader<'_>, what: &'static str) -> Result<f32, NavMeshError> {
    let v = f32::from_bits(r.u32()?);
    if v.is_finite() { Ok(v) } else { Err(NavMeshError::NonFinite(what)) }
}

fn vector(r: &mut Reader<'_>, what: &'static str) -> Result<[f32; 3], NavMeshError> {
    Ok([finite(r, what)?, finite(r, what)?, finite(r, what)?])
}

fn bytes4(r: &mut Reader<'_>) -> Result<[u8; 4], NavMeshError> {
    let b = r.take(4)?;
    Ok([b[0], b[1], b[2], b[3]])
}

/// An index that must be below `len`, or [`NONE`] when `none_ok`.
fn index(what: &'static str, v: u16, len: usize, none_ok: bool) -> Result<(), NavMeshError> {
    if (none_ok && v == NONE) || usize::from(v) < len {
        Ok(())
    } else {
        Err(NavMeshError::BadIndex(what, i64::from(v)))
    }
}

/// Reads navigation mesh export `index` of `package` (module documentation).
pub fn read_navmesh(package: &Package, index_: i32) -> Result<NavMesh, NavMeshError> {
    let object = package.object(index_)?;
    let class = if object.class == 0 { String::new() } else { package.object(object.class)?.name.clone() };
    if class != "GBXNavMesh" {
        return Err(NavMeshError::NotNavMesh(class));
    }
    let tagged = props::read_tagged(package, index_, PLACED_ACTOR_PREFIX, &NoArrayTypes)?;
    let (begin, end) = props::property_range(package, index_, PLACED_ACTOR_PREFIX)?;
    let base = begin - PLACED_ACTOR_PREFIX;
    let mut r = package.reader();
    r.set_pos(base + tagged.end);
    r.set_limit(end);

    let nv = count(&mut r, "vertices", 12)?;
    let mut vertices = Vec::with_capacity(nv);
    for _ in 0..nv {
        vertices.push(vector(&mut r, "vertex")?);
    }
    let np = count(&mut r, "polys", 14)?;
    let mut polys = Vec::with_capacity(np);
    for _ in 0..np {
        let mut verts = [0u16; NUM_EDGES];
        for v in &mut verts {
            *v = u16_at(&mut r)?;
        }
        let mut neighbours = [0u16; NUM_EDGES];
        for n in &mut neighbours {
            *n = u16_at(&mut r)?;
        }
        polys.push(NavPoly { verts, neighbours, poly_data: u16_at(&mut r)? });
    }
    let nd = count(&mut r, "poly data", 20)?;
    let mut poly_data = Vec::with_capacity(nd);
    for _ in 0..nd {
        let nc = count(&mut r, "cross-level links", 6)?;
        let mut cross_level = Vec::with_capacity(nc);
        for _ in 0..nc {
            cross_level.push(CrossLevelLink { edge: u16_at(&mut r)?, mesh: u16_at(&mut r)?, poly: u16_at(&mut r)? });
        }
        let special = r.i32()?;
        if special < 0 {
            return Err(NavMeshError::BadCount("special moves", special));
        }
        if special != 0 {
            return Err(NavMeshError::SpecialMovesNotRead(special));
        }
        let mut obstacles = [0u16; MAX_POLY_OBSTACLES];
        for o in &mut obstacles {
            *o = u16_at(&mut r)?;
        }
        poly_data.push(NavPolyData { cross_level, obstacles });
    }
    let ns = count(&mut r, "path sizes", 20)?;
    let mut path_sizes = Vec::with_capacity(ns);
    for _ in 0..ns {
        path_sizes.push(NavPathSize {
            radius: finite(&mut r, "path size radius")?,
            height: finite(&mut r, "path size height")?,
            draw_color: bytes4(&mut r)?,
            obstacle_draw_color: bytes4(&mut r)?,
            start_index: r.i32()?,
        });
    }
    let x_cells = r.i32()?;
    let y_cells = r.i32()?;
    let origin = vector(&mut r, "lookup origin")?;
    let cell_size = vector(&mut r, "lookup cell size")?;
    let ni = count(&mut r, "lookup cell indices", 4)?;
    let mut cell_indices = Vec::with_capacity(ni);
    for _ in 0..ni {
        cell_indices.push(r.i32()?);
    }
    let ncell = count(&mut r, "lookup cells", 4)?;
    let mut cells = Vec::with_capacity(ncell);
    for _ in 0..ncell {
        let k = count(&mut r, "lookup cell triangles", 2)?;
        let mut cell = Vec::with_capacity(k);
        for _ in 0..k {
            cell.push(u16_at(&mut r)?);
        }
        cells.push(cell);
    }
    let nob = count(&mut r, "obstacles", 8)?;
    let mut obstacles = Vec::with_capacity(nob);
    for _ in 0..nob {
        let obstacle = r.i32()?;
        if obstacle != 0 {
            package.object(obstacle)?;
        }
        obstacles.push(NavObstacle { obstacle, min_affected_path_size: r.i32()? });
    }
    let left = end.saturating_sub(r.pos());
    if left != TRAILING_BYTES {
        return Err(NavMeshError::UnexpectedEnd(left));
    }
    let tail = r.take(TRAILING_BYTES)?;
    let mut trailing = [0u8; TRAILING_BYTES];
    trailing.copy_from_slice(tail);
    if trailing != [0; TRAILING_BYTES] {
        return Err(NavMeshError::TrailingNotZero(trailing));
    }

    // The structural checks of the record.
    let mut used = vec![false; poly_data.len()];
    for p in &polys {
        for v in p.verts {
            index("triangle vertex", v, vertices.len(), false)?;
        }
        for n in p.neighbours {
            index("triangle neighbour", n, polys.len(), true)?;
        }
        index("triangle data", p.poly_data, poly_data.len(), true)?;
        if let Some(u) = (p.poly_data != NONE).then(|| used.get_mut(usize::from(p.poly_data))).flatten() {
            *u = true;
        }
    }
    if let Some(i) = used.iter().position(|u| !u) {
        return Err(NavMeshError::UnusedPolyData(i));
    }
    for (t, p) in polys.iter().enumerate() {
        for (k, &n) in p.neighbours.iter().enumerate() {
            let Some(q) = (n != NONE).then(|| polys.get(usize::from(n))).flatten() else { continue };
            let mut shared: Vec<u16> = p.verts.iter().copied().filter(|v| q.verts.contains(v)).collect();
            shared.sort_unstable();
            shared.dedup();
            if shared.len() == 2 {
                let edge = [p.verts[k], p.verts[(k + 1) % NUM_EDGES]];
                if !(shared.contains(&edge[0]) && shared.contains(&edge[1])) {
                    return Err(NavMeshError::NeighbourNotOnEdge(t, k));
                }
            }
        }
    }
    for d in &poly_data {
        for c in &d.cross_level {
            index("cross-level edge", c.edge, NUM_EDGES, false)?;
        }
        for o in d.obstacles {
            index("triangle obstacle", o, obstacles.len(), true)?;
        }
    }
    for s in &path_sizes {
        if s.start_index < 0 || usize::try_from(s.start_index).map_or(true, |i| i > polys.len()) {
            return Err(NavMeshError::BadIndex("path size start", i64::from(s.start_index)));
        }
    }
    let grid = i64::from(x_cells).checked_mul(i64::from(y_cells));
    if x_cells < 0 || y_cells < 0 || grid != i64::try_from(cell_indices.len()).ok() {
        return Err(NavMeshError::BadLookup("the cell index count is not XCells x YCells"));
    }
    let mut listed = vec![false; cells.len()];
    for &c in &cell_indices {
        if c == -1 {
            continue;
        }
        let slot = usize::try_from(c).ok().and_then(|i| listed.get_mut(i));
        match slot {
            Some(seen) if !*seen => *seen = true,
            Some(_) => return Err(NavMeshError::BadLookup("a cell is listed twice")),
            None => return Err(NavMeshError::BadIndex("lookup cell index", i64::from(c))),
        }
    }
    if listed.iter().any(|seen| !seen) {
        return Err(NavMeshError::BadLookup("a cell is never listed"));
    }
    let mut covered = vec![false; polys.len()];
    for cell in &cells {
        for &t in cell {
            index("lookup cell triangle", t, polys.len(), false)?;
            if let Some(c) = covered.get_mut(usize::from(t)) {
                *c = true;
            }
        }
    }
    if let Some(t) = covered.iter().position(|c| !c) {
        return Err(NavMeshError::UncoveredTriangle(t));
    }
    // Compute Origin + cell count * CellSize in f64 before applying the reader margin. Promoting before the
    // product avoids f32 overflow for finite cell sizes; x_cells and y_cells are nonnegative here.
    let extent = |axis: usize, cells: i32| {
        let start = f64::from(origin[axis]);
        let end = start + f64::from(cells) * f64::from(cell_size[axis]);
        (start.min(end) - f64::from(GRID_TOLERANCE), start.max(end) + f64::from(GRID_TOLERANCE))
    };
    let ((x0, x1), (y0, y1)) = (extent(0, x_cells), extent(1, y_cells));
    if let Some(v) = vertices.iter().position(|p| {
        let (x, y) = (f64::from(p[0]), f64::from(p[1]));
        !(x >= x0 && x <= x1 && y >= y0 && y <= y1)
    }) {
        return Err(NavMeshError::VertexOutsideGrid(v));
    }
    Ok(NavMesh {
        path: package.path_of(index_)?,
        vertices,
        polys,
        poly_data,
        path_sizes,
        lookup: NavLookup { x_cells, y_cells, origin, cell_size, cell_indices, cells },
        obstacles,
        trailing,
    })
}

#[cfg(test)]
mod tests;
