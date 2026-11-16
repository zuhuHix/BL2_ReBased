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
//! ([`frame::Affine::mirrors`]) goes to its own batch, drawn with the other front face. Each instance carries its
//! normal matrix too ([`normal_matrix`]): the inverse transpose of the transform's 3x3, so that a non-uniform scale
//! (Sanctuary has many) turns a slanted face's normal correctly.
//!
//! **The diffuse texture** of a material is picked by parameter name with the rule of `tools/prepare_level.py`
//! (`material_parameters`, `CHANNELS` and `ALIASES`, `channel_for_parameter`, `parameter_priority`, and the
//! channel loop of `Scene.material`): the chain's named texture parameters, a child's replacing its parent's of
//! the same name and the base material's sample-parameter expressions giving the defaults; then, among those whose
//! name maps to the diffuse channel, the first one, replaced by a later one that is not a `Common_Textures.Stub`
//! texture where the current one is, or by one of higher priority. A candidate that does not decode is skipped, as
//! the tool's loop skips a texture whose decode raises: the plan keeps each material's candidates in the tool's order
//! ([`DiffuseCandidate`]) and the renderer takes the best one that decodes ([`pick_from`]), which is the tool's pick
//! among the decodable ones (the loop keeps the first of the best-ranked). The tool's inferences for materials with
//! no named diffuse (a sole unnamed `_Dif` sample, cooked texture resources, inspected fallbacks and recipes) are not
//! ported: such a material is drawn untextured, and [`PlanStats`] counts why. Two more differences: an explicit null override in a child does not hide
//! its parent's texture here (the scene records no null entries), and the tool also accepts the
//! `MaterialExpressionTextureSampleParameter2D_<n>` names, which only its unnamed inference uses.
//!
//! **Sidedness** is the base material's `TwoSided`; a material whose chain gives none (or a section with no material)
//! is drawn two-sided, so that nothing is hidden by a guess.
//!
//! **Material slots** (lane H8-MATERIALS). With a [`MaterialLibrary`] ([`DrawPlan::build_with`]), each section's
//! material becomes a [`MaterialSlot`]: its diffuse candidates (named, or the one texture the library's fallback
//! rules found), an unconnected-input constant colour, its blend mode, unlit or lit, its opacity mask clip value, and
//! the sky inputs of the sky dome's material. Without one ([`DrawPlan::build`]) a slot has only the named
//! candidates and draws opaque and lit, as stage 2 did. Blended instances (translucent, additive, modulate) are drawn
//! after the opaque ones in back-to-front order ([`back_to_front`]).
//!
//! **Baked lighting** (lane H9-LIGHTMAPS). With a [`SceneLighting`] ([`DrawPlan::build_lit`]), each vertex carries
//! its mesh's light-map UV set (`LightMapCoordinateIndex`), and each instance whose component has a light map carries
//! what the renderer needs for it ([`InstanceLight`]): a texture light map's atlas rectangle and coefficient scales,
//! or a per-vertex one's scales, and the dominant light's shadow map rectangle. Instances whose textures differ are
//! batched apart ([`LightingSlot`]); a per-vertex light map is a slot of its own, since its samples are per component.
//! A component whose light map cannot be applied (its mesh has no such UV set, or the per-vertex samples do not match
//! the vertex count) is drawn as before and counted.
//!
//! Status: UNVERIFIED as the game's look. This is the B2 benchmark's level of drawing: one diffuse texture, a sun and
//! ambient light, and the baked light maps where read.

use ow_assets::mesh::MeshLod;
use ow_core::DetMap;

use crate::frame::{self, Affine, Mat4, Ue3Matrix};
use crate::lightmap::{COEFFICIENTS, LightMap, SAMPLE_BYTES, SceneLighting};
use crate::material::{Blend, DEFAULT_CLIP, DiffuseSource, MaterialLibrary, SkyInputs};
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

/// A texture a material names under a diffuse parameter, with what the tool ranks it by.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DiffuseCandidate {
    /// The texture's object path (a key of [`Scene::textures`]).
    pub path: String,
    /// [`parameter_priority`] of its parameter.
    pub priority: i32,
    /// A `Common_Textures.Stub` texture.
    pub stub: bool,
}

