//! Owner: lane H-NAV. A **host stand-in** path query over the game's own navigation mesh (`ow_assets::navmesh`), and
//! the AI's movement seam answered with it (`ow_ai::awareness::actions::Locomotion`). It is not the game's
//! path-finder: `GBXNavMeshPathFinder.FindPathToLocation` and its path modifiers are natives whose rules are not read,
//! so every rule below is the host's own and **UNVERIFIED** as the game's. Parity needs a clean-room analyst lane.
//!
//! AI-assisted (Claude Code; completion: Codex / GPT-6.1 Sol). Decision: `docs/decisions/2026-11/2026-11-16-78-*`; format record
//! `docs/verification/NAVMESH_FORMAT.md`.
//!
//! **The query** ([`NavMap::find_path`]: a start, an end and a pawn radius, in uu, Z-up; no unit or axis change, the
//! mesh is in the game's frame like everything below `frame.rs`):
//! 1. *The run.* A mesh's triangles come in runs, one per path size (`StartIndex` to the next one's). The run used is
//!    the one with the smallest radius at or above the pawn's (it was built for a pawn that wide); none fits is
//!    [`NavError::NoRunFits`].
//! 2. *The triangles under the points* ([`NavMap::locate`]). The point's lookup cell (`Origin`, `CellSize`; the cell
//!    index is `y x XCells + x`, an oracle in decision 78), then the cell's triangles of the run whose plan (x, y)
//!    contains the point, then the highest floor at most [`STEP_UP`] above the point and at most the run's `Height`
//!    below it. A point on no triangle is [`NavError::OffMesh`]. Start and end on different meshes (levels) is
//!    `NotYet`: the cross-level links name meshes through `ConnectedMeshes`, which is not typed yet.
//! 3. *The search.* A* over the run's triangles, a triangle's neighbours being its `ConnectedPolys` inside the run
//!    (links into another run are not followed), step cost and estimate the distance between triangle centroids,
//!    ties broken by triangle index, so the same query gives the same path. No path is [`NavError::NoPath`].
//! 4. *The waypoints.* Start, triangle centroids, shared-edge midpoints, end. Each segment stays in one planar
//!    triangle; no planar shortcut across changing floor heights. Only neighbours sharing the slot's exact edge
//!    are followed. Unresolved geometric links and obstacle-bearing triangles are conservatively excluded.
//!    The query alone proves no collision clearance: call [`NavPath::check_collision`] before walking a route.
//!
//! **Movement seam** ([`NavLocomotion`]). The navigation handle declares no pawn, so the host registers each handle's
//! pawn and radius ([`NavLocomotion::register`]); an unregistered handle is `NotYet`. A path made is kept per pawn for
//! the host to follow ([`NavLocomotion::path_of`]); following it, and the latent natives, are the host AI lane's.

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::fmt;
use std::rc::Rc;

use ow_ai::awareness::Vec3;
use ow_ai::awareness::actions::Locomotion;
use ow_assets::navmesh::{NONE, NavMesh};
use ow_core::num::sqrt_f64;
use ow_core::{DetMap, NotYet};
use ow_object::ids::ObjRef;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::movement::geom::{V3, floor_index};

/// The record a `NotYet` of this module names.
pub const RECORD: &str =
    "docs/decisions/2026-11/2026-11-16-78-host-path-stand-in-over-the-navigation-mesh-h-nav.md";

/// How far above a point a floor may be and still count as under it, uu: the pawn's `MaxStepHeight` (35, read live,
/// `movement::mover::MovementParams::maya`). A host choice for the stand-in.
pub const STEP_UP: f64 = 35.0;

/// Which point a query could not place.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    Start,
    End,
}

/// Why a query gave no path.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum NavError {
    /// Non-finite coordinates, or a non-positive/non-finite pawn dimension.
    InvalidInput,
    /// H4 cannot support the route: missing/mismatched floor or blocked pawn sweep.
    CollisionUnsupported(&'static str),
    /// No mesh has a run for a pawn this wide (the radius).
    NoRunFits(f64),
    /// The point is on no triangle of a fitting run.
    OffMesh(Which, V3),
    /// The two points are on the same mesh and run, but no chain of neighbours joins them.
    NoPath,
    /// What is not read or built yet (a path across meshes, an unregistered handle).
    NotYet(NotYet),
}

