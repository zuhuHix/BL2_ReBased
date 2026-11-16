//! Owner: lane H8-MATERIALS. The scene's light as its packages give it: the dominant directional light (its actor's
//! rotation, its component's colour and brightness), the sky light (upper and lower colours and brightnesses), and a
//! census of the lightmap and shadow-map exports the levels hold.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-58-*`.
//!
//! **Where the reads come from.** `tools/prepare_level.py` `Scene.lighting_manifest`:
//! - the directional light is the level's `Engine.DominantDirectionalLight` actor (read at the placed-actor prefix
//!   through the definition store) and its `DominantDirectionalLightComponent`, whose tags do not start at a
//!   component's 8 bytes: serialized light data comes first. The tool finds them with `anchored_properties`: every
//!   position of the `LightmassSettings` name in the export's bytes (`property_name_offsets`) is tried as a tag
//!   stream start, and exactly one must parse with `LightmassSettings` first, hold `Brightness` and `LightColor`, and
//!   end at the export's end. [`anchored_tags`] is that rule; no other offset is searched;
//! - the sky light is the `SkyLightComponent` of the level's sky light actor, read at 8 through the store, its class
//!   defaults applied (the tool notes `Brightness` is not serialized on Sanctuary's: the default applies here).
//!
//! A light's direction is its actor's forward axis ([`crate::frame::rotator_forward`]): UE3 lights shine along it, so
//! the direction toward the light is its negative. `FColor` bytes become linear colour as UE3's `FLinearColor` does it,
//! each byte over 255 to the power 2.2 (public UE3 behaviour, UNVERIFIED for BL2).
//!
//! **Lightmaps** are counted, not decoded: `Engine.LightMapTexture2D` (and the shadow maps) are not classes the
//! `ow-assets` texture reader takes ([`LightmapCensus::decodable`] says what it answered for the first one), and the
//! components' lightmap data and UV placement are in untagged bytes no reader here knows.
//!
//! Status: UNVERIFIED as the game's lighting: the values are the packages', how BL2 combines them with its lightmaps
//! and post process is not read.

use ow_core::units::Rotator;
use ow_defs::DefStore;
use ow_object::heap::ObjectSpace;
use ow_object::prefix::PropertyPrefix;
use ow_object::value::Value;
use ow_package::package::Package;
use ow_package::props::{self, NoArrayTypes, PropEntry, PropValue};

use crate::frame::{self, Vec3};
use crate::scene::{Scene, read, read_float, read_rotator, struct_field};

/// The tag the directional light component's stream starts with (`prepare_level.py`).
pub const DIRECTIONAL_ANCHOR: &str = "LightmassSettings";

/// The dominant directional light.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Sun {
    /// The actor's path in its level's file, and its level.
    pub path: String,
    pub level: String,
    pub rotation: Rotator,
    /// Linear colour.
    pub color: [f32; 3],
    pub brightness: f32,
    /// Where the component's tags start in its export.
    pub tags_at: usize,
}

impl Sun {
    /// The unit direction toward the light: the actor's backward axis.
    pub fn toward(&self) -> Vec3 {
        frame::rotator_forward(self.rotation).map(|v| -v)
    }
}

/// The sky light.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct SkyLight {
    pub path: String,
    pub level: String,
    /// Linear colours and brightnesses of the upper and lower hemispheres.
    pub color: [f32; 3],
    pub brightness: f32,
    pub lower_color: [f32; 3],
    pub lower_brightness: f32,
}

/// Lightmap and shadow-map exports, counted over the scene's levels.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LightmapCensus {
    pub light_map_textures: usize,
    pub shadow_maps_2d: usize,
    pub shadow_maps_1d: usize,
    /// What the `ow-assets` texture reader answered for the first `LightMapTexture2D`: `Ok(())` decoded, else its
    /// error.
    pub decodable: Option<Result<(), String>>,
}

/// The scene's light (module documentation).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SceneLights {
    /// The first of each, in level then export order, and how many there are.
    pub sun: Option<Sun>,
    pub suns: usize,
    pub sky: Option<SkyLight>,
    pub skies: usize,
    pub lightmaps: LightmapCensus,
    /// What could not be read, one line each.
    pub errors: Vec<String>,
}