/// The diffuse candidates of `material` in the tool's order (module documentation): its named texture parameters as
/// the tool's dict holds them, the deepest parent first and a nearer level replacing a name in place, then those
/// whose name maps to the diffuse channel.
pub fn diffuse_candidates(material: &SceneMaterial) -> Vec<DiffuseCandidate> {
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
    parameters
        .into_iter()
        .filter(|(name, _)| channel_for_parameter(name) == Some(Channel::Diffuse))
        .map(|(name, texture)| DiffuseCandidate {
            path: texture.to_string(),
            priority: parameter_priority(&name),
            stub: is_stub(texture),
        })
        .collect()
}

/// The tool's channel loop over `candidates`, skipping those `usable` refuses (a texture that does not decode): the
/// first candidate, replaced by a later one that is not a stub where the current one is, or by one of the same
/// stubness and higher priority.
pub fn pick_from<'a>(
    candidates: &'a [DiffuseCandidate],
    usable: &dyn Fn(&str) -> bool,
) -> Option<&'a DiffuseCandidate> {
    let mut picked: Option<&DiffuseCandidate> = None;
    for c in candidates.iter().filter(|c| usable(&c.path)) {
        let replace = match picked {
            None => true,
            Some(current) => {
                (current.stub && !c.stub)
                    || (current.stub == c.stub && c.priority > current.priority)
            }
        };
        if replace {
            picked = Some(c);
        }
    }
    picked
}

/// The diffuse texture of `material` by the tool's parameter rule, every candidate taken as decodable, or `None`.
pub fn pick_diffuse(material: &SceneMaterial) -> Option<String> {
    pick_from(&diffuse_candidates(material), &|_| true).map(|c| c.path.clone())
}

/// One vertex as the renderer uploads it: position in uu, the packed normal unpacked, the first UV set, and the
/// light-map UV set (the first set too when the mesh is built without one).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub lightmap_uv: [f32; 2],
}

/// Bytes per uploaded vertex: 10 floats.
pub const VERTEX_BYTES: usize = 40;

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
    /// Every vertex has the light-map UV set asked for ([`MeshData::from_lod_with`]).
    pub lightmap_uvs: bool,
}

impl MeshData {
    /// LOD 0 of a decoded mesh. `None` for an LOD with no vertex, an index past the vertices, or a section's range
    /// past the index buffer (`ow-assets` checks these already; this keeps the renderer from trusting it blindly).
    pub fn from_lod(path: &str, lod: &MeshLod) -> Option<MeshData> {
        Self::from_lod_with(path, lod, 0)
    }