impl fmt::Display for NavError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NavError::InvalidInput => f.write_str("invalid navigation input"),
            NavError::CollisionUnsupported(reason) => {
                write!(f, "route unsupported by H4 collision: {reason}")
            }
            NavError::NoRunFits(r) => write!(
                f,
                "no navigation mesh run fits a pawn of radius {}",
                ow_core::fmt::fmt_f(*r, 1)
            ),
            NavError::OffMesh(w, p) => write!(
                f,
                "the {} point ({} {} {}) is on no navigation triangle",
                if *w == Which::Start { "start" } else { "end" },
                ow_core::fmt::fmt_f(p[0], 1),
                ow_core::fmt::fmt_f(p[1], 1),
                ow_core::fmt::fmt_f(p[2], 1)
            ),
            NavError::NoPath => f.write_str("no path joins the two points on the navigation mesh"),
            NavError::NotYet(n) => write!(f, "{n}"),
        }
    }
}

impl std::error::Error for NavError {}

/// A path: the mesh and run it is on, the triangles it crosses and its waypoints on the floor.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct NavPath {
    /// Index of the mesh in the [`NavMap`].
    pub mesh: usize,
    /// The radius of the run used.
    pub run_radius: f64,
    pub triangles: Vec<u32>,
    /// From the start to the end, on the floor, uu.
    pub waypoints: Vec<V3>,
    /// The sum of the segments' lengths, uu.
    pub length: f64,
}

impl NavPath {
    /// Conservative headless H4 check, not a game-pathfinder or continuous floor proof. Samples floor support at
    /// most every 10 uu (within 1 uu in height, walkable normal Z >= 0.70), then sweeps the pawn just above it.
    /// Missing collision, start-solid and slopes needing a different cylinder lift are typed refusals.
    pub fn check_collision(
        &self,
        world: &crate::movement::CollisionWorld,
        radius: f64,
        half_height: f64,
    ) -> Result<(), NavError> {
        use crate::movement::collide::{SKIN, Shape};
        if !radius.is_finite()
            || radius <= 0.0
            || !half_height.is_finite()
            || half_height <= 0.0
            || radius > self.run_radius
            || self.waypoints.is_empty()
            || !self.waypoints.iter().flatten().all(|x| x.is_finite())
        {
            return Err(NavError::InvalidInput);
        }
        let ray = Shape::Cylinder {
            radius: 0.0,
            half_height: 0.0,
        };
        let pawn = Shape::Cylinder {
            radius,
            half_height,
        };
        let check_floor = |p: V3| {
            let hit = world.trace(&ray, [p[0], p[1], p[2] + 2.0], [p[0], p[1], p[2] - 2.0]);
            if !hit.blocked()
                || hit.start_solid
                || (hit.end[2] - p[2]).abs() > 1.0
                || hit.normal[2] < 0.70
            {
                Err(NavError::CollisionUnsupported(
                    "missing, non-walkable or mismatched floor",
                ))
            } else {
                Ok(())
            }
        };
        check_floor(self.waypoints[0])?;
        let first = self.waypoints[0];
        let standing = [first[0], first[1], first[2] + half_height + 2.0 * SKIN];
        let hit = world.trace(&pawn, standing, standing);
        if hit.blocked() || hit.start_solid {
            return Err(NavError::CollisionUnsupported(
                "blocked or start-solid pawn placement",
            ));
        }
        for w in self.waypoints.windows(2) {
            let steps = (dist(w[0], w[1]) / 10.0).ceil().max(1.0) as usize;
            for i in 1..=steps {
                let t = i as f64 / steps as f64;
                check_floor([
                    w[0][0] + (w[1][0] - w[0][0]) * t,
                    w[0][1] + (w[1][1] - w[0][1]) * t,
                    w[0][2] + (w[1][2] - w[0][2]) * t,
                ])?;
            }
            let lift = half_height + 2.0 * SKIN;
            let hit = world.trace(
                &pawn,
                [w[0][0], w[0][1], w[0][2] + lift],
                [w[1][0], w[1][1], w[1][2] + lift],
            );
            if hit.blocked() || hit.start_solid {
                return Err(NavError::CollisionUnsupported(
                    "blocked or start-solid pawn sweep",
                ));
            }
        }
        Ok(())
    }
}

