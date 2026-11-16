//! Owner: lane H9-LIGHTMAPS. The scene's baked lighting as its packages hold it: each placed static mesh
//! component's light map (a texture light map in an atlas, or one sample per vertex) and the dominant light's shadow
//! map, the lighting textures they name, and the light-map UV set of each mesh.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-65-*`.
//!
//! **Where the layout comes from.** No record, tool or C++ reader in this repository decodes light maps, and no
//! listing was read: the layout below was fitted to the installed packages and checked with structural oracles
//! (AGENTS.md rule 4), on every `StaticMeshComponent` of Sanctuary's ten level files (4,890 of them).
//! A component's tags end with `None` (read at the component prefix); its untagged tail is, in this order:
//! - a count of LODs; for each LOD:
//!   - an array of shadow map references (`ShadowMap2D` exports): a count, then 32-bit references;
//!   - an array of shadow vertex buffer references (`ShadowMap1D` exports), the same way;
//!   - a light map type word: 0 none, 1 per-vertex, 2 texture;
//!   - for type 1 or 2: an array of light GUIDs (a count, then 16 bytes each);
//!   - for type 2: three coefficient slots, each a texture reference (a `LightMapTexture2D` export or 0) and a scale
//!     of three floats; then the atlas UV scale (two floats) and UV bias (two floats);
//!   - for type 1: a reference to the component itself, then a bulk array of samples (a flags word, a count, a byte
//!     size, the absolute file offset of the data, then the data: 8 bytes, two `FColor`s, per vertex), three scales
//!     of three floats, and a second bulk array (empty on Sanctuary);
//!   - a byte: 1 when per-vertex override colours follow (a stride word of 4, a count, a bulk element size of 4, the
//!     count again, then 4 bytes per vertex), else 0.
//!
//! The oracles: every one of the 4,890 tails is consumed exactly to its export's end; every bulk array's offset word
//! equals the data's own position in the file; every type-2 slot names a `LightMapTexture2D` (slots 0 and 1) or
//! nothing (slot 2); the light map counts match the census of decision 2026-11-16-58 (136 `LightMapTexture2D`
//! exports). [`parse_lods`] refuses anything else. What the fields mean beyond their shape is read from the data:
//! slot 0's textures are named `EnvironmentLightmap*` and carry scale (1, 1, 1), slot 1's `DominantBouncemap*` with a
//! per-component scale; every `ShadowMap2D` of Sanctuary carries the dominant directional light's `LightGuid`.
//! How the game combines them is **UNVERIFIED** (the renderer's decode is a labelled fit: `crate::render`).

use ow_core::OrderedMap;
use ow_defs::DefStore;
use ow_object::prefix::PropertyPrefix;
use ow_package::package::Package;
use ow_package::props::{self, NoArrayTypes, PropEntry, PropValue};

use crate::scene::{ExportKey, Holder, Scene};

/// The coefficient slots of a texture light map.
pub const COEFFICIENTS: usize = 3;
/// Bytes of one per-vertex light map sample: two `FColor`s (B, G, R, A each).
pub const SAMPLE_BYTES: usize = 8;
/// The key prefix of a lighting texture in a renderer's texture source: `lightmap:<file>.<path>` (light map atlases
/// are top-level objects of each level file, so their bare paths repeat across levels).
pub const KEY_PREFIX: &str = "lightmap:";

/// A light map as a component's LOD serializes it (references unresolved).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum RawLightMap {
    None,
    /// Per vertex: two `FColor`s per vertex, and a scale per coefficient.
    Vertex {
        owner: i32,
        guids: usize,
        samples: Vec<[u8; SAMPLE_BYTES]>,
        scales: [[f32; 3]; COEFFICIENTS],
    },
    /// In atlas textures: a texture reference and a scale per coefficient, and where the component's rectangle is.
    Texture {
        guids: usize,
        textures: [i32; COEFFICIENTS],
        scales: [[f32; 3]; COEFFICIENTS],
        uv_scale: [f32; 2],
        uv_bias: [f32; 2],
    },
}

/// One LOD of a component's tail.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct RawLod {
    pub shadow_maps: Vec<i32>,
    pub shadow_vertex_buffers: Vec<i32>,
    pub light_map: RawLightMap,
    /// The count of per-vertex override colours that follow, when the flag byte says they do.
    pub override_colors: Option<u32>,
}