    /// [`MeshData::from_lod`] with UV set `lightmap_set` as the light-map UVs (the first set where a vertex lacks it,
    /// and then [`MeshData::lightmap_uvs`] is false).
    pub fn from_lod_with(path: &str, lod: &MeshLod, lightmap_set: usize) -> Option<MeshData> {
        if lod.vertices.is_empty()
            || lod
                .indices
                .iter()
                .any(|&i| i as usize >= lod.vertices.len())
        {
            return None;
        }
        let vertices = lod
            .vertices
            .iter()
            .map(|v| {
                let uv = v.uvs.first().copied().unwrap_or([0.0, 0.0]);
                Vertex {
                    position: v.position,
                    normal: v.normal,
                    uv,
                    lightmap_uv: v.uvs.get(lightmap_set).copied().unwrap_or(uv),
                }
            })
            .collect();
        let lightmap_uvs = lod.vertices.iter().all(|v| v.uvs.len() > lightmap_set);
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
            lightmap_uvs,
        })
    }

    /// The vertices as the vertex buffer holds them: position, normal, UV, light-map UV, little-endian `f32`s.
    pub fn vertex_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.vertices.len() * VERTEX_BYTES);
        for v in &self.vertices {
            for f in v
                .position
                .iter()
                .chain(&v.normal)
                .chain(&v.uv)
                .chain(&v.lightmap_uv)
            {
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
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BatchKey {
    /// Index into [`DrawPlan::meshes`].
    pub mesh: u32,
    /// The section slot.
    pub section: u32,
    /// Index into [`DrawPlan::slots`] (the section's material); `None` draws untextured, opaque and lit.
    pub material: Option<u32>,
    /// Index into [`DrawPlan::lightings`]; `None` for instances with no light map or shadow map applied.
    pub lighting: Option<u32>,
    pub two_sided: bool,
    /// Drawn with the other front face ([`Affine::mirrors`]).
    pub mirrored: bool,
}

/// The normal matrix of a transform, for column vectors: the inverse transpose of its 3x3, in the engine's
/// column-major order, as three columns. With the transform's rows (the engine matrix's columns) `a`, `b`, `c`, the
/// inverse transpose is `[b x c, c x a, a x b] / det`; dividing by the determinant's sign only keeps the direction
/// (the shader normalises), and a flattened transform (determinant 0) keeps its cross products. Computed in `f64` and
/// rounded to `f32` once.
pub fn normal_matrix(affine: &Affine) -> [[f32; 3]; 3] {
    let cross = |u: [f64; 3], v: [f64; 3]| {
        [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ]
    };
    let [a, b, c] = affine.rows;
    let sign = if affine.determinant() < 0.0 {
        -1.0
    } else {
        1.0
    };
    [cross(b, c), cross(c, a), cross(a, b)].map(|col| col.map(|v| (v * sign) as f32))
}

/// How an instance's light map is applied ([`InstanceLight::mode`]).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LightMode {
    /// No light map: the scene's sky term.
    #[default]
    None,
    /// The batch's per-vertex samples.
    Vertex,
    /// The batch's atlas textures.
    Texture,
}

/// How an instance's shadow map is read ([`InstanceLight::shadow`]).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShadowMode {
    /// None: the sun is unshadowed.
    #[default]
    None,
    /// The texel is the light's visibility.
    Factor,
    /// The texel is a distance field, thresholded at one half (UNVERIFIED reading).
    DistanceField,
}

/// An instance's baked lighting (module documentation).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct InstanceLight {
    pub mode: LightMode,
    /// The light map's atlas rectangle: UV scale then bias.
    pub lightmap_rect: [f32; 4],
    /// The scales of coefficients 0 and 1.
    pub scales: [[f32; 3]; 2],
    pub shadow: ShadowMode,
    /// The shadow map's atlas rectangle: UV scale then bias.
    pub shadow_rect: [f32; 4],
}

/// One drawn placement of a batch: its world matrix and its normal matrix ([`normal_matrix`]), and its baked
/// lighting.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Instance {
    pub world: Mat4,
    pub normal: [[f32; 3]; 3],
    pub light: InstanceLight,
}

impl Instance {
    pub fn new(affine: &Affine) -> Instance {
        Instance {
            world: affine.to_engine(),
            normal: normal_matrix(affine),
            light: InstanceLight::default(),
        }
    }
}

/// What identifies a texture lighting slot: its coefficient 0 and 1 atlases and its shadow atlas.
type LightingKey = (Option<String>, Option<String>, Option<String>);

/// The lighting textures or samples a batch binds: coefficient 0 and 1 atlases (keys of
/// [`SceneLighting::textures`]), the shadow atlas, or a per-vertex light map's samples.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LightingSlot {
    pub textures: [Option<String>; 2],
    pub shadow: Option<String>,
    /// One sample per LOD 0 vertex of the batch's mesh.
    pub samples: Option<Vec<[u8; SAMPLE_BYTES]>>,
}