/// A point placed on a mesh.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    pub mesh: usize,
    pub triangle: u32,
    /// The point at the triangle's floor height.
    pub on_floor: V3,
}

/// The meshes of a map (one per level that has one), each with a name for reports.
#[derive(Debug, Clone, PartialEq)]
pub struct NavMap {
    meshes: Vec<(String, NavMesh)>,
}

fn v(p: [f32; 3]) -> V3 {
    [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]
}

fn dist(a: V3, b: V3) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    sqrt_f64(d[0] * d[0] + d[1] * d[1] + d[2] * d[2])
}

/// The triangle's floor height under plan point (x, y), when the point is inside it (barycentric, a small tolerance).
fn floor_at(t: [V3; 3], x: f64, y: f64) -> Option<f64> {
    let [a, b, c] = t;
    let det = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
    if det.abs() < 1e-12 {
        return None;
    }
    let l1 = ((b[1] - c[1]) * (x - c[0]) + (c[0] - b[0]) * (y - c[1])) / det;
    let l2 = ((c[1] - a[1]) * (x - c[0]) + (a[0] - c[0]) * (y - c[1])) / det;
    let l3 = 1.0 - l1 - l2;
    let tol = -1e-6;
    (l1 >= tol && l2 >= tol && l3 >= tol).then(|| l1 * a[2] + l2 * b[2] + l3 * c[2])
}

/// An `f64` that orders with `total_cmp`, for the open set.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Cost(f64);
impl Eq for Cost {}
impl PartialOrd for Cost {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Cost {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl NavMap {
    pub fn new(meshes: Vec<(String, NavMesh)>) -> NavMap {
        NavMap { meshes }
    }

    pub fn meshes(&self) -> &[(String, NavMesh)] {
        &self.meshes
    }

    /// The meshes of every level of `scene` (each `GBXNavMesh` export of each level, in level order then export
    /// order), read with `ow_assets::navmesh::read_navmesh` from the store the scene was loaded with. A mesh that does
    /// not read is left out and its error returned beside the map.
    pub fn from_scene(
        scene: &crate::scene::Scene,
        defs: &ow_defs::DefStore,
    ) -> (NavMap, Vec<String>) {
        let mut meshes = Vec::new();
        let mut problems = Vec::new();
        for level in &scene.levels {
            let pkg = defs.packages().get(level.package);
            for index in 1..=i32::try_from(pkg.exports.len()).unwrap_or(i32::MAX) {
                let is_mesh = pkg
                    .object(index)
                    .ok()
                    .filter(|o| o.class != 0)
                    .and_then(|o| pkg.object(o.class).ok())
                    .is_some_and(|c| c.name == "GBXNavMesh");
                if !is_mesh {
                    continue;
                }
                match ow_assets::navmesh::read_navmesh(pkg, index) {
                    Ok(m) => meshes.push((level.name.clone(), m)),
                    Err(e) => problems.push(format!("{} export {index}: {e}", level.name)),
                }
            }
        }
        (NavMap { meshes }, problems)
    }

