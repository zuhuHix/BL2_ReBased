//! Owner: lane H4-MOVEMENT. A loaded scene's collision: every static mesh placement whose component blocks, with the
//! convex and box hulls of its mesh's `RB_BodySetup` placed in the world, plus a census of what was read and what is
//! not yet.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-52-*`.
//!
//! - **Which placements collide.** Every placement of [`Scene::placements`], hidden ones included (a hidden helper
//!   keeps its collision, as `tools/prepare_level.py` keeps it), whose component has `CollideActors` and
//!   `BlockActors` both true. The flags are read from the definition store's object, so a flag the component does
//!   not tag takes the class default; `prepare_level.py` reads only the component's own tags and takes an untagged
//!   flag as true, so the two can differ (counted in [`CollisionCensus`]). The owner actor's `bCollideActors` and
//!   `bBlockActors` are not read (the tool does not read them either). UNVERIFIED as the game's collision filter.
//! - **What a placement collides with.** Its mesh's body (`ow_assets::collision::read_body_setup`), each hull's
//!   vertices through the placement's world transform (`frame::component_to_world`, or a collection entry's
//!   matrix and scale, `frame::Affine::from_ue3_matrix`), then made a [`Brush`]. An affine map keeps a hull convex.
//! - **Not yet** (each a census line, none replaced by a guess): a mesh with no body (the game's per-polygon
//!   collision of such a mesh is not read; `prepare_level.py`'s two named triangle stand-ins are not applied),
//!   terrain and BSP collision (`tools/terrain_decode.py` and `tools/bsp_decode.py` are not ported), and blocking
//!   volumes (`tools/collision_volumes.py` is not ported; their placement in the scene is not read either).

use ow_assets::collision::{BodyCollision, read_body_setup};
use ow_core::{DetMap, NotYet};
use ow_defs::DefStore;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::prefix::PropertyPrefix;
use ow_object::value::Value;

use super::collide::{Brush, CollisionWorld};
use super::geom::V3;
use crate::frame::{Affine, Placement as FramePlacement, Ue3Matrix, component_to_world};
use crate::scene::{MeshStatus, Pose, Scene};

/// The record a `NotYet` of this module names.
pub const RECORD: &str = "docs/decisions/2026-11/2026-11-16-52-host-movement-and-collision-h4.md";

/// What the collision build read and placed, and what it could not.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CollisionCensus {
    /// Every placement of the scene.
    pub placements: usize,
    /// Placements whose component has both flags true, and those with either false.
    pub blocking: usize,
    pub not_blocking: usize,
    /// Of the non-blocking placements: those whose component tags neither flag false itself (the tool would have
    /// kept them, taking an untagged flag as true).
    pub not_blocking_by_class_default: usize,
    /// Placements whose component's flags could not be read (the error text and count).
    pub flags_unreadable: DetMap<String, usize>,
    /// Blocking placements by what their mesh gave.
    pub placed_with_hulls: usize,
    pub mesh_without_body: usize,
    pub mesh_not_decoded: usize,
    pub body_refused: usize,
    pub body_empty: usize,
    pub pose_refused: usize,
    /// Distinct meshes of blocking placements: with a body that decoded, without a body, refused.
    pub meshes_with_body: usize,
    pub meshes_without_body: usize,
    pub meshes_body_refused: DetMap<String, usize>,
    /// Hulls over the distinct decoded bodies, and brushes placed in the world (one per hull per placement).
    pub body_hulls: usize,
    pub brushes: usize,
    /// Brushes by the level their placement belongs to.
    pub brushes_by_level: DetMap<String, usize>,
    /// Hulls that did not become a brush, by reason.
    pub brushes_refused: DetMap<String, usize>,
    /// The parts no collision is read for yet.
    pub not_yet: Vec<(String, u64, NotYet)>,
}

/// The collision world of a scene and its census.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct LevelCollision {
    pub world: CollisionWorld,
    pub census: CollisionCensus,
}

