//! Owner: lane H2-DRAW (host track, stage 2). What the renderer draws, built from a [`Scene`] with no GPU: the meshes
//! to upload (LOD 0 vertices and indices), the textures to decode, and the instance batches, one per mesh section,
//! diffuse texture, sidedness and mirroring, each holding the world matrices of its placements.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-51-*`.
//!
//! **World transforms** come from the boundary module ([`crate::frame`], lane H3): an attached placement is
//! [`frame::component_to_world`] of its component pose and its actor pose; a collection placement is
//! [`frame::Affine::from_ue3_matrix`] of the tail matrix, scaled by the kept `Scale3D` times scale. A placement whose
//! transform the boundary refuses (a non-finite value) is counted, not drawn. A mirroring transform
//! ([`frame::Affine::mirrors`]) goes to its own batch, drawn with the other front face.
//!
//! **The diffuse texture** of a material is picked by parameter name with the rule of `tools/prepare_level.py`
//! (`material_parameters`, `CHANNELS` and `ALIASES`, `channel_for_parameter`, `parameter_priority`, and the
//! channel loop of `Scene.material`): the chain's named texture parameters, a child's replacing its parent's of
//! the same name and the base material's sample-parameter expressions giving the defaults; then, among those whose
//! name maps to the diffuse channel, the first one, replaced by a later one that is not a `Common_Textures.Stub`
//! texture where the current one is, or by one of higher priority. The tool's inferences for materials with no named
//! diffuse (a sole unnamed `_Dif` sample, cooked texture resources, inspected fallbacks and recipes) are not ported:
//! such a material is drawn untextured. Two more differences: an explicit null override in a child does not hide
//! its parent's texture here (the scene records no null entries), and the tool also accepts the
//! `MaterialExpressionTextureSampleParameter2D_<n>` names, which only its unnamed inference uses.
//!
//! **Sidedness** is the base material's `TwoSided`; a material whose chain gives none (or a section with no material)
//! is drawn two-sided, so that nothing is hidden by a guess.
//!
//! Status: UNVERIFIED as the game's look. This is the B2 benchmark's level of drawing: one diffuse texture, a sun and
//! ambient light.

use ow_assets::mesh::MeshLod;
use ow_core::DetMap;

use crate::frame::{self, Affine, Mat4, Ue3Matrix};
use crate::scene::{MeshStatus, Pose, Scene, SceneMaterial};

/// A material channel the tool names (`prepare_level.py` `CHANNELS` values).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Diffuse,
    Normal,
    Specular,
    Emissive,
}

/// `prepare_level.py` `CHANNELS`, then `ALIASES`, by folded parameter name.
const CHANNEL_NAMES: [(&str, Channel); 23] = [
    ("p_diffuse", Channel::Diffuse),
    ("diffuse", Channel::Diffuse),
    ("p_normal", Channel::Normal),
    ("normal", Channel::Normal),
    ("p_specular", Channel::Specular),
    ("specular", Channel::Specular),
    ("p_emissive", Channel::Emissive),
    ("emissive", Channel::Emissive),
    ("p_dif", Channel::Diffuse),
    ("dif", Channel::Diffuse),
    ("ad_diff", Channel::Diffuse),
    ("tex_diff", Channel::Diffuse),
    ("diff_texture", Channel::Diffuse),
    ("p_snowdiffuse", Channel::Diffuse),
    ("diffuse_a", Channel::Diffuse),
    ("diffuse_b", Channel::Diffuse),
    ("diffuse_c", Channel::Diffuse),
    ("p_nrm", Channel::Normal),
    ("nrm", Channel::Normal),
    ("normalmap", Channel::Normal),
    ("tex_spec", Channel::Specular),
    ("emissive_gp", Channel::Emissive),
    ("emissiveopacityinput", Channel::Emissive),
];

/// The channel a texture parameter feeds (`prepare_level.py` `channel_for_parameter`): its name folded, looked up in
/// `CHANNELS`, then `ALIASES`.
pub fn channel_for_parameter(name: &str) -> Option<Channel> {
    let folded = name.to_lowercase();
    CHANNEL_NAMES
        .iter()
        .find(|(n, _)| *n == folded)
        .map(|&(_, c)| c)
}

/// `prepare_level.py` `parameter_priority`: a direct diffuse override over a shared master default.
pub fn parameter_priority(name: &str) -> i32 {
    match name.to_lowercase().as_str() {
        "p_diffuse" | "diffuse" => 100,
        "diffuse_a" | "diffuse_b" | "diffuse_c" | "p_dif" | "dif" | "ad_diff" | "tex_diff" => 90,
        "p_snowdiffuse" => 50,
        _ => 80,
    }
}

/// A placeholder texture the tool never prefers over a real one (`prepare_level.py`, `'common_textures.stub.'`).
fn is_stub(texture: &str) -> bool {
    texture.to_lowercase().contains("common_textures.stub.")
}

