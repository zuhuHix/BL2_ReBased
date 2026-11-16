//! Owner: lane H4-MOVEMENT. The collision world: convex brushes (planes) and a swept-shape trace against them.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-52-*`.
//!
//! **Our own code, a host stand-in.** How the game sweeps its pawn against a convex hull was not read (the walking
//! step and floor finding are "Not read yet" in `NATIVE_PLAYER_MOVEMENT.md`). This is the plane-shifting sweep of
//! the public Quake-family engines, written here from the idea, not from any code: a convex solid is the set of
//! points behind all of its planes; sweeping a convex shape against it is sweeping a point against the solid grown
//! by the shape, and the grown solid is approximated by moving every plane out by the shape's support distance along
//! its normal. The approximation is exact on faces and blocks a little early near edges and corners, by at most
//! `r (1 / cos(a / 2) - 1)` for a radius `r` at a corner of angle `a` between two faces' normals (17 uu at a right
//! angle for radius 42), less where an axis-aligned bevel plane bounds it: every brush also carries the six planes of
//! its bounding box. UNVERIFIED as the game's contact rule.
//!
//! A triangle is a brush too: a thin solid of two opposed face planes and three edge planes, so terrain, BSP and
//! triangle-mesh collision can enter the same world once they are decoded (none is in this lane).
//!
//! The trace stops [`SKIN`] uu short of the surface along its normal, so a resting shape never starts the next trace
//! inside the plane it rests on (a numeric margin, our choice, UNVERIFIED as any game value). A trace that starts
//! inside a brush and ends outside it ignores that brush (it may leave), and one that stays inside is blocked at
//! once; both report [`Hit::start_solid`].
//!
//! **Order and determinism.** Brushes keep the order they were added in; a trace visits candidate brushes in
//! ascending index (the grid's candidates sorted), and the nearest hit wins, the lower index on an exact tie. The
//! same brushes and the same trace give the same bits.

use super::geom::{V3, add, cross, dot, floor_index, normalize, scale, sub};
use ow_core::num::sqrt_f64;

/// How far short of a surface a trace stops, uu, along the surface normal (module documentation).
pub const SKIN: f64 = 0.03125;

/// Side of a grid cell, uu.
pub const CELL: f64 = 512.0;

/// The most grid cells a world may have; a larger extent widens the cells instead.
pub const MAX_CELLS: usize = 1 << 20;

/// How far from the origin, uu, a brush's points may lie on any axis (our bound, not a game value: Sanctuary's
/// farthest brush point is about 253,000 uu out). A hull or triangle beyond it is refused
/// ([`BrushError::OutOfBounds`]), and [`CollisionWorld::new`] leaves out of its grid any brush built by hand beyond
/// it, so the grid's size stays bounded.
pub const WORLD_BOUND: f64 = 1_048_576.0;

/// True when every coordinate of `p` is finite and within [`WORLD_BOUND`].
fn in_bound(p: V3) -> bool {
    p.iter().all(|v| v.is_finite() && v.abs() <= WORLD_BOUND)
}

/// The shape that moves, centred on its location. Z is the vertical axis; neither shape turns.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// A vertical cylinder of `radius` and `half_height` (the pawn's `CylinderComponent`: `CollisionRadius` and
    /// `CollisionHeight`, which is a half height).
    Cylinder { radius: f64, half_height: f64 },
    /// A vertical capsule of `radius` whose total half height, hemispheres included, is `half_height`.
    Capsule { radius: f64, half_height: f64 },
}

impl Shape {
    /// The support distance along unit normal `n`: how far the shape reaches from its centre in that direction.
    pub fn support(&self, n: V3) -> f64 {
        match *self {
            Shape::Cylinder { radius, half_height } => {
                radius * sqrt_f64(n[0] * n[0] + n[1] * n[1]) + half_height * n[2].abs()
            }
            Shape::Capsule { radius, half_height } => radius + (half_height - radius).max(0.0) * n[2].abs(),
        }
    }

    /// Half the shape's bounding box.
    pub fn extent(&self) -> V3 {
        match *self {
            Shape::Cylinder { radius, half_height } | Shape::Capsule { radius, half_height } => {
                [radius, radius, half_height]
            }
        }
    }

    pub fn half_height(&self) -> f64 {
        self.extent()[2]
    }
}

/// A plane: unit normal `n` and distance `d`; a point `p` is behind it (inside) when `n . p <= d`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    pub n: V3,
    pub d: f64,
}