/// An `FColor` (bytes) as linear colour: each channel over 255 to the power 2.2.
pub fn linear_color(rgb: [u8; 3]) -> [f32; 3] {
    rgb.map(|c| ow_core::num::pow_f32(f32::from(c) / 255.0, 2.2))
}

impl SceneLights {
    /// Reads the lights of `scene`'s levels through `defs` and `space` (the store and space the scene was loaded
    /// with).
    pub fn read(defs: &mut DefStore, space: &mut ObjectSpace, scene: &Scene) -> SceneLights {
        let mut lights = SceneLights::default();
        for level in &scene.levels {
            let package = level.package;
            let count = defs.packages().get(package).exports.len();
            for index in 1..=i32::try_from(count).unwrap_or(i32::MAX) {
                let (class, path) = {
                    let pkg = defs.packages().get(package);
                    let Ok(entry) = pkg.object(index) else {
                        continue;
                    };
                    let class = if entry.class == 0 {
                        String::from("Class")
                    } else {
                        pkg.path_of(entry.class).unwrap_or_default()
                    };
                    (class, pkg.path_of(index).unwrap_or_default())
                };
                match class.as_str() {
                    "Engine.LightMapTexture2D" => {
                        lights.lightmaps.light_map_textures += 1;
                        if lights.lightmaps.decodable.is_none() {
                            let pkg = defs.packages().get(package);
                            let tried = ow_assets::texture::read_texture(
                                pkg,
                                index,
                                PropertyPrefix::Ordinary.bytes(),
                                &NoArrayTypes,
                                &scene.assets.tfc_root,
                            );
                            lights.lightmaps.decodable =
                                Some(tried.map(|_| ()).map_err(|e| e.to_string()));
                        }
                    }
                    "Engine.ShadowMap2D" => lights.lightmaps.shadow_maps_2d += 1,
                    "Engine.ShadowMap1D" => lights.lightmaps.shadow_maps_1d += 1,
                    "Engine.DominantDirectionalLight" => {
                        lights.suns += 1;
                        if lights.sun.is_none() {
                            match read_sun(defs, space, package, index) {
                                Ok((rotation, color, brightness, tags_at)) => {
                                    lights.sun = Some(Sun {
                                        path,
                                        level: level.name.clone(),
                                        rotation,
                                        color,
                                        brightness,
                                        tags_at,
                                    });
                                }
                                Err(e) => lights.errors.push(format!("{}:{path}: {e}", level.name)),
                            }
                        }
                    }
                    "Engine.SkyLightComponent" if path.starts_with("TheWorld.PersistentLevel.") => {
                        lights.skies += 1;
                        if lights.sky.is_none() {
                            match read_sky(defs, space, package, index) {
                                Ok(sky) => {
                                    lights.sky = Some(SkyLight {
                                        path,
                                        level: level.name.clone(),
                                        ..sky
                                    })
                                }
                                Err(e) => lights.errors.push(format!("{}:{path}: {e}", level.name)),
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        lights
    }
}

/// The directional light actor `index`'s rotation, and its component's linear colour, brightness and tag start.
fn read_sun(
    defs: &mut DefStore,
    space: &mut ObjectSpace,
    package: u32,
    index: i32,
) -> Result<(Rotator, [f32; 3], f32, usize), String> {
    let actor = defs
        .load_export(space, package, index, PropertyPrefix::PlacedActor)
        .map_err(|e| e.to_string())?;
    let rotation = read_rotator(space, actor, "Rotation")?;
    let pkg = defs.packages().get(package);
    let component = (1..=i32::try_from(pkg.exports.len()).unwrap_or(i32::MAX))
        .find(|&i| {
            pkg.object(i).is_ok_and(|e| e.outer == index)
                && pkg
                    .object(i)
                    .ok()
                    .and_then(|e| pkg.path_of(e.class).ok())
                    .as_deref()
                    == Some("Engine.DominantDirectionalLightComponent")
        })
        .ok_or("no DominantDirectionalLightComponent inside the actor")?;
    let (tags_at, entries) = anchored_tags(
        pkg,
        component,
        DIRECTIONAL_ANCHOR,
        &["Brightness", "LightColor"],
    )?;
    let brightness = entries
        .iter()
        .find_map(|e| match (e.name.as_str(), &e.value) {
            ("Brightness", PropValue::Float(f)) => Some(*f),
            _ => None,
        })
        .ok_or("Brightness is not a float")?;
    let color = entries
        .iter()
        .find_map(|e| match (e.name.as_str(), &e.value) {
            ("LightColor", PropValue::Struct { fields, .. }) => color_fields(fields),
            _ => None,
        })
        .ok_or("LightColor is not a colour")?;
    Ok((rotation, linear_color(color), brightness, tags_at))
}

/// The `R`, `G`, `B` bytes of a decoded `Color` struct's fields.
fn color_fields(fields: &[PropEntry]) -> Option<[u8; 3]> {
    let byte = |name: &str| {
        fields
            .iter()
            .find_map(|f| match (f.name.as_str(), &f.value) {
                (n, PropValue::Byte(b)) if n == name => Some(*b),
                _ => None,
            })
    };
    Some([byte("R")?, byte("G")?, byte("B")?])
}

/// `prepare_level.py` `anchored_properties`: the one start, among the positions of name `anchor` (number 0) in export
/// `index`'s bytes, from which the tag stream parses with `anchor` first, holds every name of `required`, and ends at
/// the export's end. `Err` unless exactly one start does.
pub fn anchored_tags(
    pkg: &Package,
    index: i32,
    anchor: &str,
    required: &[&str],
) -> Result<(usize, Vec<PropEntry>), String> {
    let object = pkg.object(index).map_err(|e| e.to_string())?;
    let offset = usize::try_from(object.offset).map_err(|_| "a negative export offset")?;
    let size = usize::try_from(object.size).map_err(|_| "a negative export size")?;
    let payload = pkg
        .data
        .get(offset..offset.checked_add(size).ok_or("export end overflows")?)
        .ok_or("the export's bytes are past the data")?;
    let mut candidates = Vec::new();
    for (name_index, name) in pkg.names.iter().enumerate() {
        if name != anchor {
            continue;
        }
        let mut marker = i32::try_from(name_index)
            .map_err(|_| "a name index past i32")?
            .to_le_bytes()
            .to_vec();
        marker.extend_from_slice(&0i32.to_le_bytes());
        for start in 0..payload.len().saturating_sub(marker.len() - 1) {
            if payload[start..].starts_with(&marker)
                && let Ok(tagged) = props::read_tagged(pkg, index, start, &NoArrayTypes)
                && tagged.end == size
                && tagged.entries.first().map(|e| e.name.as_str()) == Some(anchor)
                && required
                    .iter()
                    .all(|r| tagged.entries.iter().any(|e| e.name == *r))
            {
                candidates.push((start, tagged.entries));
            }
        }
    }
    match candidates.len() {
        1 => Ok(candidates.remove(0)),
        n => Err(format!("{n} complete {anchor} tag streams, not one")),
    }
}

/// A sky light component's colours and brightnesses, through the store (path and level filled by the caller).
fn read_sky(
    defs: &mut DefStore,
    space: &mut ObjectSpace,
    package: u32,
    index: i32,
) -> Result<SkyLight, String> {
    let o = defs
        .load_export(space, package, index, PropertyPrefix::Component)
        .map_err(|e| e.to_string())?;
    let color = |name: &str| -> Result<[f32; 3], String> {
        let v = read(space, o, name)?.ok_or_else(|| format!("the class declares no {name}"))?;
        let byte = |c: &str| match struct_field(space, v, c) {
            Some(Value::Byte(b)) => Ok(*b),
            _ => Err(format!("{name}.{c} is not a byte")),
        };
        Ok(linear_color([byte("R")?, byte("G")?, byte("B")?]))
    };
    Ok(SkyLight {
        path: String::new(),
        level: String::new(),
        color: color("LightColor")?,
        brightness: read_float(space, o, "Brightness")?,
        lower_color: color("LowerColor")?,
        lower_brightness: read_float(space, o, "LowerBrightness")?,
    })
}

#[cfg(test)]
mod tests;