/// The diffuse texture of `material` by the tool's parameter rule (module documentation), or `None`.
pub fn pick_diffuse(material: &SceneMaterial) -> Option<&str> {
    // The tool builds its parameter dict from the deepest parent up, each level replacing the names it sets; an
    // insertion-ordered map with replace-in-place is a Python dict.
    let mut entries: Vec<_> = material
        .textures
        .iter()
        .filter(|t| t.parameter.is_some())
        .collect();
    entries.sort_by_key(|t| std::cmp::Reverse(t.depth));
    let mut parameters: Vec<(String, &str)> = Vec::new();
    for t in entries {
        let Some(name) = t.parameter.as_deref() else {
            continue;
        };
        if channel_for_parameter(name).is_none() {
            continue;
        }
        let folded = name.to_lowercase();
        match parameters.iter_mut().find(|(n, _)| *n == folded) {
            Some(slot) => slot.1 = t.path.as_str(),
            None => parameters.push((folded, t.path.as_str())),
        }
    }
    let mut picked: Option<(&str, i32, bool)> = None;
    for (name, texture) in parameters {
        if channel_for_parameter(&name) != Some(Channel::Diffuse) {
            continue;
        }
        let priority = parameter_priority(&name);
        let stub = is_stub(texture);
        let replace = match picked {
            None => true,
            Some((_, current_priority, current_stub)) => {
                (current_stub && !stub) || (current_stub == stub && priority > current_priority)
            }
        };
        if replace {
            picked = Some((texture, priority, stub));
        }
    }
    picked.map(|(t, _, _)| t)
}

/// One vertex as the renderer uploads it: position in uu, the packed normal unpacked, the first UV set.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

/// Bytes per uploaded vertex: 8 floats.
pub const VERTEX_BYTES: usize = 32;

/// A section's index range in its mesh's index buffer.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionRange {
    pub first_index: u32,
    pub index_count: u32,
}

/// A mesh's LOD 0 as the renderer uploads it.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct MeshData {
    /// The mesh's object path (a key of [`Scene::meshes`]).
    pub path: String,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    /// By section slot.
    pub sections: Vec<SectionRange>,
}

impl MeshData {
    /// LOD 0 of a decoded mesh. `None` when a section's range does not fit the index buffer (`ow-assets` checks
    /// that already; this keeps the renderer from trusting it blindly).
    pub fn from_lod(path: &str, lod: &MeshLod) -> Option<MeshData> {
        let vertices = lod
            .vertices
            .iter()
            .map(|v| Vertex {
                position: v.position,
                normal: v.normal,
                uv: v.uvs.first().copied().unwrap_or([0.0, 0.0]),
            })
            .collect();
        let mut sections = Vec::with_capacity(lod.sections.len());
        for s in &lod.sections {
            let count = s.face_count.checked_mul(3)?;
            let end = s.first_index.checked_add(count)?;
            if end as usize > lod.indices.len() {
                return None;
            }
            sections.push(SectionRange {
                first_index: s.first_index,
                index_count: count,
            });
        }
        Some(MeshData {
            path: path.to_string(),
            vertices,
            indices: lod.indices.clone(),
            sections,
        })
    }

    /// The vertices as the vertex buffer holds them: position, normal, UV, little-endian `f32`s.
    pub fn vertex_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.vertices.len() * VERTEX_BYTES);
        for v in &self.vertices {
            for f in v.position.iter().chain(&v.normal).chain(&v.uv) {
                out.extend_from_slice(&f.to_le_bytes());
            }
        }
        out
    }

    /// The indices as the index buffer holds them: little-endian `u32`s.
    pub fn index_bytes(&self) -> Vec<u8> {
        self.indices.iter().flat_map(|i| i.to_le_bytes()).collect()
    }
}

/// What one instanced draw shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BatchKey {
    /// Index into [`DrawPlan::meshes`].
    pub mesh: u32,
    /// The section slot.
    pub section: u32,
    /// Index into [`DrawPlan::textures`]; `None` draws untextured.
    pub texture: Option<u32>,
    pub two_sided: bool,
    /// Drawn with the other front face ([`Affine::mirrors`]).
    pub mirrored: bool,
}

/// One instanced draw: a mesh section and the world matrices of its placements, in placement order.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Batch {
    pub key: BatchKey,
    pub instances: Vec<Mat4>,
}

/// What [`DrawPlan::build`] counted.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlanStats {
    pub placements: usize,
    /// Hidden by their serialized flags.
    pub hidden: usize,
    /// Not hidden, but their mesh did not decode.
    pub no_mesh: usize,
    /// Not hidden, but the boundary refused their transform.
    pub bad_transform: usize,
    /// Drawn.
    pub drawn: usize,
    /// Drawn with a mirroring transform.
    pub mirrored: usize,
    /// Section instances over all batches.
    pub instances: usize,
    /// Sections with no triangles, skipped.
    pub empty_sections: usize,
}