/// A bounds-checked little-endian cursor over a tail.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Cursor<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self.at.checked_add(n).ok_or("a size overflows")?;
        let out = self.bytes.get(self.at..end).ok_or_else(|| {
            format!(
                "the tail ends before byte {end} (it has {})",
                self.bytes.len()
            )
        })?;
        self.at = end;
        Ok(out)
    }

    fn word(&mut self) -> Result<i32, String> {
        let b = self.take(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn float(&mut self) -> Result<f32, String> {
        let b = self.take(4)?;
        let f = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        if f.is_finite() {
            Ok(f)
        } else {
            Err(format!("a non-finite float before byte {}", self.at))
        }
    }

    fn vector(&mut self) -> Result<[f32; 3], String> {
        Ok([self.float()?, self.float()?, self.float()?])
    }

    /// A count of elements of `size` bytes each that must fit in what is left.
    fn count(&mut self, size: usize) -> Result<usize, String> {
        let n = self.word()?;
        let n = usize::try_from(n).map_err(|_| format!("a negative count {n}"))?;
        let left = self.bytes.len() - self.at;
        if n.checked_mul(size).is_none_or(|b| b > left) {
            return Err(format!(
                "a count of {n} elements of {size} bytes past the tail's {left} bytes"
            ));
        }
        Ok(n)
    }

    fn refs(&mut self) -> Result<Vec<i32>, String> {
        let n = self.count(4)?;
        (0..n).map(|_| self.word()).collect()
    }

    /// A bulk array: flags, count, byte size, offset, then the data. The flags must be 0 and the offset the data's
    /// own position in the file (`tail_at` is the tail's).
    fn bulk(&mut self, tail_at: usize, element: usize) -> Result<&[u8], String> {
        let flags = self.word()?;
        let count = self.word()?;
        let size = self.word()?;
        let offset = self.word()?;
        if flags != 0 {
            return Err(format!("a bulk array with flags {flags}"));
        }
        let (Ok(count), Ok(size)) = (usize::try_from(count), usize::try_from(size)) else {
            return Err("a negative bulk count or size".into());
        };
        if count.checked_mul(element) != Some(size) {
            return Err(format!(
                "a bulk array of {count} elements in {size} bytes, not {element} each"
            ));
        }
        let here = tail_at.checked_add(self.at).ok_or("a position overflows")?;
        if usize::try_from(offset).ok() != Some(here) {
            return Err(format!(
                "a bulk array's offset {offset} is not its position {here}"
            ));
        }
        self.take(size)
    }
}

/// Parses a component's untagged tail (module documentation), `tail_at` being its absolute position in the file.
/// `Err` unless the bytes are consumed exactly.
pub fn parse_lods(tail: &[u8], tail_at: usize) -> Result<Vec<RawLod>, String> {
    let mut c = Cursor { bytes: tail, at: 0 };
    // The smallest LOD is 13 bytes (two empty arrays, type 0, the flag byte).
    let lods = c.count(13)?;
    let mut out = Vec::with_capacity(lods);
    for _ in 0..lods {
        let shadow_maps = c.refs()?;
        let shadow_vertex_buffers = c.refs()?;
        let kind = c.word()?;
        let guids = match kind {
            1 | 2 => {
                let n = c.count(16)?;
                c.take(n * 16)?;
                n
            }
            _ => 0,
        };
        let light_map = match kind {
            0 => RawLightMap::None,
            1 => {
                let owner = c.word()?;
                let data = c.bulk(tail_at, SAMPLE_BYTES)?;
                let samples = data
                    .chunks_exact(SAMPLE_BYTES)
                    .map(|s| {
                        let mut a = [0u8; SAMPLE_BYTES];
                        a.copy_from_slice(s);
                        a
                    })
                    .collect();
                let scales = [c.vector()?, c.vector()?, c.vector()?];
                if !c.bulk(tail_at, SAMPLE_BYTES)?.is_empty() {
                    return Err("a second per-vertex sample array that is not empty".into());
                }
                RawLightMap::Vertex {
                    owner,
                    guids,
                    samples,
                    scales,
                }
            }
            2 => {
                let mut textures = [0i32; COEFFICIENTS];
                let mut scales = [[0f32; 3]; COEFFICIENTS];
                for (t, s) in textures.iter_mut().zip(scales.iter_mut()) {
                    *t = c.word()?;
                    *s = c.vector()?;
                }
                let uv_scale = [c.float()?, c.float()?];
                let uv_bias = [c.float()?, c.float()?];
                RawLightMap::Texture {
                    guids,
                    textures,
                    scales,
                    uv_scale,
                    uv_bias,
                }
            }
            k => return Err(format!("light map type {k}")),
        };
        let override_colors = match c.take(1)?[0] {
            0 => None,
            1 => {
                let stride = c.word()?;
                let n = c.word()?;
                let element = c.word()?;
                let count = c.count(4)?;
                if stride != 4 || element != 4 || usize::try_from(n).ok() != Some(count) {
                    return Err(format!(
                        "an override colour stream {stride} {n} {element} {count}"
                    ));
                }
                c.take(count * 4)?;
                Some(u32::try_from(count).map_err(|_| "a colour count past u32")?)
            }
            b => return Err(format!("an override colour flag {b}")),
        };
        out.push(RawLod {
            shadow_maps,
            shadow_vertex_buffers,
            light_map,
            override_colors,
        });
    }
    if c.at != tail.len() {
        return Err(format!("{} bytes left after the LODs", tail.len() - c.at));
    }
    Ok(out)
}

/// A component's light map, ready for the renderer.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum LightMap {
    /// Atlas textures by coefficient (keys of [`SceneLighting::textures`], `None` for an empty slot), their scales,
    /// and the component's rectangle in the atlas: `uv * uv_scale + uv_bias`.
    Texture {
        textures: [Option<String>; COEFFICIENTS],
        scales: [[f32; 3]; COEFFICIENTS],
        uv_scale: [f32; 2],
        uv_bias: [f32; 2],
    },
    /// One sample per LOD 0 vertex, and a scale per coefficient.
    Vertex {
        samples: Vec<[u8; SAMPLE_BYTES]>,
        scales: [[f32; 3]; COEFFICIENTS],
    },
}