    /// The triangle range and radius and height of the run of mesh `m` fit for `radius` (module documentation).
    fn run(&self, m: usize, radius: f64) -> Option<(usize, usize, f64, f64)> {
        let mesh = &self.meshes.get(m)?.1;
        let mut starts: Vec<(usize, f64, f64)> = mesh
            .path_sizes
            .iter()
            .filter_map(|s| {
                Some((
                    usize::try_from(s.start_index).ok()?,
                    f64::from(s.radius),
                    f64::from(s.height),
                ))
            })
            .collect();
        starts.sort_by(|a, b| a.0.cmp(&b.0));
        let mut best: Option<(usize, usize, f64, f64)> = None;
        for (i, &(start, r, h)) in starts.iter().enumerate() {
            let end = starts.get(i + 1).map_or(mesh.polys.len(), |n| n.0);
            if start < end
                && end <= mesh.polys.len()
                && r >= radius
                && h > 0.0
                && best.is_none_or(|b| r < b.2)
            {
                best = Some((start, end, r, h));
            }
        }
        best
    }

    fn corners(mesh: &NavMesh, t: usize) -> Option<[V3; 3]> {
        let p = mesh.polys.get(t)?;
        let mut out = [[0.0; 3]; 3];
        for (k, c) in out.iter_mut().enumerate() {
            *c = v(*mesh.vertices.get(usize::from(p.verts[k]))?);
        }
        Some(out)
    }

    /// Places `p` on the first mesh (in map order) with a fitting run and a triangle under it.
    pub fn locate(&self, p: V3, radius: f64) -> Option<Placed> {
        if !radius.is_finite() || radius <= 0.0 || !p.iter().all(|v| v.is_finite()) {
            return None;
        }
        (0..self.meshes.len()).find_map(|m| self.locate_on(m, p, radius))
    }

    fn locate_on(&self, m: usize, p: V3, radius: f64) -> Option<Placed> {
        let (lo, hi, _, height) = self.run(m, radius)?;
        let mesh = &self.meshes[m].1;
        let g = &mesh.lookup;
        let (x_cells, y_cells) = (
            usize::try_from(g.x_cells).ok()?,
            usize::try_from(g.y_cells).ok()?,
        );
        let fx = (p[0] - f64::from(g.origin[0])) / f64::from(g.cell_size[0]);
        let fy = (p[1] - f64::from(g.origin[1])) / f64::from(g.cell_size[1]);
        if !(fx >= 0.0 && fy >= 0.0) || x_cells == 0 || y_cells == 0 {
            return None;
        }
        let (cx, cy) = (floor_index(fx, x_cells), floor_index(fy, y_cells));
        // A point exactly on the far edge belongs to the last cell; beyond it is off the grid.
        if fx > x_cells as f64 || fy > y_cells as f64 {
            return None;
        }
        let (cx, cy) = (cx.min(x_cells - 1), cy.min(y_cells - 1));
        let slot = *g
            .cell_indices
            .get(cy.checked_mul(x_cells)?.checked_add(cx)?)?;
        let cell = g.cells.get(usize::try_from(slot).ok()?)?;
        let mut best: Option<(f64, u32)> = None;
        for &t in cell {
            let ti = usize::from(t);
            if ti < lo || ti >= hi {
                continue;
            }
            let Some(z) = Self::corners(mesh, ti).and_then(|c| floor_at(c, p[0], p[1])) else {
                continue;
            };
            if z <= p[2] + STEP_UP && p[2] - z <= height && best.is_none_or(|b| z > b.0) {
                best = Some((z, u32::from(t)));
            }
        }
        best.map(|(z, t)| Placed {
            mesh: m,
            triangle: t,
            on_floor: [p[0], p[1], z],
        })
    }

    fn centroid(mesh: &NavMesh, t: usize) -> V3 {
        Self::corners(mesh, t).map_or([0.0; 3], |[a, b, c]| {
            [
                (a[0] + b[0] + c[0]) / 3.0,
                (a[1] + b[1] + c[1]) / 3.0,
                (a[2] + b[2] + c[2]) / 3.0,
            ]
        })
    }