/// A convex solid: the points behind every plane. `lo` and `hi` bound it. `source` is the caller's tag (the census
/// entry the brush came from).
#[derive(Debug, Clone, PartialEq)]
pub struct Brush {
    pub planes: Vec<Plane>,
    pub lo: V3,
    pub hi: V3,
    pub source: u32,
}

/// Why points did not make a brush.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BrushError {
    /// Fewer than four points, or a non-finite one.
    TooFewPoints,
    /// No face plane found: the points are flat or collinear.
    Flat,
    /// More points than the face search takes without triangles ([`MAX_SEARCH_POINTS`]).
    TooManyPointsWithoutFaces,
    /// A degenerate triangle (no area).
    DegenerateTriangle,
    /// A point beyond [`WORLD_BOUND`].
    OutOfBounds,
}

impl std::fmt::Display for BrushError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            BrushError::TooFewPoints => "fewer than four finite points",
            BrushError::Flat => "no face plane (flat or collinear points)",
            BrushError::TooManyPointsWithoutFaces => "too many points to search for faces without triangles",
            BrushError::DegenerateTriangle => "degenerate triangle",
            BrushError::OutOfBounds => "a point beyond the world bound",
        })
    }
}

/// Without triangles, the faces are searched over every point triple: at most this many points.
pub const MAX_SEARCH_POINTS: usize = 96;

fn bounds(points: &[V3]) -> (V3, V3) {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in points {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    (lo, hi)
}

/// Adds `p` unless a plane with the same normal and distance is there.
fn push_unique(planes: &mut Vec<Plane>, p: Plane, tolerance: f64) {
    if !planes
        .iter()
        .any(|q| dot(q.n, p.n) > 1.0 - 1e-9 && (q.d - p.d).abs() <= tolerance)
    {
        planes.push(p);
    }
}

/// The six planes of the box `lo`..`hi`.
fn bevels(planes: &mut Vec<Plane>, lo: V3, hi: V3, tolerance: f64) {
    for axis in 0..3 {
        let mut n = [0.0; 3];
        n[axis] = 1.0;
        push_unique(planes, Plane { n, d: hi[axis] }, tolerance);
        n[axis] = -1.0;
        push_unique(planes, Plane { n, d: -lo[axis] }, tolerance);
    }
}

impl Brush {
    /// The convex hull of `points` (world space). Its face planes come from `triangles` (vertex indices) when given,
    /// each kept only when every point is behind it; otherwise from every point triple whose plane has all points on
    /// one side. Normals point away from the points' centroid.
    pub fn from_hull(points: &[V3], triangles: &[[u32; 3]], source: u32) -> Result<Brush, BrushError> {
        if points.len() < 4 || points.iter().any(|p| !super::geom::is_finite(*p)) {
            return Err(BrushError::TooFewPoints);
        }
        if !points.iter().all(|p| in_bound(*p)) {
            return Err(BrushError::OutOfBounds);
        }
        let (lo, hi) = bounds(points);
        let size = (0..3).map(|i| hi[i] - lo[i]).fold(0.0, f64::max);
        let tolerance = 1e-4 * (1.0 + size);
        let count = points.len() as f64;
        let centroid = scale(points.iter().fold([0.0; 3], |a, p| add(a, *p)), 1.0 / count);
        let mut planes = Vec::new();
        let candidate = |a: V3, b: V3, c: V3, planes: &mut Vec<Plane>| {
            let Some(mut n) = normalize(cross(sub(b, a), sub(c, a))) else {
                return;
            };
            let mut d = dot(n, a);
            if dot(n, centroid) > d {
                n = scale(n, -1.0);
                d = -d;
            }
            if points.iter().all(|p| dot(n, *p) - d <= tolerance) {
                // The plane through the outermost point, so no point is left outside by the tolerance.
                let d = points.iter().map(|p| dot(n, *p)).fold(d, f64::max);
                push_unique(planes, Plane { n, d }, tolerance);
            }
        };
        let in_range = |t: &[u32; 3]| t.iter().all(|&i| (i as usize) < points.len());
        if !triangles.is_empty() && triangles.iter().all(in_range) {
            for t in triangles {
                candidate(points[t[0] as usize], points[t[1] as usize], points[t[2] as usize], &mut planes);
            }
        } else {
            if points.len() > MAX_SEARCH_POINTS {
                return Err(BrushError::TooManyPointsWithoutFaces);
            }
            for i in 0..points.len() {
                for j in i + 1..points.len() {
                    for k in j + 1..points.len() {
                        candidate(points[i], points[j], points[k], &mut planes);
                    }
                }
            }
        }
        // A hull's faces enclose it in every direction; fewer than four face planes is a flat set of points.
        if planes.len() < 4 {
            return Err(BrushError::Flat);
        }
        bevels(&mut planes, lo, hi, tolerance);
        Ok(Brush { planes, lo, hi, source })
    }

    /// A triangle as a thin solid: its plane both ways and the three planes through its edges, perpendicular to it.
    pub fn from_triangle(a: V3, b: V3, c: V3, source: u32) -> Result<Brush, BrushError> {
        if ![a, b, c].iter().all(|p| in_bound(*p)) {
            return Err(BrushError::OutOfBounds);
        }
        let Some(n) = normalize(cross(sub(b, a), sub(c, a))) else {
            return Err(BrushError::DegenerateTriangle);
        };
        let points = [a, b, c];
        let (lo, hi) = bounds(&points);
        let size = (0..3).map(|i| hi[i] - lo[i]).fold(0.0, f64::max);
        let tolerance = 1e-6 * (1.0 + size);
        let d = dot(n, a);
        let mut planes = vec![Plane { n, d }, Plane { n: scale(n, -1.0), d: -d }];
        for (p, q, other) in [(a, b, c), (b, c, a), (c, a, b)] {
            let Some(mut m) = normalize(cross(sub(q, p), n)) else {
                return Err(BrushError::DegenerateTriangle);
            };
            if dot(m, sub(other, p)) > 0.0 {
                m = scale(m, -1.0);
            }
            push_unique(&mut planes, Plane { n: m, d: dot(m, p) }, tolerance);
        }
        bevels(&mut planes, lo, hi, tolerance);
        Ok(Brush { planes, lo, hi, source })
    }
}

/// The result of a trace.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    /// How far along the move the shape got, 0 to 1 (1: no hit).
    pub fraction: f64,
    /// Where the shape's centre ends.
    pub end: V3,
    /// The normal of the plane hit (zero when nothing was hit).
    pub normal: V3,
    /// The index of the brush hit.
    pub brush: Option<u32>,
    /// The start was inside some brush (module documentation).
    pub start_solid: bool,
}