/// One instanced draw: a mesh section and its placements, in placement order.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Batch {
    pub key: BatchKey,
    pub instances: Vec<Instance>,
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
    /// Section instances drawn untextured because the section has no material (no override, no section material).
    pub untextured_no_material: usize,
    /// Untextured because the section's material is not in [`Scene::materials`].
    pub untextured_unknown_material: usize,
    /// Untextured because the material has no diffuse: by parameter, and (with a library) by the fallback rules, as
    /// a constant colour or as the sky.
    pub untextured_no_diffuse: usize,
    /// Textured by a fallback rule of the library (unnamed sample, cooked `_Dif`, sole cooked texture).
    pub fallback_textured: usize,
    /// Drawn with an unconnected input's constant colour.
    pub constant_coloured: usize,
    /// Section instances by blend mode (the sky dome counted under `sky_instances` instead).
    pub by_blend: [usize; 5],
    /// Section instances of the sky dome's material.
    pub sky_instances: usize,
    /// Section instances of a blended material (translucent, additive or modulate) with neither a diffuse nor a
    /// constant, not batched: what they blend is the stripped graph's, and drawn with grey a translucent one covered
    /// Sanctuary's water decals in white, an additive one would lay a grey veil and a modulate one darken by about
    /// half. Counted under `untextured_no_diffuse` and `by_blend` too.
    pub blended_skipped: usize,
    /// Drawn placements lit by a texture light map, by a per-vertex one, and with a shadow map (lane H9).
    pub lit_texture: usize,
    pub lit_vertex: usize,
    pub shadowed: usize,
    /// Drawn placements whose light map was not applied: the mesh lacks the light-map UV set (or its set was not
    /// read: `LightingCensus::uv_set_unread`), or the per-vertex samples do not match the mesh's vertex count.
    pub lightmap_no_uvs: usize,
    pub lightmap_vertex_mismatch: usize,
    /// Texture light maps with a third coefficient texture, which the renderer does not bind (none on Sanctuary).
    pub third_coefficient: usize,
    /// Drawn placements whose shadow map was not applied because the mesh lacks the light-map UV set (review r1 of
    /// PR #365).
    pub shadow_no_uvs: usize,
}

impl PlanStats {
    /// The section instances of `blend`.
    pub fn blend_count(&self, blend: Blend) -> usize {
        self.by_blend[blend_index(blend)]
    }
}

/// The place of `blend` in [`PlanStats::by_blend`].
fn blend_index(blend: Blend) -> usize {
    match blend {
        Blend::Opaque => 0,
        Blend::Masked => 1,
        Blend::Translucent => 2,
        Blend::Additive => 3,
        Blend::Modulate => 4,
    }
}

/// How a section's material is drawn (module documentation).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialSlot {
    /// The material's object path.
    pub path: String,
    /// The diffuse candidates in the tool's order; empty draws untextured.
    pub diffuse: Vec<DiffuseCandidate>,
    /// An unconnected input's constant, drawn instead of a texture.
    pub constant: Option<[f32; 3]>,
    pub blend: Blend,
    pub unlit: bool,
    /// The opacity mask clip value of a masked material.
    pub clip: f32,
    /// The sky dome's inputs: drawn first, unlit, behind everything.
    pub sky: Option<SkyInputs>,
}

impl MaterialSlot {
    /// The slot of `material` at `path`, read with `library` when given.
    fn new(
        path: &str,
        material: &SceneMaterial,
        library: Option<&MaterialLibrary>,
    ) -> MaterialSlot {
        let named = diffuse_candidates(material);
        let info = library.and_then(|l| l.materials.get(path));
        let mut slot = MaterialSlot {
            path: path.to_string(),
            diffuse: named,
            constant: None,
            blend: Blend::Opaque,
            unlit: false,
            clip: DEFAULT_CLIP,
            sky: None,
        };
        if let Some(info) = info {
            slot.blend = info.blend;
            slot.unlit = info.unlit();
            slot.clip = info.opacity_mask_clip;
            slot.sky = info.sky.clone();
            if slot.diffuse.is_empty() {
                if let Some(t) = info.diffuse.texture() {
                    slot.diffuse.push(DiffuseCandidate {
                        path: t.to_string(),
                        priority: 0,
                        stub: is_stub(t),
                    });
                } else if let DiffuseSource::Constant(c) = info.diffuse {
                    slot.constant = Some(c);
                }
            }
        }
        slot
    }
}