    /// The portal from triangle `a` into its neighbour `b` (the shared edge's two vertices).
    fn portal(mesh: &NavMesh, a: usize, b: usize) -> Option<(V3, V3)> {
        let p = mesh.polys.get(a)?;
        let k = p
            .neighbours
            .iter()
            .position(|&n| usize::from(n) == b && n != NONE)?;
        let va = v(*mesh.vertices.get(usize::from(p.verts[k]))?);
        let vb = v(*mesh.vertices.get(usize::from(p.verts[(k + 1) % 3]))?);
        let other = mesh.polys.get(b)?;
        if p.verts[k] == p.verts[(k + 1) % 3]
            || !(0..3).any(|j| {
                let edge = [other.verts[j], other.verts[(j + 1) % 3]];
                edge == [p.verts[k], p.verts[(k + 1) % 3]]
                    || edge == [p.verts[(k + 1) % 3], p.verts[k]]
            })
        {
            return None;
        }
        Some((va, vb))
    }

    /// A path from `start` to `end` for a pawn of `radius` (module documentation).
    pub fn find_path(&self, start: V3, end: V3, radius: f64) -> Result<NavPath, NavError> {
        if !radius.is_finite()
            || radius <= 0.0
            || !start.iter().chain(end.iter()).all(|v| v.is_finite())
        {
            return Err(NavError::InvalidInput);
        }
        if (0..self.meshes.len()).all(|m| self.run(m, radius).is_none()) {
            return Err(NavError::NoRunFits(radius));
        }
        let s = self
            .locate(start, radius)
            .ok_or(NavError::OffMesh(Which::Start, start))?;
        let e = match self.locate_on(s.mesh, end, radius) {
            Some(e) => e,
            None => {
                return Err(match self.locate(end, radius) {
                    Some(_) => NavError::NotYet(NotYet::not_read(
                        "a path across navigation meshes (ConnectedMeshes is not typed)",
                        RECORD,
                    )),
                    None => NavError::OffMesh(Which::End, end),
                });
            }
        };
        let (lo, hi, run_radius, _) = self
            .run(s.mesh, radius)
            .ok_or(NavError::NoRunFits(radius))?;
        let mesh = &self.meshes[s.mesh].1;
        let triangles = Self::search(mesh, lo, hi, s.triangle as usize, e.triangle as usize)?;
        let waypoints = Self::floor_waypoints(mesh, &triangles, s.on_floor, e.on_floor)?;
        let length = waypoints.windows(2).map(|w| dist(w[0], w[1])).sum();
        Ok(NavPath {
            mesh: s.mesh,
            run_radius,
            triangles: triangles.iter().map(|&t| t as u32).collect(),
            waypoints,
            length,
        })
    }

    /// A* over the run's triangles (module documentation).
    fn search(
        mesh: &NavMesh,
        lo: usize,
        hi: usize,
        from: usize,
        to: usize,
    ) -> Result<Vec<usize>, NavError> {
        let usable = |t: usize| {
            let p = &mesh.polys[t];
            Self::corners(mesh, t).is_some_and(|c| {
                floor_at(c, Self::centroid(mesh, t)[0], Self::centroid(mesh, t)[1]).is_some()
            }) && (p.poly_data == NONE
                || mesh
                    .poly_data
                    .get(usize::from(p.poly_data))
                    .is_some_and(|d| d.obstacles.iter().all(|&o| o == NONE)))
        };
        if !usable(from) || !usable(to) {
            return Err(NavError::NotYet(NotYet::not_read(
                "degenerate or obstacle-bearing navigation triangle",
                RECORD,
            )));
        }
        let goal = Self::centroid(mesh, to);
        let n = hi - lo;
        let mut cost = vec![f64::INFINITY; n];
        let mut came = vec![usize::MAX; n];
        let mut open = BinaryHeap::new();
        cost[from - lo] = 0.0;
        open.push(Reverse((
            Cost(dist(Self::centroid(mesh, from), goal)),
            from,
        )));
        while let Some(Reverse((_, t))) = open.pop() {
            if t == to {
                let mut path = vec![t];
                let mut at = t;
                while at != from {
                    at = came[at - lo];
                    path.push(at);
                }
                path.reverse();
                return Ok(path);
            }
            let here = Self::centroid(mesh, t);
            for &nb in &mesh.polys[t].neighbours {
                let u = usize::from(nb);
                if nb == NONE
                    || u < lo
                    || u >= hi
                    || !usable(u)
                    || Self::portal(mesh, t, u).is_none()
                {
                    continue;
                }
                let there = Self::centroid(mesh, u);
                let c = cost[t - lo] + dist(here, there);
                if c < cost[u - lo] {
                    cost[u - lo] = c;
                    came[u - lo] = t;
                    open.push(Reverse((Cost(c + dist(there, goal)), u)));
                }
            }
        }
        Err(NavError::NoPath)
    }