impl Hit {
    pub fn blocked(&self) -> bool {
        self.brush.is_some()
    }
}

/// The brushes and a uniform grid over them in X and Y.
#[derive(Debug, Clone, PartialEq)]
pub struct CollisionWorld {
    brushes: Vec<Brush>,
    origin: [f64; 2],
    cell: f64,
    columns: usize,
    rows: usize,
    cells: Vec<Vec<u32>>,
}

impl CollisionWorld {
    /// A world of `brushes`, in that order. A brush whose bounds are not finite or lie beyond [`WORLD_BOUND`] is kept
    /// in [`CollisionWorld::brushes`] but left out of the grid, so no trace ever hits it (the constructors refuse
    /// such brushes; only one built by hand can be one).
    pub fn new(brushes: Vec<Brush>) -> CollisionWorld {
        let gridded = |b: &Brush| in_bound(b.lo) && in_bound(b.hi);
        let (mut lo, mut hi) = ([0.0f64; 2], [0.0f64; 2]);
        if let Some(first) = brushes.iter().find(|b| gridded(b)) {
            lo = [first.lo[0], first.lo[1]];
            hi = [first.hi[0], first.hi[1]];
        }
        for b in brushes.iter().filter(|b| gridded(b)) {
            for i in 0..2 {
                lo[i] = lo[i].min(b.lo[i]);
                hi[i] = hi[i].max(b.hi[i]);
            }
        }
        let mut cell = CELL;
        // Both extents are at most twice the world bound, so each count is far below `MAX_CELLS` after a few
        // doublings of the cell; `floor_index` caps it anyway.
        let span = |c: f64| -> (usize, usize) {
            let cols = floor_index((hi[0] - lo[0]) / c, MAX_CELLS).saturating_add(1);
            let rows = floor_index((hi[1] - lo[1]) / c, MAX_CELLS).saturating_add(1);
            (cols, rows)
        };
        let (mut columns, mut rows) = span(cell);
        while columns.saturating_mul(rows) > MAX_CELLS {
            cell *= 2.0;
            (columns, rows) = span(cell);
        }
        let mut world = CollisionWorld {
            brushes,
            origin: lo,
            cell,
            columns,
            rows,
            cells: vec![Vec::new(); columns * rows],
        };
        for index in 0..world.brushes.len() {
            if !gridded(&world.brushes[index]) {
                continue;
            }
            let (lo, hi) = (world.brushes[index].lo, world.brushes[index].hi);
            let ((c0, r0), (c1, r1)) = (world.cell_of(lo), world.cell_of(hi));
            for r in r0..=r1 {
                for c in c0..=c1 {
                    let slot = r * world.columns + c;
                    // `index` is below `brushes.len()`, which the caller built in memory: it fits a u32 unless the
                    // world holds four billion brushes, which no level does; such a brush is left out of the grid.
                    if let Ok(i) = u32::try_from(index) {
                        world.cells[slot].push(i);
                    }
                }
            }
        }
        world
    }