/// The dominant directional light's shadow map of a component.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct ShadowMap {
    /// A key of [`SceneLighting::textures`] (a `ShadowMapTexture2D`).
    pub texture: String,
    /// The component's rectangle in the shadow atlas: `uv * uv_scale + uv_bias`.
    pub uv_scale: [f32; 2],
    pub uv_bias: [f32; 2],
    /// `bIsShadowFactorTexture` (class default true): false on every Sanctuary shadow map, whose textures look like
    /// distance fields (UNVERIFIED reading).
    pub shadow_factor: bool,
}

/// What a placed component's LOD 0 holds.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ComponentLight {
    pub light_map: Option<LightMap>,
    pub shadow: Option<ShadowMap>,
}

/// What [`SceneLighting::read`] counted.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LightingCensus {
    pub placements: usize,
    /// Tails that parsed exactly.
    pub parsed: usize,
    /// Tails refused by [`parse_lods`] or whose references did not check (first reasons in `errors`).
    pub refused: usize,
    pub texture_light_maps: usize,
    pub vertex_light_maps: usize,
    pub no_light_map: usize,
    /// Components with a shadow map of the dominant light.
    pub shadowed: usize,
    /// Shadow map references whose `LightGuid` is not the dominant light's (not used).
    pub other_shadow_maps: usize,
    /// Per-vertex shadow buffers (`ShadowMap1D`), not read.
    pub shadow_vertex_buffers: usize,
}

/// The scene's baked lighting.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SceneLighting {
    /// By placement key (the component's export).
    pub components: OrderedMap<ExportKey, ComponentLight>,
    /// Lighting textures by key ([`KEY_PREFIX`]), with their class and holder.
    pub textures: OrderedMap<String, (String, Holder)>,
    /// Each decoded mesh's `LightMapCoordinateIndex` (0 where the mesh does not serialize it: UNVERIFIED default).
    pub uv_sets: OrderedMap<String, usize>,
    pub census: LightingCensus,
    /// The first refusal reasons.
    pub errors: Vec<String>,
}

/// How many refusal reasons [`SceneLighting::errors`] keeps.
const KEPT_ERRORS: usize = 20;