    /// Each segment belongs to one triangle, including its floor height.
    fn floor_waypoints(
        mesh: &NavMesh,
        corridor: &[usize],
        start: V3,
        end: V3,
    ) -> Result<Vec<V3>, NavError> {
        let mut out = vec![start];
        for (i, &t) in corridor.iter().enumerate() {
            out.push(Self::centroid(mesh, t));
            if let Some(&next) = corridor.get(i + 1) {
                let (a, b) = Self::portal(mesh, t, next).ok_or_else(|| {
                    NavError::NotYet(NotYet::not_read(
                        "navigation link without a shared edge",
                        RECORD,
                    ))
                })?;
                out.push([
                    (a[0] + b[0]) / 2.0,
                    (a[1] + b[1]) / 2.0,
                    (a[2] + b[2]) / 2.0,
                ]);
            }
        }
        out.push(end);
        out.dedup();
        Ok(out)
    }
}

/// The AI's movement seam answered with [`NavMap`] (module documentation).
pub struct NavLocomotion {
    map: Rc<NavMap>,
    /// Handle to (pawn, radius), registered by the host.
    handles: DetMap<ObjRef, (ObjRef, f64)>,
    /// The pawn's last path, its destination, and the stance byte last asked for.
    paths: DetMap<ObjRef, NavPath>,
    speeds: DetMap<ObjRef, u8>,
}

/// How near the path's end the pawn must be to have arrived: within the run's radius in plan and the run's height
/// vertically. A host choice.
fn arrived(path: &NavPath, at: V3) -> bool {
    let Some(end) = path.waypoints.last() else {
        return false;
    };
    let (dx, dy) = (at[0] - end[0], at[1] - end[1]);
    let plan = sqrt_f64(dx * dx + dy * dy);
    plan <= path.run_radius && (at[2] - end[2]).abs() <= 220.0
}

impl NavLocomotion {
    pub fn new(map: Rc<NavMap>) -> NavLocomotion {
        NavLocomotion {
            map,
            handles: DetMap::new(),
            paths: DetMap::new(),
            speeds: DetMap::new(),
        }
    }

    /// Says that navigation handle `handle` moves `pawn`, a pawn of collision radius `radius` uu.
    pub fn register(&mut self, handle: ObjRef, pawn: ObjRef, radius: f64) {
        self.handles.insert(handle, (pawn, radius));
    }

    /// The pawn's current path, for the host to follow.
    pub fn path_of(&self, pawn: ObjRef) -> Option<&NavPath> {
        self.paths.get(&pawn)
    }

    /// The stance byte last set for the pawn.
    pub fn speed_of(&self, pawn: ObjRef) -> Option<u8> {
        self.speeds.get(&pawn).copied()
    }

