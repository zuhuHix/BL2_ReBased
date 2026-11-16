//! Owner: lane H4-MOVEMENT (ow-assets addition granted by the hub, 2026-11-16). A static mesh's simple collision: the
//! convex and box elements of an `RB_BodySetup` export's tagged `AggGeom` (a `KAggregateGeom`), in the mesh's local
//! space and units (uu). The port of `tools/collision_geometry.py` `hulls()`, the reader `tools/prepare_level.py`
//! used for every Sanctuary mesh body (decision 2026-09-14-04, and 2026-10-21-02 for the sphere and capsule split).
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-52-*`.
//!
//! **What is read, and how.** The export's tags are walked from 4 bytes in (an ordinary object's prefix, as the tool
//! reads `Engine.RB_BodySetup` through `--scene-records`), with the array element types of [`COLLISION_ARRAYS`]: the
//! collision lines of `tools/level-arrays.schema`, the schema the tool passed. No offset is new: the tag walker and
//! the fixed `Vector`, `Box`, `Plane` and `Matrix` layouts are `ow_package::props`'s (the C++ reader's, observed on
//! Ash's bodies, decision 2026-09-14-04). Arrays the schema does not type (`EdgeDirections`, `FaceNormalDirections`,
//! `FacePlaneData`) stay raw and are not used, as in the tool.
//!
//! - `ConvexElems`: each element's `VertexData` (vertices) and `FaceTriData` (triangle indices, optional), checked
//!   against its `ElemBox` when one is tagged (valid, ordered, every vertex inside it within 0.1 uu).
//! - `BoxElems`: the eight corners of the box `X`, `Y`, `Z` (full sizes) placed by its `TM` (rows X, Y, Z axes, W the
//!   origin; the W column must be 0, 0, 0, 1 within 1e-5).
//! - `SphereElems` and `SphylElems` are refused, as `hulls()` refuses them: the tool decodes them only for blocking
//!   volumes, and the capsule's axis convention is UNVERIFIED (decision 2026-10-21-02).
//! - An element with `bNoRBCollision` or `bPerPolyShape` set, an untyped aggregate field, a hull without volume, a
//!   non-finite value or more than 4,096 hulls refuses the whole body, never part of it.
//!
//! **Stricter than `hulls()`, on purpose** (each refuses the body where the tool reads on):
//! - a field tagged twice in one struct is refused; the tool's `fields()` keeps the last one;
//! - an `ElemBox` that is tagged but has no `Min`/`Max` is refused; the tool skips an empty (falsy) `ElemBox`;
//! - an `AggGeom` tag that did not decode as a struct is refused; the tool sees only decoded tags, so for it the body
//!   simply has no aggregate and no hulls;
//!
//! **Different from `hulls()` both ways:** an aggregate field outside the four shape lists is accepted only when it is
//! `false`, `0` or byte `0`. The tool's `shapes()` iterates every aggregate field, so it accepts an empty string or
//! list (refused here) and raises an uncaught `TypeError` on `0`, `0.0`, `False` or `None` (accepted here for the
//! first three).
//!
//! None of these four cases occurs on Sanctuary (no body was refused there).
//!
//! Which of these shapes the game's pawn movement tests against is UNVERIFIED in general. One live observation
//! supports it for Sanctuary's stair ramps: the pawn's floor normal there equals the ramp hull's
//! (`COLLISION_WALKING_VERIFICATION.md`, 2026-10-21). The cooked PhysX data and the per-polygon (kDOP) collision of a
//! mesh with no body are not read.

use std::fmt;

use ow_package::package::{Package, PackageError};
use ow_package::props::{self, PropEntry, PropError, PropValue};
use ow_package::schema::SchemaTyper;

/// The array element types the body is walked with: the collision lines of `tools/level-arrays.schema`.
pub const COLLISION_ARRAYS: &str = "ConvexElems=StructProperty:KConvexElem\n\
                                    BoxElems=StructProperty:KBoxElem\n\
                                    SphereElems=StructProperty:KSphereElem\n\
                                    SphylElems=StructProperty:KSphylElem\n\
                                    VertexData=StructProperty:Vector\n\
                                    FaceTriData=IntProperty\n";

/// Where a body's tags start: an ordinary object's prefix (`--scene-records` reads `RB_BodySetup` from 4).
pub const BODY_PROPERTY_OFFSET: usize = 4;

/// The most vertices one convex element may have, and the most hulls one body may have (`collision_geometry.py`).
pub const MAX_HULL_VERTICES: usize = 4096;
pub const MAX_BODY_HULLS: usize = 4096;

/// One convex hull in the mesh's local space (uu): its vertices and, when the element tagged `FaceTriData`, its
/// triangles as vertex indices. Box elements give eight corners and no triangles.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct ConvexHull {
    pub vertices: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

/// What a mesh body holds: its hulls in element order (every `ConvexElems` entry, then every `BoxElems` entry, in the
/// order the aggregate serialized them).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct BodyCollision {
    /// The body's object path.
    pub path: String,
    pub hulls: Vec<ConvexHull>,
}

/// Why a body could not be read. The texts are `collision_geometry.py`'s where it has the case.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollisionError {
    Package(PackageError),
    Props(PropError),
    /// The export is not an `RB_BodySetup` (its class name).
    NotBodySetup(String),
    /// "Undecoded aggregate collision field: <name>"
    UndecodedField(String),
    /// "Unsupported collision shape: <kind>"
    UnsupportedShape(String),
    /// "Unsupported collision shape flags"
    UnsupportedFlags,
    /// "Nonfinite collision vertex"
    NonFinite,
    /// "Invalid collision triangle indices"
    TriangleIndices,
    /// "Invalid collision bounds"
    Bounds,
    /// "Collision vertices outside source bounds"
    OutsideBounds,
    /// "Invalid collision box dimensions"
    BoxDimensions,
    /// "Non-affine collision transform"
    NonAffine,
    /// "Invalid convex vertex count"
    VertexCount,
    /// "Degenerate collision hull"
    Degenerate,
    /// "Too many collision hulls"
    TooManyHulls,
    /// A field has the wrong shape (the field and what was expected).
    Malformed(String),
}

impl fmt::Display for CollisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CollisionError::Package(e) => write!(f, "{e}"),
            CollisionError::Props(e) => write!(f, "{e}"),
            CollisionError::NotBodySetup(c) => write!(f, "export is {c}, not RB_BodySetup"),
            CollisionError::UndecodedField(n) => write!(f, "Undecoded aggregate collision field: {n}"),
            CollisionError::UnsupportedShape(k) => write!(f, "Unsupported collision shape: {k}"),
            CollisionError::UnsupportedFlags => f.write_str("Unsupported collision shape flags"),
            CollisionError::NonFinite => f.write_str("Nonfinite collision vertex"),
            CollisionError::TriangleIndices => f.write_str("Invalid collision triangle indices"),
            CollisionError::Bounds => f.write_str("Invalid collision bounds"),
            CollisionError::OutsideBounds => f.write_str("Collision vertices outside source bounds"),
            CollisionError::BoxDimensions => f.write_str("Invalid collision box dimensions"),
            CollisionError::NonAffine => f.write_str("Non-affine collision transform"),
            CollisionError::VertexCount => f.write_str("Invalid convex vertex count"),
            CollisionError::Degenerate => f.write_str("Degenerate collision hull"),
            CollisionError::TooManyHulls => f.write_str("Too many collision hulls"),
            CollisionError::Malformed(what) => write!(f, "malformed collision field: {what}"),
        }
    }
}

impl std::error::Error for CollisionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CollisionError::Package(e) => Some(e),
            CollisionError::Props(e) => Some(e),
            _ => None,
        }
    }
}

impl From<PackageError> for CollisionError {
    fn from(e: PackageError) -> Self {
        CollisionError::Package(e)
    }
}

impl From<PropError> for CollisionError {
    fn from(e: PropError) -> Self {
        CollisionError::Props(e)
    }
}

/// Reads body export `index` of `package` (module documentation).
pub fn read_body_setup(package: &Package, index: i32) -> Result<BodyCollision, CollisionError> {
    let object = package.object(index)?;
    let class = if object.class == 0 {
        String::new()
    } else {
        package.object(object.class)?.name.clone()
    };
    if class != "RB_BodySetup" {
        return Err(CollisionError::NotBodySetup(class));
    }
    let mut typer = SchemaTyper::new_empty();
    typer
        .add_text(COLLISION_ARRAYS)
        .map_err(|e| CollisionError::Malformed(format!("schema: {e}")))?;
    let tagged = props::read_tagged(package, index, BODY_PROPERTY_OFFSET, &typer)?;
    let hulls = match tagged.entries.iter().find(|e| e.name == "AggGeom") {
        None => Vec::new(),
        Some(entry) => aggregate(&entry.value)?,
    };
    Ok(BodyCollision {
        path: package.path_of(index)?,
        hulls,
    })
}

/// The fields of a struct value, or an error naming `what`.
fn fields<'v>(value: &'v PropValue, what: &str) -> Result<&'v [PropEntry], CollisionError> {
    match value {
        PropValue::Struct { fields, .. } => Ok(fields),
        _ => Err(CollisionError::Malformed(format!("{what} is not a struct"))),
    }
}

/// Field `name` of `entries`, the first one tagged (the tool's `fields()` keeps the last of a repeated name; no
/// collision struct repeats one, and a repeat is refused here rather than resolved).
fn field<'v>(entries: &'v [PropEntry], name: &str) -> Result<Option<&'v PropValue>, CollisionError> {
    let mut found = entries.iter().filter(|e| e.name == name);
    let first = found.next().map(|e| &e.value);
    if found.next().is_some() {
        return Err(CollisionError::Malformed(format!("{name} tagged twice")));
    }
    Ok(first)
}

fn float(value: &PropValue, what: &str) -> Result<f64, CollisionError> {
    match value {
        PropValue::Float(v) if v.is_finite() => Ok(f64::from(*v)),
        PropValue::Float(_) => Err(CollisionError::NonFinite),
        _ => Err(CollisionError::Malformed(format!("{what} is not a float"))),
    }
}

fn flag(entries: &[PropEntry], name: &str) -> Result<bool, CollisionError> {
    match field(entries, name)? {
        None => Ok(false),
        Some(PropValue::Bool(b)) => Ok(*b),
        Some(_) => Err(CollisionError::Malformed(format!("{name} is not a bool"))),
    }
}

/// A `Vector` (or a `Plane`'s X, Y, Z) as a point.
fn point(value: &PropValue, what: &str) -> Result<[f64; 3], CollisionError> {
    let entries = fields(value, what)?;
    let mut out = [0.0; 3];
    for (slot, axis) in out.iter_mut().zip(["X", "Y", "Z"]) {
        let v = field(entries, axis)?.ok_or_else(|| CollisionError::Malformed(format!("{what}.{axis} missing")))?;
        *slot = float(v, what)?;
    }
    Ok(out)
}

/// A `Matrix`'s X, Y, Z axis rows and its origin row, refused when the W column is not (0, 0, 0, 1) within 1e-5
/// (`collision_geometry.py` `affine_rows`).
fn affine_rows(value: &PropValue) -> Result<[[f64; 3]; 4], CollisionError> {
    let entries = fields(value, "TM")?;
    let mut rows = [[0.0; 3]; 4];
    for (i, plane) in ["XPlane", "YPlane", "ZPlane", "WPlane"].into_iter().enumerate() {
        let p = field(entries, plane)?.ok_or_else(|| CollisionError::Malformed(format!("TM.{plane} missing")))?;
        rows[i] = point(p, plane)?;
        let w = field(fields(p, plane)?, "W")?
            .ok_or_else(|| CollisionError::Malformed(format!("TM.{plane}.W missing")))?;
        let w = float(w, plane)?;
        let expected = if i == 3 { 1.0 } else { 0.0 };
        if (w - expected).abs() > 1e-5 {
            return Err(CollisionError::NonAffine);
        }
    }
    Ok(rows)
}

/// `collision_geometry.py` `shapes()` then `hulls()`: the aggregate's convex and box elements.
fn aggregate(value: &PropValue) -> Result<Vec<ConvexHull>, CollisionError> {
    let entries = fields(value, "AggGeom")?;
    let mut hulls = Vec::new();
    for entry in entries {
        if matches!(entry.value, PropValue::Raw(_)) {
            return Err(CollisionError::UndecodedField(entry.name.clone()));
        }
    }
    for entry in entries {
        let kind = entry.name.as_str();
        let elements: &[PropValue] = match &entry.value {
            PropValue::Array(items) => items,
            // The tool skips a field that is not one of the four shape lists only when it is empty (falsy).
            PropValue::Bool(false) | PropValue::Int(0) | PropValue::Byte(0) => &[],
            _ => return Err(CollisionError::UnsupportedShape(kind.to_string())),
        };
        if elements.is_empty() {
            continue;
        }
        match kind {
            "ConvexElems" => {
                for e in elements {
                    hulls.push(convex(e)?);
                }
            }
            "BoxElems" => {
                for e in elements {
                    hulls.push(box_element(e)?);
                }
            }
            // `hulls()` refuses spheres and capsules (module documentation), as it refuses any other kind.
            _ => return Err(CollisionError::UnsupportedShape(kind.to_string())),
        }
        if hulls.len() > MAX_BODY_HULLS {
            return Err(CollisionError::TooManyHulls);
        }
    }
    Ok(hulls)
}

fn shape_flags(entries: &[PropEntry]) -> Result<(), CollisionError> {
    if flag(entries, "bNoRBCollision")? || flag(entries, "bPerPolyShape")? {
        return Err(CollisionError::UnsupportedFlags);
    }
    Ok(())
}

fn convex(value: &PropValue) -> Result<ConvexHull, CollisionError> {
    let entries = fields(value, "KConvexElem")?;
    shape_flags(entries)?;
    let mut vertices = Vec::new();
    match field(entries, "VertexData")? {
        None => {}
        Some(PropValue::Array(items)) => {
            for v in items {
                vertices.push(point(v, "VertexData")?);
            }
        }
        Some(_) => return Err(CollisionError::Malformed("VertexData is not a vector array".into())),
    }
    let mut indices = Vec::new();
    match field(entries, "FaceTriData")? {
        None => {}
        Some(PropValue::Array(items)) => {
            for i in items {
                match i {
                    PropValue::Int(n) => indices.push(*n),
                    _ => return Err(CollisionError::TriangleIndices),
                }
            }
        }
        Some(_) => return Err(CollisionError::TriangleIndices),
    }
    let count = vertices.len();
    if indices.len() % 3 != 0 || indices.iter().any(|&i| usize::try_from(i).map_or(true, |i| i >= count)) {
        return Err(CollisionError::TriangleIndices);
    }
    if let Some(bounds) = field(entries, "ElemBox")? {
        let b = fields(bounds, "ElemBox")?;
        let lo = point(field(b, "Min")?.ok_or(CollisionError::Bounds)?, "ElemBox.Min")?;
        let hi = point(field(b, "Max")?.ok_or(CollisionError::Bounds)?, "ElemBox.Max")?;
        let valid = matches!(field(b, "IsValid")?, Some(PropValue::Byte(1)));
        if !valid || (0..3).any(|i| lo[i] > hi[i]) {
            return Err(CollisionError::Bounds);
        }
        if vertices.iter().any(|v| (0..3).any(|i| v[i] < lo[i] - 0.1 || v[i] > hi[i] + 0.1)) {
            return Err(CollisionError::OutsideBounds);
        }
    }
    volume_check(&vertices)?;
    // Every index was checked to be at least 0 and below the vertex count (at most 4,096), so each fits a u32.
    let index = |i: i32| u32::try_from(i).map_err(|_| CollisionError::TriangleIndices);
    let mut triangles = Vec::with_capacity(indices.len() / 3);
    for t in indices.chunks_exact(3) {
        triangles.push([index(t[0])?, index(t[1])?, index(t[2])?]);
    }
    Ok(ConvexHull { vertices, triangles })
}

fn box_element(value: &PropValue) -> Result<ConvexHull, CollisionError> {
    let entries = fields(value, "KBoxElem")?;
    shape_flags(entries)?;
    let mut sizes = [0.0; 3];
    for (slot, axis) in sizes.iter_mut().zip(["X", "Y", "Z"]) {
        let v = field(entries, axis)?.ok_or(CollisionError::BoxDimensions)?;
        let v = float(v, axis).map_err(|_| CollisionError::BoxDimensions)?;
        if v <= 0.0 {
            return Err(CollisionError::BoxDimensions);
        }
        *slot = v;
    }
    let tm = field(entries, "TM")?.ok_or_else(|| CollisionError::Malformed("KBoxElem.TM missing".into()))?;
    let rows = affine_rows(tm)?;
    // itertools.product((-1, 1), repeat=3): the last axis changes fastest.
    let mut vertices = Vec::with_capacity(8);
    for corner in 0..8u32 {
        let signs = [
            if corner & 4 == 0 { -1.0 } else { 1.0 },
            if corner & 2 == 0 { -1.0 } else { 1.0 },
            if corner & 1 == 0 { -1.0 } else { 1.0 },
        ];
        let mut v = rows[3];
        for (i, cell) in v.iter_mut().enumerate() {
            for j in 0..3 {
                *cell += signs[j] * sizes[j] * 0.5 * rows[j][i];
            }
        }
        vertices.push(v);
    }
    volume_check(&vertices)?;
    Ok(ConvexHull {
        vertices,
        triangles: Vec::new(),
    })
}

/// `collision_geometry.py` `convex_volume_check`: 4 to 4,096 vertices that are not all on one plane. The roots of
/// the tool's normalisation are avoided: the test "some edge leaves the plane by more than 1e-5" is made on the
/// unnormalised normal, scaled by its length, which is the same comparison.
fn volume_check(vertices: &[[f64; 3]]) -> Result<(), CollisionError> {
    if !(4..=MAX_HULL_VERTICES).contains(&vertices.len()) {
        return Err(CollisionError::VertexCount);
    }
    let base = vertices[0];
    let edges: Vec<[f64; 3]> = vertices[1..]
        .iter()
        .map(|v| [v[0] - base[0], v[1] - base[1], v[2] - base[2]])
        .collect();
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let Some(edge) = edges.iter().copied().find(|e| dot(*e, *e) > 1e-12) else {
        return Err(CollisionError::Degenerate);
    };
    for e in &edges {
        let n = [
            edge[1] * e[2] - edge[2] * e[1],
            edge[2] * e[0] - edge[0] * e[2],
            edge[0] * e[1] - edge[1] * e[0],
        ];
        let squared = dot(n, n);
        // |n| > 1e-8, compared squared.
        if squared > 1e-16 {
            // |n . x| / |n| > 1e-5  <=>  (n . x)^2 > 1e-10 |n|^2.
            if edges.iter().any(|x| {
                let d = dot(n, *x);
                d * d > 1e-10 * squared
            }) {
                return Ok(());
            }
            return Err(CollisionError::Degenerate);
        }
    }
    Err(CollisionError::Degenerate)
}

#[cfg(test)]
mod tests;