impl SceneLighting {
    /// Reads every placement's component of `scene` (loaded through `defs`); `sun_guid` is the dominant directional
    /// light's `LightGuid` ([`crate::lighting::Sun::light_guid`]), which picks the shadow map.
    pub fn read(defs: &DefStore, scene: &Scene, sun_guid: Option<[i32; 4]>) -> SceneLighting {
        let mut out = SceneLighting::default();
        for (path, mesh) in scene.meshes.iter() {
            let Some(holder) = &mesh.holder else { continue };
            let pkg = defs.packages().get(holder.package);
            let index = props::read_tagged(
                pkg,
                holder.export,
                PropertyPrefix::Ordinary.bytes(),
                &NoArrayTypes,
            )
            .ok()
            .and_then(|t| int_of(&t.entries, "LightMapCoordinateIndex"))
            .and_then(|i| usize::try_from(i).ok())
            .unwrap_or(0);
            out.uv_sets.insert(path.clone(), index);
        }
        for (key, _) in scene.placements.iter() {
            out.census.placements += 1;
            let Some(level) = usize::try_from(key.level)
                .ok()
                .and_then(|l| scene.levels.get(l))
            else {
                continue;
            };
            match out.component(defs, level.package, key.export, sun_guid) {
                Ok(light) => {
                    out.census.parsed += 1;
                    match &light.light_map {
                        Some(LightMap::Texture { .. }) => out.census.texture_light_maps += 1,
                        Some(LightMap::Vertex { .. }) => out.census.vertex_light_maps += 1,
                        None => out.census.no_light_map += 1,
                    }
                    out.census.shadowed += usize::from(light.shadow.is_some());
                    out.components.insert(*key, light);
                }
                Err(e) => {
                    out.census.refused += 1;
                    if out.errors.len() < KEPT_ERRORS {
                        out.errors
                            .push(format!("{}:{}: {e}", level.name, key.export));
                    }
                }
            }
        }
        out
    }

    /// LOD 0 of component `export` of `package`.
    fn component(
        &mut self,
        defs: &DefStore,
        package: ow_package::PackageId,
        export: i32,
        sun_guid: Option<[i32; 4]>,
    ) -> Result<ComponentLight, String> {
        let pkg = defs.packages().get(package);
        let object = pkg.object(export).map_err(|e| e.to_string())?;
        let offset = usize::try_from(object.offset).map_err(|_| "a negative export offset")?;
        let size = usize::try_from(object.size).map_err(|_| "a negative export size")?;
        let tagged = props::read_tagged(
            pkg,
            export,
            PropertyPrefix::Component.bytes(),
            &NoArrayTypes,
        )
        .map_err(|e| e.to_string())?;
        let end = offset
            .checked_add(size)
            .ok_or("the export's end overflows")?;
        let tail_at = offset
            .checked_add(tagged.end)
            .ok_or("the tail's position overflows")?;
        let tail = pkg
            .data
            .get(tail_at..end)
            .ok_or("the tail is past the data")?;
        let lods = parse_lods(tail, tail_at)?;
        let Some(lod) = lods.first() else {
            return Ok(ComponentLight::default());
        };
        self.census.shadow_vertex_buffers += lod.shadow_vertex_buffers.len();
        let light_map = match &lod.light_map {
            RawLightMap::None => None,
            RawLightMap::Vertex {
                owner,
                samples,
                scales,
                ..
            } => {
                if *owner != export {
                    return Err(format!(
                        "a per-vertex light map owned by {owner}, not by the component"
                    ));
                }
                Some(LightMap::Vertex {
                    samples: samples.clone(),
                    scales: *scales,
                })
            }
            RawLightMap::Texture {
                textures,
                scales,
                uv_scale,
                uv_bias,
                ..
            } => {
                let mut keys: [Option<String>; COEFFICIENTS] = Default::default();
                for (k, &t) in keys.iter_mut().zip(textures) {
                    if t != 0 {
                        *k = Some(self.texture(pkg, package, t, "LightMapTexture2D")?);
                    }
                }
                Some(LightMap::Texture {
                    textures: keys,
                    scales: *scales,
                    uv_scale: *uv_scale,
                    uv_bias: *uv_bias,
                })
            }
        };
        let mut shadow = None;
        for &r in &lod.shadow_maps {
            let class = class_of(pkg, r)?;
            if class != "ShadowMap2D" {
                return Err(format!("a shadow map reference to a {class}"));
            }
            let entries =
                props::read_tagged(pkg, r, PropertyPrefix::Ordinary.bytes(), &NoArrayTypes)
                    .map_err(|e| e.to_string())?
                    .entries;
            if sun_guid.is_none() || guid_of(&entries, "LightGuid") != sun_guid || shadow.is_some()
            {
                self.census.other_shadow_maps += 1;
                continue;
            }
            let texture = match entries
                .iter()
                .find(|e| e.name == "Texture")
                .map(|e| &e.value)
            {
                Some(PropValue::Object(t)) if *t > 0 => {
                    self.texture(pkg, package, *t, "ShadowMapTexture2D")?
                }
                _ => return Err("a shadow map with no texture".into()),
            };
            shadow = Some(ShadowMap {
                texture,
                uv_scale: vector2_of(&entries, "CoordinateScale").ok_or("no CoordinateScale")?,
                uv_bias: vector2_of(&entries, "CoordinateBias").ok_or("no CoordinateBias")?,
                shadow_factor: entries
                    .iter()
                    .find_map(|e| match (e.name.as_str(), &e.value) {
                        ("bIsShadowFactorTexture", PropValue::Bool(b)) => Some(*b),
                        _ => None,
                    })
                    .unwrap_or(true),
            });
        }
        Ok(ComponentLight { light_map, shadow })
    }