    fn radius_of(&self, pawn: ObjRef) -> Result<f64, VmError> {
        self.handles
            .values()
            .find(|(p, _)| *p == pawn)
            .map(|(_, r)| *r)
            .ok_or_else(|| {
                unregistered("the radius of a pawn no navigation handle was registered for")
            })
    }
}

fn unregistered(what: &'static str) -> VmError {
    NotYet::layer_absent(
        what,
        "the host's registration (nav::NavLocomotion::register)",
        RECORD,
    )
    .into()
}

/// The pawn's `Location`, from the VM's heap.
fn location(vm: &mut Vm, pawn: ObjRef) -> Result<V3, VmError> {
    use ow_combat::hit::access::{field, get, number};
    let missing = |what: String| VmError::Script {
        function: "nav".to_string(),
        message: what,
    };
    let loc = get(vm, pawn, "Location")?
        .ok_or_else(|| missing("the pawn declares no Location".to_string()))?;
    let mut out = [0.0; 3];
    for (slot, n) in out.iter_mut().zip(["X", "Y", "Z"]) {
        *slot = field(vm, &loc, n)
            .map(|x| number(&x))
            .ok_or_else(|| missing(format!("the pawn's Location has no {n}")))?;
    }
    Ok(out)
}

impl Locomotion for NavLocomotion {
    fn create_path(&mut self, vm: &mut Vm, handle: ObjRef, dest: Vec3) -> Result<bool, VmError> {
        host_only(vm)?;
        let &(pawn, radius) = self
            .handles
            .get(&handle)
            .ok_or_else(|| unregistered("the pawn of a navigation handle"))?;
        self.paths.remove(&pawn);
        let from = location(vm, pawn)?;
        match self.map.find_path(from, v(dest), radius) {
            Ok(path) => {
                self.paths.insert(pawn, path);
                Ok(true)
            }
            Err(NavError::NotYet(n)) => Err(n.into()),
            Err(_) => {
                self.paths.remove(&pawn);
                Ok(false)
            }
        }
    }

    fn clear_path(&mut self, vm: &mut Vm, handle: ObjRef, _stop: bool) -> Result<bool, VmError> {
        host_only(vm)?;
        let &(pawn, _) = self
            .handles
            .get(&handle)
            .ok_or_else(|| unregistered("the pawn of a navigation handle"))?;
        Ok(self.paths.remove(&pawn).is_some())
    }

    fn path_is_valid(&mut self, vm: &mut Vm, pawn: ObjRef) -> Result<bool, VmError> {
        host_only(vm)?;
        Ok(self.paths.contains_key(&pawn))
    }

    fn invalidate_path(&mut self, vm: &mut Vm, pawn: ObjRef) -> Result<(), VmError> {
        host_only(vm)?;
        self.paths.remove(&pawn);
        Ok(())
    }

    fn reached_destination(&mut self, vm: &mut Vm, pawn: ObjRef) -> Result<bool, VmError> {
        host_only(vm)?;
        let Some(path) = self.paths.get(&pawn) else {
            return Ok(false);
        };
        let at = location(vm, pawn)?;
        // The pawn's location is its centre; the path is on the floor: compare the pawn's floor point.
        let Some(placed) = self.map.locate_on(path.mesh, at, path.run_radius) else {
            return Ok(false);
        };
        Ok(arrived(path, placed.on_floor))
    }

    fn can_move_to(&mut self, vm: &mut Vm, pawn: ObjRef, goal: Vec3) -> Result<bool, VmError> {
        host_only(vm)?;
        let radius = self.radius_of(pawn)?;
        let from = location(vm, pawn)?;
        match self.map.find_path(from, v(goal), radius) {
            Ok(_) => Ok(true),
            Err(NavError::NotYet(n)) => Err(n.into()),
            Err(_) => Ok(false),
        }
    }

    fn set_movement_speed(&mut self, vm: &mut Vm, pawn: ObjRef, speed: u8) -> Result<(), VmError> {
        host_only(vm)?;
        self.speeds.insert(pawn, speed);
        Ok(())
    }

    fn nearest_on_navmesh(&mut self, vm: &mut Vm, pawn: ObjRef) -> Result<Option<Vec3>, VmError> {
        host_only(vm)?;
        let radius = self.radius_of(pawn)?;
        let at = location(vm, pawn)?;
        Ok(self.map.locate(at, radius).map(|p| {
            [
                p.on_floor[0] as f32,
                p.on_floor[1] as f32,
                p.on_floor[2] as f32,
            ]
        }))
    }
}

fn host_only(vm: &Vm) -> Result<(), VmError> {
    if vm.parity == ow_core::ParityMode::Reference {
        return Err(NotYet::not_read(
            "HOST STAND-IN navigation cannot answer Reference movement",
            RECORD,
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