/// The order blended instances are drawn in: farthest from `eye` first (by each instance's origin), ties in
/// `(batch, instance)` order. `items` are `(batch, instance, origin)`; the result keeps `(batch, instance)`.
pub fn back_to_front(items: &[(usize, u32, [f32; 3])], eye: [f32; 3]) -> Vec<(usize, u32)> {
    let mut keyed: Vec<(f64, usize, u32)> = items
        .iter()
        .map(|&(b, i, o)| {
            let d = [0, 1, 2].map(|k| f64::from(o[k]) - f64::from(eye[k]));
            (d[0] * d[0] + d[1] * d[1] + d[2] * d[2], b, i)
        })
        .collect();
    keyed.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    keyed.into_iter().map(|(_, b, i)| (b, i)).collect()
}

/// Everything the renderer needs from a scene.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct DrawPlan {
    pub meshes: Vec<MeshData>,
    /// The materials the batches use, first-use order.
    pub slots: Vec<MaterialSlot>,
    /// The lighting the batches bind, first-use order.
    pub lightings: Vec<LightingSlot>,
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
    /// The plan for every placement of `scene` that is not hidden, its materials by named diffuse only (stage 2).
    pub fn build(scene: &Scene) -> DrawPlan {
        Self::build_with(scene, None)
    }

    /// The plan with each material read through `library` when given (module documentation).
    pub fn build_with(scene: &Scene, library: Option<&MaterialLibrary>) -> DrawPlan {
        Self::build_lit(scene, library, None)
    }

    /// [`DrawPlan::build_with`], with the baked lighting of `lighting` when given (module documentation).
    pub fn build_lit(
        scene: &Scene,
        library: Option<&MaterialLibrary>,
        lighting: Option<&SceneLighting>,
    ) -> DrawPlan {
        let mut stats = PlanStats::default();
        let mut lightings: Vec<LightingSlot> = Vec::new();
        let mut lighting_ids: DetMap<LightingKey, u32> = DetMap::new();
        let mut meshes: Vec<MeshData> = Vec::new();
        let mut mesh_ids: DetMap<String, Option<u32>> = DetMap::new();
        let mut slots: Vec<MaterialSlot> = Vec::new();
        let mut slot_ids: DetMap<String, u32> = DetMap::new();
        let mut batches: DetMap<BatchKey, Vec<Instance>> = DetMap::new();
        for (key, p) in scene.placements.iter() {
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
                            .and_then(|lod| {
                                // A mesh whose light-map UV set was not read has none to apply.
                                let set = lighting.map(|l| l.uv_sets.get(p.mesh.as_str()).copied());
                                let mut data = MeshData::from_lod_with(
                                    &p.mesh,
                                    lod,
                                    set.flatten().unwrap_or(0),
                                )?;
                                if set == Some(None) {
                                    data.lightmap_uvs = false;
                                }
                                Some(data)
                            })
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
            let mut instance = Instance::new(&affine);
            let mesh = &meshes[mesh_id as usize];
            let lighting_id = match lighting.and_then(|l| l.components.get(key)) {
                Some(light) => {
                    let mut slot = LightingSlot::default();
                    match &light.light_map {
                        Some(LightMap::Texture {
                            textures,
                            scales,
                            uv_scale,
                            uv_bias,
                        }) if mesh.lightmap_uvs => {
                            slot.textures = [textures[0].clone(), textures[1].clone()];
                            stats.third_coefficient +=
                                usize::from(textures[COEFFICIENTS - 1].is_some());
                            instance.light.mode = LightMode::Texture;
                            instance.light.lightmap_rect =
                                [uv_scale[0], uv_scale[1], uv_bias[0], uv_bias[1]];
                            instance.light.scales = [scales[0], scales[1]];
                            stats.lit_texture += 1;
                        }
                        Some(LightMap::Vertex { samples, scales })
                            if samples.len() == mesh.vertices.len() =>
                        {
                            slot.samples = Some(samples.clone());
                            instance.light.mode = LightMode::Vertex;
                            instance.light.scales = [scales[0], scales[1]];
                            stats.lit_vertex += 1;
                        }
                        Some(LightMap::Texture { .. }) => stats.lightmap_no_uvs += 1,
                        Some(LightMap::Vertex { .. }) => stats.lightmap_vertex_mismatch += 1,
                        None => {}
                    }
                    if let Some(shadow) = light.shadow.as_ref().filter(|_| mesh.lightmap_uvs) {
                        slot.shadow = Some(shadow.texture.clone());
                        instance.light.shadow = if shadow.shadow_factor {
                            ShadowMode::Factor
                        } else {
                            ShadowMode::DistanceField
                        };
                        instance.light.shadow_rect = [
                            shadow.uv_scale[0],
                            shadow.uv_scale[1],
                            shadow.uv_bias[0],
                            shadow.uv_bias[1],
                        ];
                        stats.shadowed += 1;
                    } else if light.shadow.is_some() {
                        stats.shadow_no_uvs += 1;
                    }
                    if slot == LightingSlot::default() {
                        None
                    } else if slot.samples.is_some() {
                        // Per-vertex samples belong to this component alone.
                        lightings.push(slot);
                        Some(u32::try_from(lightings.len() - 1).unwrap_or(u32::MAX))
                    } else {
                        let id = (
                            slot.textures[0].clone(),
                            slot.textures[1].clone(),
                            slot.shadow.clone(),
                        );
                        Some(*lighting_ids.entry(id).or_insert_with(|| {
                            lightings.push(slot);
                            u32::try_from(lightings.len() - 1).unwrap_or(u32::MAX)
                        }))
                    }
                }
                None => None,
            };
            let effective = scene.effective_materials(p);
            for (slot, range) in mesh.sections.iter().enumerate() {
                if range.index_count == 0 {
                    stats.empty_sections += 1;
                    continue;
                }
                let path = effective.get(slot).and_then(|m| m.as_deref());
                let material = path.and_then(|m| scene.materials.get(m).map(|sm| (m, sm)));
                let index = material.map(|(m, sm)| {
                    *slot_ids.entry(m.to_string()).or_insert_with(|| {
                        slots.push(MaterialSlot::new(m, sm, library));
                        u32::try_from(slots.len() - 1).unwrap_or(u32::MAX)
                    })
                });
                let made = index.and_then(|i| slots.get(i as usize));
                match (path, made) {
                    (None, _) => stats.untextured_no_material += 1,
                    (Some(_), None) => stats.untextured_unknown_material += 1,
                    (Some(_), Some(m)) if m.sky.is_some() => {}
                    (Some(_), Some(m)) if m.constant.is_some() => stats.constant_coloured += 1,
                    (Some(_), Some(m)) if m.diffuse.is_empty() => stats.untextured_no_diffuse += 1,
                    (Some(_), Some(m)) => {
                        let named =
                            material.is_some_and(|(_, sm)| !diffuse_candidates(sm).is_empty());
                        if !named && !m.diffuse.is_empty() {
                            stats.fallback_textured += 1;
                        }
                    }
                }
                match made {
                    Some(m) if m.sky.is_some() => stats.sky_instances += 1,
                    Some(m) => stats.by_blend[blend_index(m.blend)] += 1,
                    None => stats.by_blend[blend_index(Blend::Opaque)] += 1,
                }
                if made.is_some_and(|m| {
                    matches!(
                        m.blend,
                        Blend::Translucent | Blend::Additive | Blend::Modulate
                    ) && m.sky.is_none()
                        && m.constant.is_none()
                        && m.diffuse.is_empty()
                }) {
                    stats.blended_skipped += 1;
                    continue;
                }
                let two_sided = library
                    .and_then(|l| path.and_then(|m| l.materials.get(m)))
                    .and_then(|i| i.two_sided)
                    .or_else(|| material.and_then(|(_, sm)| sm.two_sided))
                    .unwrap_or(true);
                let batch = BatchKey {
                    mesh: mesh_id,
                    section: u32::try_from(slot).unwrap_or(u32::MAX),
                    material: index,
                    lighting: lighting_id,
                    two_sided,
                    mirrored,
                };
                batches.entry(batch).or_default().push(instance);
                stats.instances += 1;
            }
        }
        DrawPlan {
            meshes,
            slots,
            lightings,
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