/// A body read once per mesh path.
enum BodyState {
    Decoded(BodyCollision),
    NoBody,
    NotDecoded,
    Refused(String),
}

/// Builds the collision of `scene`, reading component flags and bodies through `defs` and `space` (the store and
/// object space the scene was loaded with).
pub fn build(scene: &Scene, defs: &mut DefStore, space: &mut ObjectSpace) -> LevelCollision {
    let mut census = CollisionCensus::default();
    let mut bodies: DetMap<String, BodyState> = DetMap::new();
    let mut brushes = Vec::new();
    for (key, placement) in scene.placements.iter() {
        census.placements = census.placements.saturating_add(1);
        let Some(level) = scene.levels.get(key.level as usize) else {
            bump(&mut census.flags_unreadable, "no such level".into());
            continue;
        };
        let flags = defs
            .load_export(space, level.package, key.export, PropertyPrefix::Component)
            .map_err(|e| e.to_string())
            .and_then(|o| Ok((read_bool(space, o, "CollideActors")?, read_bool(space, o, "BlockActors")?)));
        let (collide, block) = match flags {
            Ok(f) => f,
            Err(e) => {
                bump(&mut census.flags_unreadable, e);
                continue;
            }
        };
        if !(collide && block) {
            census.not_blocking = census.not_blocking.saturating_add(1);
            if !own_flag_false(defs, level.package, key.export) {
                census.not_blocking_by_class_default = census.not_blocking_by_class_default.saturating_add(1);
            }
            continue;
        }
        census.blocking = census.blocking.saturating_add(1);
        if !bodies.contains_key(placement.mesh.as_str()) {
            let state = body_of(scene, defs, &placement.mesh);
            match &state {
                BodyState::Decoded(b) => {
                    census.meshes_with_body = census.meshes_with_body.saturating_add(1);
                    census.body_hulls = census.body_hulls.saturating_add(b.hulls.len());
                }
                BodyState::NoBody => census.meshes_without_body = census.meshes_without_body.saturating_add(1),
                BodyState::Refused(reason) => {
                    bump(&mut census.meshes_body_refused, reason.clone());
                }
                BodyState::NotDecoded => {}
            }
            bodies.insert(placement.mesh.clone(), state);
        }
        let body = match bodies.get(placement.mesh.as_str()) {
            Some(BodyState::Decoded(b)) => b,
            Some(BodyState::NoBody) => {
                census.mesh_without_body = census.mesh_without_body.saturating_add(1);
                continue;
            }
            Some(BodyState::Refused(_)) => {
                census.body_refused = census.body_refused.saturating_add(1);
                continue;
            }
            Some(BodyState::NotDecoded) | None => {
                census.mesh_not_decoded = census.mesh_not_decoded.saturating_add(1);
                continue;
            }
        };
        if body.hulls.is_empty() {
            census.body_empty = census.body_empty.saturating_add(1);
            continue;
        }
        let Some(affine) = world_affine(&placement.pose) else {
            census.pose_refused = census.pose_refused.saturating_add(1);
            continue;
        };
        census.placed_with_hulls = census.placed_with_hulls.saturating_add(1);
        let source = u32::try_from(census.placements - 1).unwrap_or(u32::MAX);
        for hull in &body.hulls {
            let points: Vec<V3> = hull.vertices.iter().map(|v| affine.point(*v)).collect();
            match Brush::from_hull(&points, &hull.triangles, source) {
                Ok(b) => {
                    brushes.push(b);
                    census.brushes = census.brushes.saturating_add(1);
                    bump(&mut census.brushes_by_level, level.name.clone());
                }
                Err(e) => bump(&mut census.brushes_refused, e.to_string()),
            }
        }
    }
    let terrain_components: u64 = scene.terrains.values().map(|t| u64::from(t.components)).sum();
    census.not_yet.push((
        format!("terrain collision ({} actors)", scene.terrains.len()),
        terrain_components,
        NotYet::not_ported("terrain collision (tools/terrain_decode.py)", RECORD),
    ));
    let bsp: u64 = scene.models.values().map(|m| u64::from(m.models)).sum();
    census.not_yet.push((
        "BSP collision (models)".into(),
        bsp,
        NotYet::not_ported("BSP collision (tools/bsp_decode.py)", RECORD),
    ));
    census.not_yet.push((
        "blocking volumes".into(),
        0,
        NotYet::not_ported("blocking volumes (tools/collision_volumes.py)", RECORD),
    ));
    census.not_yet.push((
        "per-polygon collision of meshes without a body (placements)".into(),
        census.mesh_without_body as u64,
        NotYet::not_read("per-polygon (kDOP) static mesh collision", RECORD),
    ));
    LevelCollision { world: CollisionWorld::new(brushes), census }
}