    /// The key of export `index` of `pkg`, which must be of class `class`; recorded with its holder.
    fn texture(
        &mut self,
        pkg: &Package,
        package: ow_package::PackageId,
        index: i32,
        class: &str,
    ) -> Result<String, String> {
        let found = class_of(pkg, index)?;
        if found != class {
            return Err(format!("a {class} reference to a {found}"));
        }
        let key = format!(
            "{KEY_PREFIX}{}.{}",
            pkg.name,
            pkg.path_of(index).map_err(|e| e.to_string())?
        );
        if !self.textures.contains_key(key.as_str()) {
            let holder = Holder {
                package,
                file: pkg.name.clone(),
                export: index,
            };
            self.textures.insert(key.clone(), (found, holder));
        }
        Ok(key)
    }

    /// Decodes lighting texture `key` through `ow_assets::texture::read_lighting_texture`.
    pub fn decode(
        &self,
        defs: &DefStore,
        scene: &Scene,
        key: &str,
    ) -> Result<ow_assets::texture::TextureAsset, String> {
        let (_, holder) = self
            .textures
            .get(key)
            .ok_or_else(|| format!("{key} is not a lighting texture of the scene"))?;
        ow_assets::texture::read_lighting_texture(
            defs.packages().get(holder.package),
            holder.export,
            PropertyPrefix::Ordinary.bytes(),
            &NoArrayTypes,
            &scene.assets.tfc_root,
        )
        .map_err(|e| e.to_string())
    }
}

/// The class name of export `index` (an export of a class the file imports or exports).
fn class_of(pkg: &Package, index: i32) -> Result<String, String> {
    if index <= 0 {
        return Err(format!("reference {index} is not an export"));
    }
    let object = pkg.object(index).map_err(|e| e.to_string())?;
    if object.class == 0 {
        return Ok("Class".into());
    }
    Ok(pkg
        .object(object.class)
        .map_err(|e| e.to_string())?
        .name
        .clone())
}

fn int_of(entries: &[PropEntry], name: &str) -> Option<i32> {
    entries
        .iter()
        .find_map(|e| match (e.name.as_str(), &e.value) {
            (n, PropValue::Int(i)) if n == name => Some(*i),
            _ => None,
        })
}

fn struct_fields<'a>(entries: &'a [PropEntry], name: &str) -> Option<&'a [PropEntry]> {
    entries
        .iter()
        .find_map(|e| match (e.name.as_str(), &e.value) {
            (n, PropValue::Struct { fields, .. }) if n == name => Some(fields.as_slice()),
            _ => None,
        })
}

fn float_of(entries: &[PropEntry], name: &str) -> Option<f32> {
    entries
        .iter()
        .find_map(|e| match (e.name.as_str(), &e.value) {
            (n, PropValue::Float(f)) if n == name => Some(*f),
            _ => None,
        })
}

/// A `Vector2D` struct property's (X, Y).
fn vector2_of(entries: &[PropEntry], name: &str) -> Option<[f32; 2]> {
    let f = struct_fields(entries, name)?;
    Some([float_of(f, "X")?, float_of(f, "Y")?])
}

/// A `Guid` struct property's (A, B, C, D).
pub fn guid_of(entries: &[PropEntry], name: &str) -> Option<[i32; 4]> {
    let f = struct_fields(entries, name)?;
    Some([
        int_of(f, "A")?,
        int_of(f, "B")?,
        int_of(f, "C")?,
        int_of(f, "D")?,
    ])
}

#[cfg(test)]
mod tests;