/// Everything the renderer needs from a scene.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct DrawPlan {
    pub meshes: Vec<MeshData>,
    /// Texture object paths (keys of [`Scene::textures`]) the batches name, first-use order.
    pub textures: Vec<String>,
    /// Sorted by [`BatchKey`].
    pub batches: Vec<Batch>,
    pub stats: PlanStats,
}

/// The world transform of a placement pose (module documentation), or `None` where the boundary refuses it.
pub fn world_affine(pose: &Pose) -> Option<Affine> {
    match *pose {
        Pose::Attached { actor, component } => {
            let c = frame::Placement::new(
                component.translation,
                component.rotation,
                component.scale,
                component.scale3d,
            );
            let a = frame::Placement::new(
                actor.location,
                actor.rotation,
                actor.draw_scale,
                actor.draw_scale3d,
            );
            frame::component_to_world(&c, &a)
        }
        Pose::Collection {
            matrix,
            scale3d,
            scale,
            ..
        } => {
            let plane = |i: usize| [matrix[i], matrix[i + 1], matrix[i + 2], matrix[i + 3]];
            let m = Ue3Matrix {
                x_plane: plane(0),
                y_plane: plane(4),
                z_plane: plane(8),
                w_plane: plane(12),
            };
            // The boundary multiplies each row by an f32 scale; the product of Scale3D and scale is kept in f64 here,
            // as the tool keeps it, and applied to the unscaled rows.
            let mut affine = Affine::from_ue3_matrix(&m, [1.0, 1.0, 1.0])?;
            for (row, s) in affine.rows.iter_mut().zip(scale3d) {
                let k = f64::from(s) * f64::from(scale);
                for cell in row.iter_mut() {
                    *cell *= k;
                }
            }
            affine.is_finite().then_some(affine)
        }
    }
}

impl DrawPlan {
    /// The plan for every placement of `scene` that is not hidden (module documentation).
    pub fn build(scene: &Scene) -> DrawPlan {
        let mut stats = PlanStats::default();
        let mut meshes: Vec<MeshData> = Vec::new();
        let mut mesh_ids: DetMap<String, Option<u32>> = DetMap::new();
        let mut textures: Vec<String> = Vec::new();
        let mut texture_ids: DetMap<String, u32> = DetMap::new();
        let mut batches: DetMap<BatchKey, Vec<Mat4>> = DetMap::new();
        for p in scene.placements.values() {
            stats.placements += 1;
            if p.is_hidden() {
                stats.hidden += 1;
                continue;
            }
            let mesh_id = match mesh_ids.get(&p.mesh) {
                Some(id) => *id,
                None => {
                    let id = match scene.meshes.get(p.mesh.as_str()).map(|m| &m.status) {
                        Some(MeshStatus::Decoded(asset)) => asset
                            .lods
                            .first()
                            .and_then(|lod| MeshData::from_lod(&p.mesh, lod))
                            .map(|data| {
                                meshes.push(data);
                                u32::try_from(meshes.len() - 1).unwrap_or(u32::MAX)
                            }),
                        _ => None,
                    };
                    mesh_ids.insert(p.mesh.clone(), id);
                    id
                }
            };
            let Some(mesh_id) = mesh_id else {
                stats.no_mesh += 1;
                continue;
            };
            let Some(affine) = world_affine(&p.pose) else {
                stats.bad_transform += 1;
                continue;
            };
            stats.drawn += 1;
            let mirrored = affine.mirrors();
            stats.mirrored += usize::from(mirrored);
            let world = affine.to_engine();
            let effective = scene.effective_materials(p);
            let mesh = &meshes[mesh_id as usize];
            for (slot, range) in mesh.sections.iter().enumerate() {
                if range.index_count == 0 {
                    stats.empty_sections += 1;
                    continue;
                }
                let material = effective
                    .get(slot)
                    .and_then(|m| m.as_deref())
                    .and_then(|m| scene.materials.get(m));
                let texture = material.and_then(pick_diffuse).map(|t| {
                    *texture_ids.entry(t.to_string()).or_insert_with(|| {
                        textures.push(t.to_string());
                        u32::try_from(textures.len() - 1).unwrap_or(u32::MAX)
                    })
                });
                let key = BatchKey {
                    mesh: mesh_id,
                    section: u32::try_from(slot).unwrap_or(u32::MAX),
                    texture,
                    two_sided: material.and_then(|m| m.two_sided).unwrap_or(true),
                    mirrored,
                };
                batches.entry(key).or_default().push(world);
                stats.instances += 1;
            }
        }
        DrawPlan {
            meshes,
            textures,
            batches: batches
                .into_iter()
                .map(|(key, instances)| Batch { key, instances })
                .collect(),
            stats,
        }
    }
}

#[cfg(test)]
mod tests;