    pub fn brushes(&self) -> &[Brush] {
        &self.brushes
    }

    /// The grid cell holding X, Y of `p`, clamped into the grid.
    fn cell_of(&self, p: V3) -> (usize, usize) {
        let index = |v: f64, o: f64, n: usize| -> usize { floor_index((v - o) / self.cell, n.saturating_sub(1)) };
        (index(p[0], self.origin[0], self.columns), index(p[1], self.origin[1], self.rows))
    }

    /// The brushes whose cells the box `lo`..`hi` touches, ascending, each once.
    fn candidates(&self, lo: V3, hi: V3) -> Vec<u32> {
        let ((c0, r0), (c1, r1)) = (self.cell_of(lo), self.cell_of(hi));
        let mut out = Vec::new();
        for r in r0..=r1 {
            for c in c0..=c1 {
                out.extend_from_slice(&self.cells[r * self.columns + c]);
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Sweeps `shape` from `start` to `end`.
    pub fn trace(&self, shape: &Shape, start: V3, end: V3) -> Hit {
        let e = shape.extent();
        let mut lo = [0.0; 3];
        let mut hi = [0.0; 3];
        for i in 0..3 {
            lo[i] = start[i].min(end[i]) - e[i] - 1.0;
            hi[i] = start[i].max(end[i]) + e[i] + 1.0;
        }
        let mut best = Hit { fraction: 1.0, end, normal: [0.0; 3], brush: None, start_solid: false };
        for index in self.candidates(lo, hi) {
            let brush = &self.brushes[index as usize];
            if (0..3).any(|i| brush.hi[i] < lo[i] || brush.lo[i] > hi[i]) {
                continue;
            }
            match clip(brush, shape, start, end) {
                Clip::Miss => {}
                Clip::StartSolid { escapes } => {
                    best.start_solid = true;
                    if !escapes {
                        return Hit { fraction: 0.0, end: start, normal: [0.0; 3], brush: Some(index), start_solid: true };
                    }
                }
                Clip::Hit { fraction, normal } => {
                    if fraction < best.fraction {
                        best.fraction = fraction;
                        best.normal = normal;
                        best.brush = Some(index);
                    }
                }
            }
        }
        if best.brush.is_some() {
            best.end = add(start, scale(sub(end, start), best.fraction));
        }
        best
    }
}

enum Clip {
    Miss,
    StartSolid { escapes: bool },
    Hit { fraction: f64, normal: V3 },
}

/// The segment `start`..`end` against `brush` grown by `shape` (module documentation).
fn clip(brush: &Brush, shape: &Shape, start: V3, end: V3) -> Clip {
    let mut enter = -1.0f64;
    let mut leave = 1.0f64;
    let mut normal = [0.0; 3];
    let mut starts_out = false;
    let mut ends_out = false;
    for p in &brush.planes {
        let d = p.d + shape.support(p.n);
        let d1 = dot(p.n, start) - d;
        let d2 = dot(p.n, end) - d;
        if d1 > 0.0 {
            starts_out = true;
        }
        if d2 > 0.0 {
            ends_out = true;
        }
        // Wholly in front of this plane: the segment cannot touch the solid. A move that keeps at least half the skin
        // away counts as in front, so a slide along a plane at the skin's distance (a ramp, a wall) is not taken
        // for an entry by the rounding of a parallel move.
        if d1 > 0.0 && (d2 >= 0.5 * SKIN || d2 >= d1) {
            return Clip::Miss;
        }
        if d1 <= 0.0 && d2 <= 0.0 {
            continue;
        }
        if d1 > d2 {
            // Clamped to 0 here, before the comparison: a start under the skin from the plane gives a negative
            // fraction, below -1 for a short move, which would otherwise read as no entry and let the move end inside
            // the brush (review r1 of PR #349).
            let f = ((d1 - SKIN) / (d1 - d2)).max(0.0);
            if f > enter {
                enter = f;
                normal = p.n;
            }
        } else {
            let f = (d1 + SKIN) / (d1 - d2);
            if f < leave {
                leave = f;
            }
        }
    }
    if !starts_out {
        return Clip::StartSolid { escapes: ends_out };
    }
    if enter < leave && enter >= 0.0 {
        return Clip::Hit { fraction: enter, normal };
    }
    Clip::Miss
}