/// The mesh's body: resolved from its `body_setup` reference in the holder package, then read.
fn body_of(scene: &Scene, defs: &mut DefStore, mesh: &str) -> BodyState {
    let Some(m) = scene.meshes.get(mesh) else {
        return BodyState::NotDecoded;
    };
    let (MeshStatus::Decoded(asset), Some(holder)) = (&m.status, &m.holder) else {
        return BodyState::NotDecoded;
    };
    if asset.body_setup == 0 {
        return BodyState::NoBody;
    }
    let resolved = defs.code_mut().1.resolve(holder.package, asset.body_setup);
    match resolved {
        Ok(Some((package, index))) => match read_body_setup(defs.packages().get(package), index) {
            Ok(body) => BodyState::Decoded(body),
            Err(e) => BodyState::Refused(e.to_string()),
        },
        Ok(None) => BodyState::NoBody,
        Err(e) => BodyState::Refused(e.to_string()),
    }
}

/// True when the component's own tags set `CollideActors` or `BlockActors` false (`prepare_level.py` reads only
/// these). An unreadable stream counts as not set.
fn own_flag_false(defs: &DefStore, package: ow_package::PackageId, export: i32) -> bool {
    let pkg = defs.packages().get(package);
    let Ok(tagged) = ow_package::props::read_tagged(
        pkg,
        export,
        PropertyPrefix::Component.bytes(),
        &ow_package::props::NoArrayTypes,
    ) else {
        return false;
    };
    tagged.entries.iter().any(|e| {
        matches!(
            (e.name.as_str(), &e.value),
            ("CollideActors" | "BlockActors", ow_package::props::PropValue::Bool(false))
        )
    })
}

/// A placement's world transform.
fn world_affine(pose: &Pose) -> Option<Affine> {
    match pose {
        Pose::Attached { actor, component } => component_to_world(
            &FramePlacement::new(component.translation, component.rotation, component.scale, component.scale3d),
            &FramePlacement::new(actor.location, actor.rotation, actor.draw_scale, actor.draw_scale3d),
        ),
        Pose::Collection { matrix, scale3d, scale, .. } => {
            let plane = |i: usize| [matrix[i], matrix[i + 1], matrix[i + 2], matrix[i + 3]];
            let m = Ue3Matrix { x_plane: plane(0), y_plane: plane(4), z_plane: plane(8), w_plane: plane(12) };
            Affine::from_ue3_matrix(&m, [scale3d[0] * scale, scale3d[1] * scale, scale3d[2] * scale])
        }
    }
}

fn bump(map: &mut DetMap<String, usize>, key: String) {
    let n = map.entry(key).or_insert(0);
    *n = n.saturating_add(1);
}

fn read_bool(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<bool, String> {
    let class = space.heap.get(o).map_err(|e| e.to_string())?.class;
    let Some(p) = space.prop(class, name) else {
        return Err(format!("the class declares no {name}"));
    };
    match space.read(o, PropAddr::scalar(p)).map_err(|e| format!("{name}: {e}"))? {
        Value::Bool(b) => Ok(*b),
        v => Err(format!("{name} is a {}, not a bool", v.kind_name())),
    }
}
