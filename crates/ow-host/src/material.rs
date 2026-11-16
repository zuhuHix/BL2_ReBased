//! Owner: lane H8-MATERIALS. What a scene's materials say about how to draw them, read from the packages after
//! [`Scene::load`]: the base material's blend mode, lighting model, two-sidedness and opacity mask clip value, the
//! textures its cooked resource lists, its expression classes, the chain's named scalars, the diffuse source of a
//! material that names no diffuse by parameter, and the sky dome's inputs.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-58-*`.
//!
//! **Where the rules come from.** Every rule is `tools/prepare_level.py`'s, cited at its use:
//! - blend mode, lighting model and `TwoSided` (`Scene.material_metadata`), read here from the base `Material` of
//!   the chain (a material instance has no such property of its own in UE3; the tool also reads none from Sanctuary's
//!   instances);
//! - the cooked texture list (`cooked_texture_references`, the recovered 832/46 material resource prefix: two empty
//!   arrays, a resource int, a GUID, a resource int, then the texture-reference array, refused unless the arrays are
//!   empty and the count fits), read from the base material's untagged tail ([`DefStore::raw_tail`]);
//! - the diffuse of a material whose chain names no diffuse by parameter, in the tool's order
//!   (`Scene.material`): a sole unnamed `MaterialExpressionTextureSampleParameter2D_<n>` sample whose texture name
//!   ends in `_Dif` (`unnamed_diffuse_candidate`); else, from the cooked list, a sole `_Dif`/`_Diff` `Texture2D`, or,
//!   for an opaque or masked material only, a sole `Texture2D` whose name marks no auxiliary channel
//!   (`cooked_diffuse_candidate`); else, for an opaque material with an empty cooked list, the constant of an
//!   unconnected `DiffuseColor` input (`unconnected_diffuse_constant`);
//! - the sky dome's inputs (`Scene.native_sky_approximation`, `docs/verification/NATIVE_SKY_APPROXIMATION.md`): for a
//!   chain whose base is [`SKY_MASTER`], the `Transition_Track` strip and the `clouds` texture, and the scalars
//!   `Time_of_Day`, `sky_brightness`, `p_CouldBrightness` and `cloud_cap_opacity`.
//!
//! **Not ported**: the tool's inspected, path-specific rules (its `REGULAR_DIFFUSE_FALLBACKS`,
//! `INSPECTED_COLOR_FALLBACKS`, the glacier surface, scene recipes, tints and emissive calibrations). They name
//! single Sanctuary materials from screenshot inspection; such a material is drawn as the general rules leave it.
//! The tool's `DiffuseColor` rule also needs the input to be serialized; the definition store applies class
//! defaults, so an unserialized input reads as an unconnected one here, with the default's black constant, where the
//! tool takes no rule (UNVERIFIED difference; one Sanctuary material takes the rule in the tool).
//! The scene keeps no null texture override (`Scene::load` drops a `TextureParameterValues` entry whose value is
//! null), so an instance that nulls its parent's diffuse parameter keeps the parent's texture here and does not block
//! the unnamed-sample rule, where the tool's parameter dict holds the null (a known difference, not counted).
//!
//! **Where a node-graph evaluator would fit.** Cooked materials keep only their parameter expressions (their graphs
//! are stripped); [`MaterialInfo::expression_classes`] counts what survives. An evaluator of the surviving graph, or of
//! a hand-authored replacement graph, would replace [`DiffuseSource`] and the sky inputs with a compiled shader per
//! material; the renderer's material slot (`draw::MaterialSlot`) is the seam it would plug into.
//!
//! Status: UNVERIFIED as the game's look; the rules are the tool's, which were checked against host screenshots, not
//! the game.

use ow_core::{ObjectPath, OrderedMap};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_package::PackageId;

use crate::draw::{Channel, channel_for_parameter, diffuse_candidates};
use crate::scene::{
    self, Holder, Scene, SceneMaterial, SceneTexture, read, read_bool, read_float, read_object,
    read_objects, struct_field,
};

/// The stripped master graph the tool approximates as a time-of-day sky (`prepare_level.py` `NATIVE_SKY_MASTER`).
pub const SKY_MASTER: &str = "Common_Materials.Sky.Mat_SkyTimeOfDay_Master";
/// The strip's width: `Time_of_Day / 256` is the column (`prepare_level.py` `SKY_TIME_AXIS`; UNVERIFIED as the
/// graph's mapping, which is stripped).
pub const SKY_TIME_AXIS: f32 = 256.0;
/// The strip row the clouds take their colour from (`prepare_level.py` `SKY_HORIZON_ROW`).
pub const SKY_HORIZON_ROW: f32 = 0.95;
/// The opacity mask clip value of a base material that declares none (UE3's `Material` default, 1/3).
pub const DEFAULT_CLIP: f32 = 0.3333;
/// How many parents a chain may have.
const MAX_CHAIN: usize = 32;

/// A material's blend mode, by its `EBlendMode` name.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Blend {
    #[default]
    Opaque,
    Masked,
    Translucent,
    Additive,
    Modulate,
}

impl Blend {
    /// The blend of an `EBlendMode` value name; `None` for a name this renderer has no rule for.
    pub fn from_name(name: &str) -> Option<Blend> {
        Some(match name {
            "BLEND_Opaque" => Blend::Opaque,
            "BLEND_Masked" => Blend::Masked,
            "BLEND_Translucent" => Blend::Translucent,
            "BLEND_Additive" => Blend::Additive,
            "BLEND_Modulate" => Blend::Modulate,
            _ => return None,
        })
    }

    /// Drawn with depth writes (opaque and masked) or after them, sorted (the others).
    pub fn writes_depth(self) -> bool {
        matches!(self, Blend::Opaque | Blend::Masked)
    }
}

/// Where a material's diffuse comes from.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum DiffuseSource {
    /// Named by a diffuse parameter: the candidates of `draw::diffuse_candidates`.
    Named,
    /// A sole unnamed sample parameter whose texture ends in `_Dif`.
    Unnamed(String),
    /// The sole `_Dif`/`_Diff` `Texture2D` of the cooked list.
    CookedDif(String),
    /// The sole non-auxiliary `Texture2D` of the cooked list (opaque or masked materials only).
    CookedSole(String),
    /// An unconnected `DiffuseColor` input's constant, as linear colour: its `Color` bytes through
    /// [`crate::lighting::linear_color`], as UE3's `FLinearColor` converts an `FColor` (UNVERIFIED for BL2; the tool
    /// takes the bytes over 255).
    Constant([f32; 3]),
    /// None of the rules applies.
    None,
}

impl DiffuseSource {
    /// The texture this source names, if any.
    pub fn texture(&self) -> Option<&str> {
        match self {
            DiffuseSource::Unnamed(t)
            | DiffuseSource::CookedDif(t)
            | DiffuseSource::CookedSole(t) => Some(t),
            _ => None,
        }
    }

    /// The label the report counts by.
    pub fn label(&self) -> &'static str {
        match self {
            DiffuseSource::Named => "named",
            DiffuseSource::Unnamed(_) => "unnamed sample parameter",
            DiffuseSource::CookedDif(_) => "sole cooked _Dif texture",
            DiffuseSource::CookedSole(_) => "sole cooked texture",
            DiffuseSource::Constant(_) => "unconnected DiffuseColor constant",
            DiffuseSource::None => "none",
        }
    }
}

/// The sky dome's inputs (module documentation).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct SkyInputs {
    /// The time-of-day strip (`Transition_Track`).
    pub strip: String,
    /// The cloud texture (`clouds`), sampled as data.
    pub clouds: String,
    pub time_of_day: f32,
    pub sky_brightness: f32,
    pub cloud_brightness: f32,
    pub cloud_cap_opacity: f32,
}

impl SkyInputs {
    /// The strip column the time of day selects (`Time_of_Day / 256`).
    pub fn column(&self) -> f32 {
        self.time_of_day / SKY_TIME_AXIS
    }
}

/// What the packages say about one material.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialInfo {
    /// The chain's base `Material`, if it was reached.
    pub base: Option<String>,
    /// The base's `BlendMode` value name, and the blend it maps to (an unmapped name draws opaque).
    pub blend_name: Option<String>,
    pub blend: Blend,
    /// The base's `LightingModel` value name.
    pub lighting_model: Option<String>,
    pub two_sided: Option<bool>,
    pub opacity_mask_clip: f32,
    /// The base's cooked texture list, by object path.
    pub cooked_textures: Vec<String>,
    /// The class paths of the base's surviving expressions.
    pub expression_classes: Vec<String>,
    /// The chain's named scalars, base defaults first, each nearer level replacing a name in place.
    pub scalars: Vec<(String, f32)>,
    pub diffuse: DiffuseSource,
    pub sky: Option<SkyInputs>,
    /// Why part of the material could not be read; the rest keeps what was read.
    pub error: Option<String>,
}

impl MaterialInfo {
    fn empty() -> MaterialInfo {
        MaterialInfo {
            base: None,
            blend_name: None,
            blend: Blend::Opaque,
            lighting_model: None,
            two_sided: None,
            opacity_mask_clip: DEFAULT_CLIP,
            cooked_textures: Vec::new(),
            expression_classes: Vec::new(),
            scalars: Vec::new(),
            diffuse: DiffuseSource::None,
            sky: None,
            error: None,
        }
    }

    /// `MLM_Unlit`: drawn with its colour as is, no light.
    pub fn unlit(&self) -> bool {
        self.lighting_model.as_deref() == Some("MLM_Unlit")
    }

    /// A named scalar of the chain.
    pub fn scalar(&self, name: &str) -> Option<f32> {
        let folded = name.to_lowercase();
        self.scalars
            .iter()
            .find(|(n, _)| n.to_lowercase() == folded)
            .map(|&(_, v)| v)
    }
}

/// Every scene material's [`MaterialInfo`], and the textures the fallback rules name that the scene does not hold.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MaterialLibrary {
    pub materials: OrderedMap<String, MaterialInfo>,
    /// Textures named only by a cooked list, by object path.
    pub textures: OrderedMap<String, SceneTexture>,
}

impl MaterialLibrary {
    /// Reads every material of `scene` through `defs` and `space` (the store and space the scene was loaded with).
    pub fn read(defs: &mut DefStore, space: &mut ObjectSpace, scene: &Scene) -> MaterialLibrary {
        let mut library = MaterialLibrary::default();
        let classes = (
            defs.class_id(space, "Engine.MaterialInstance"),
            defs.class_id(space, "Engine.Material"),
        );
        for (path, material) in scene.materials.iter() {
            let info = match &classes {
                (Ok(instance), Ok(base)) => {
                    let mut reader = Reader {
                        defs,
                        space,
                        scene,
                        instance: *instance,
                        base: *base,
                        extra: &mut library.textures,
                    };
                    reader.material(path, material)
                }
                (Err(e), _) | (_, Err(e)) => MaterialInfo {
                    error: Some(e.to_string()),
                    ..MaterialInfo::empty()
                },
            };
            library.materials.insert(path.clone(), info);
        }
        library
    }

    /// The class and holder of texture `path`: the scene's, or one a cooked list named.
    pub fn texture(&self, scene: &Scene, path: &str) -> Option<SceneTexture> {
        scene
            .textures
            .get(path)
            .or_else(|| self.textures.get(path))
            .cloned()
    }
}

/// The reads of one library.
struct Reader<'a> {
    defs: &'a mut DefStore,
    space: &'a mut ObjectSpace,
    scene: &'a Scene,
    instance: ClassId,
    base: ClassId,
    extra: &'a mut OrderedMap<String, SceneTexture>,
}

impl Reader<'_> {
    fn material(&mut self, path: &str, material: &SceneMaterial) -> MaterialInfo {
        let mut info = MaterialInfo::empty();
        let base = match self.chain(path, &mut info) {
            Ok(base) => base,
            Err(e) => {
                info.error = Some(e);
                None
            }
        };
        info.diffuse = diffuse_source(material, &info, &|t| self.texture_class(t));
        // `unconnected_diffuse_constant`, last: an opaque material with no diffuse and an empty cooked list.
        if info.diffuse == DiffuseSource::None
            && info.blend == Blend::Opaque
            && info.cooked_textures.is_empty()
            && info.error.is_none()
            && let Some(constant) = base.and_then(|b| unconnected_constant(self.space, b))
        {
            info.diffuse = DiffuseSource::Constant(constant);
        }
        if info.base.as_deref() == Some(SKY_MASTER) {
            info.sky = sky_inputs(material, &info);
        }
        info
    }

    /// The chain from `path` to its base material: the instances' scalars, then the base's properties, expressions
    /// and cooked texture list. The base material's object, when the chain reaches one.
    fn chain(&mut self, path: &str, info: &mut MaterialInfo) -> Result<Option<ObjRef>, String> {
        let object = ObjectPath::parse(path).map_err(|e| e.to_string())?;
        let mut cursor = self
            .defs
            .find(self.space, &object)
            .map_err(|e| e.to_string())?;
        // The instances' scalar levels, nearest first.
        let mut levels: Vec<Vec<(String, f32)>> = Vec::new();
        for _ in 0..MAX_CHAIN {
            let Some(m) = cursor else {
                return Ok(None);
            };
            self.defs
                .ensure_loaded(self.space, m)
                .map_err(|e| e.to_string())?;
            let class = self.space.heap.get(m).map_err(|e| e.to_string())?.class;
            if self.space.classes.is_child_of(class, self.instance) {
                levels.push(self.named_floats(m, "ScalarParameterValues", "ParameterValue")?);
                cursor = read_object(self.space, m, "Parent")?;
            } else if self.space.classes.is_child_of(class, self.base) {
                // Read whole before the error is reported: a refused cooked list must not cost the instances'
                // scalars (the sky's inputs among them).
                let read = self.base_material(m, info);
                // Base defaults first, then each instance from the nearest the base to the material asked for.
                let mut scalars = std::mem::take(&mut info.scalars);
                for level in levels.iter().rev() {
                    for (name, value) in level {
                        let folded = name.to_lowercase();
                        match scalars.iter_mut().find(|(n, _)| n.to_lowercase() == folded) {
                            Some(slot) => slot.1 = *value,
                            None => scalars.push((name.clone(), *value)),
                        }
                    }
                }
                info.scalars = scalars;
                read?;
                return Ok(Some(m));
            } else {
                return Err("a material class outside Material and MaterialInstance".into());
            }
        }
        Err(format!("a chain longer than {MAX_CHAIN} parents"))
    }

    fn base_material(&mut self, m: ObjRef, info: &mut MaterialInfo) -> Result<(), String> {
        info.base = Some(self.path_of(m)?);
        info.blend_name = enum_name(self.space, m, "BlendMode")?;
        info.blend = info
            .blend_name
            .as_deref()
            .and_then(Blend::from_name)
            .unwrap_or(Blend::Opaque);
        info.lighting_model = enum_name(self.space, m, "LightingModel")?;
        info.two_sided = read_bool(self.space, m, "TwoSided").ok();
        info.opacity_mask_clip =
            read_float(self.space, m, "OpacityMaskClipValue").unwrap_or(DEFAULT_CLIP);
        for e in read_objects(self.space, m, "Expressions")?
            .into_iter()
            .flatten()
        {
            let class = self.space.heap.get(e).map_err(|e| e.to_string())?.class;
            info.expression_classes.push(self.class_path(class));
            self.defs
                .ensure_loaded(self.space, e)
                .map_err(|e| e.to_string())?;
            let name = match read(self.space, e, "ParameterName")? {
                Some(Value::Name(n)) => scene::parameter_name(&self.space.names.text(*n)),
                _ => None,
            };
            if let (Some(name), Ok(Some(Value::Float(v)))) =
                (name, read(self.space, e, "DefaultValue"))
            {
                info.scalars.push((name, *v));
            }
        }
        let Some(tail) = self.defs.raw_tail(m) else {
            return Err("the base material has no untagged tail".into());
        };
        let refs = cooked_texture_references(tail.bytes())?;
        for r in refs {
            // Resolved as a package reference (`PackageStore::resolve`, the tool's `--resolve`), not through the
            // store's identity by path: a texture may share its path with a material (Sanctuary's
            // `HangingWires_1A`), and the store would answer the material.
            let Some((package, index)) = self
                .defs
                .code_mut()
                .1
                .resolve(tail.package, r)
                .map_err(|e| e.to_string())?
            else {
                continue;
            };
            let pkg = self.defs.packages().get(package);
            let entry = pkg.object(index).map_err(|e| e.to_string())?;
            // As `Package.Class` (the store's `full_class`): an imported class's path, or `<file>.<path>` for a class
            // the file exports itself (a texture resolved into `Engine.upk`).
            let class = match entry.class {
                0 => String::from("Core.Class"),
                c if c < 0 => pkg.path_of(c).map_err(|e| e.to_string())?,
                c => format!(
                    "{}.{}",
                    pkg.name,
                    pkg.path_of(c).map_err(|e| e.to_string())?
                ),
            };
            let path = scene::full_path(pkg, index).map_err(|e| e.to_string())?;
            let holder = Some(Holder {
                package,
                file: pkg.name.clone(),
                export: index,
            });
            if class != "Engine.Texture2D" && class != "Engine.TextureCube" {
                // The tool refuses the whole list here ("not a supported texture").
                info.cooked_textures.clear();
                return Err(format!("cooked texture {path} is a {class}"));
            }
            if !self.scene.textures.contains_key(path.as_str())
                && !self.extra.contains_key(path.as_str())
            {
                self.extra
                    .insert(path.clone(), SceneTexture { class, holder });
            }
            info.cooked_textures.push(path);
        }
        Ok(())
    }

    /// `(name, value)` of each entry of array-of-struct property `property`: its `ParameterName` and float `field`.
    fn named_floats(
        &self,
        m: ObjRef,
        property: &str,
        field: &str,
    ) -> Result<Vec<(String, f32)>, String> {
        let items = match read(self.space, m, property)? {
            Some(Value::Array(items)) => items.clone(),
            Some(_) => return Err(format!("{property} is not an array")),
            None => return Ok(Vec::new()),
        };
        let mut out = Vec::new();
        for item in items.iter() {
            let name = match struct_field(self.space, item, "ParameterName") {
                Some(Value::Name(n)) => scene::parameter_name(&self.space.names.text(*n)),
                _ => None,
            };
            if let (Some(name), Some(Value::Float(v))) =
                (name, struct_field(self.space, item, field))
            {
                out.push((name, *v));
            }
        }
        Ok(out)
    }

    /// A texture's class path by object path, from the scene or this library (`None` when neither holds it).
    fn texture_class(&self, path: &str) -> Option<String> {
        self.scene
            .textures
            .get(path)
            .or_else(|| self.extra.get(path))
            .map(|t| t.class.clone())
    }

    fn path_of(&self, o: ObjRef) -> Result<String, String> {
        self.locate(o).map(|(path, _, _)| path)
    }

    /// The object path, class path and holder of store object `o` (as `scene`'s loader locates its handles).
    fn locate(&self, o: ObjRef) -> Result<(String, String, Option<Holder>), String> {
        let object = self.space.heap.get(o).map_err(|e| e.to_string())?;
        let class = self.class_path(object.class);
        let (package, index): (PackageId, i32) = match object.origin {
            Origin::Export { package, index } | Origin::Resource { package, index } => {
                (package, index)
            }
            other => return Err(format!("not a package object: {other:?}")),
        };
        let pkg = self.defs.packages().get(package);
        let path = scene::full_path(pkg, index).map_err(|e| e.to_string())?;
        let holder = (index > 0).then(|| Holder {
            package,
            file: pkg.name.clone(),
            export: index,
        });
        Ok((path, class, holder))
    }

    fn class_path(&self, class: ClassId) -> String {
        self.space
            .classes
            .get_layout(class)
            .map(|l| l.path().as_str().to_string())
            .unwrap_or_default()
    }
}

/// Decodes texture `path` for the renderer: through [`Scene::decode_texture`] when the scene holds it, else through
/// the holder a cooked list gave ([`MaterialLibrary::textures`]) with the same reader and checks.
pub fn decode_texture(
    defs: &DefStore,
    scene: &Scene,
    library: &MaterialLibrary,
    path: &str,
) -> Result<ow_assets::texture::TextureAsset, String> {
    if scene.textures.contains_key(path) {
        return scene.decode_texture(defs, path).map_err(|e| e.to_string());
    }
    let texture = library
        .textures
        .get(path)
        .ok_or_else(|| format!("texture {path} is in neither the scene nor the library"))?;
    if !scene::TEXTURE_CLASSES.contains(&texture.class.as_str()) {
        return Err(format!(
            "{path} is a {}, which ow-assets does not read",
            texture.class
        ));
    }
    let holder = texture
        .holder
        .as_ref()
        .ok_or_else(|| format!("no installed package holds {path}"))?;
    ow_assets::texture::read_texture(
        defs.packages().get(holder.package),
        holder.export,
        ow_object::prefix::PropertyPrefix::Ordinary.bytes(),
        &ow_package::props::NoArrayTypes,
        &scene.assets.tfc_root,
    )
    .map_err(|e| e.to_string())
}

/// The name of enum property `name` of `o` (the value's name in the enum the property is declared with); `None`
/// where the class declares no such property.
fn enum_name(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<Option<String>, String> {
    let class = space.heap.get(o).map_err(|e| e.to_string())?.class;
    let Some(p) = space.prop(class, name) else {
        return Ok(None);
    };
    let Some(Value::Byte(n)) = read(space, o, name)? else {
        return Err(format!("{name} is not a byte"));
    };
    let Some(PropKind::Byte { enum_: Some(e) }) = space.classes.decl(p).map(|d| &d.kind) else {
        return Err(format!("{name} is not declared as an enum"));
    };
    let values = space
        .classes
        .enum_layout(*e)
        .ok_or_else(|| format!("{name}'s enum has no layout"))?
        .values();
    let value = values
        .get(usize::from(*n))
        .ok_or_else(|| format!("{name} value {n} is past its enum"))?;
    Ok(Some(space.names.text(*value).to_string()))
}

/// The texture references of a cooked material's untagged tail (`prepare_level.py` `cooked_texture_references`):
/// two empty arrays (error and dependency lists; anything else is refused), a resource int, a GUID and a resource
/// int (36 bytes up to the array), then the texture-reference count and the references. The rest of the tail
/// (shader maps and the like) is not read.
pub fn cooked_texture_references(tail: &[u8]) -> Result<Vec<i32>, String> {
    let word = |at: usize| -> Option<i32> {
        tail.get(at..at + 4)
            .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    if tail.len() < 36 {
        return Err("truncated cooked material resource prefix".into());
    }
    if (word(0), word(4)) != (Some(0), Some(0)) {
        return Err("unsupported cooked material error/dependency arrays".into());
    }
    let count = word(32).ok_or("truncated cooked material resource prefix")?;
    let room = (tail.len() - 36) / 4;
    let count = usize::try_from(count)
        .ok()
        .filter(|&c| c <= room)
        .ok_or("invalid cooked material texture count")?;
    (0..count)
        .map(|i| word(36 + i * 4).ok_or_else(|| "truncated texture reference".to_string()))
        .collect()
}

/// Whether `name` ends with one of `suffixes`, optionally followed by `_<digits>` (the tools' `(?:_\d+)?$`), ASCII
/// case folded.
fn ends_with_suffix(name: &str, suffixes: &[&str]) -> bool {
    let lower = name.to_ascii_lowercase();
    let mut forms = vec![lower.as_str()];
    if let Some(at) = lower.rfind('_') {
        let digits = &lower[at + 1..];
        if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
            forms.push(&lower[..at]);
        }
    }
    forms
        .iter()
        .any(|form| suffixes.iter().any(|s| form.ends_with(s)))
}

/// `prepare_level.py` `DIFFUSE_SUFFIX`: `_dif` or `_diff`, then an optional `_<digits>`.
pub fn is_diffuse_name(path: &str) -> bool {
    ends_with_suffix(path, &["_dif", "_diff"])
}

/// `prepare_level.py` `AUXILIARY_TEXTURE`: a name that marks a normal, packed, mask, noise or utility map.
pub fn is_auxiliary_name(path: &str) -> bool {
    ends_with_suffix(
        path,
        &[
            "_n",
            "_nm",
            "_nrm",
            "normal",
            "_gray",
            "_grey",
            "_hs",
            "_spec",
            "_emis",
            "_emissive",
            "_alpha",
            "_mask",
            "_comp",
            "_lm",
            "noise",
            "_cube",
            "_detail",
            "_rough",
            "_roughness",
            "_height",
            "_bump",
            "_opacity",
            "_illum",
            "_lightmap",
            "_gloss",
            "_metal",
            "_metallic",
            "_ao",
            "_cavity",
            "_disp",
            "_displacement",
            "_refl",
            "_reflection",
            "_env",
        ],
    )
}

/// `materialexpressiontexturesampleparameter2d_<digits>`, the whole name (the tool's `fullmatch`).
fn is_unnamed_sample(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower
        .strip_prefix("materialexpressiontexturesampleparameter2d_")
        .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
}

/// The diffuse source of a material (module documentation), `class_of` giving a texture's class path.
pub fn diffuse_source(
    material: &SceneMaterial,
    info: &MaterialInfo,
    class_of: &dyn Fn(&str) -> Option<String>,
) -> DiffuseSource {
    if !diffuse_candidates(material).is_empty() {
        return DiffuseSource::Named;
    }
    // `unnamed_diffuse_candidate`: the chain's parameter dict (deepest first, nearer replacing), its unnamed sample
    // parameters whose texture ends in `_Dif` (with an optional `_<digits>`); exactly one.
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
        if channel_for_parameter(name).is_none() && !is_unnamed_sample(name) {
            continue;
        }
        let folded = name.to_lowercase();
        match parameters.iter_mut().find(|(n, _)| *n == folded) {
            Some(slot) => slot.1 = t.path.as_str(),
            None => parameters.push((folded, t.path.as_str())),
        }
    }
    let no_diffuse_name = !parameters
        .iter()
        .any(|(n, _)| channel_for_parameter(n) == Some(Channel::Diffuse));
    if no_diffuse_name {
        let mut unnamed: Vec<&str> = parameters
            .iter()
            .filter(|(n, t)| is_unnamed_sample(n) && ends_with_suffix(t, &["_dif"]))
            .map(|&(_, t)| t)
            .collect();
        unnamed.dedup();
        if let [only] = unnamed.as_slice() {
            return DiffuseSource::Unnamed((*only).to_string());
        }
    }
    // `cooked_diffuse_candidate` over the base's cooked list.
    let planar: Vec<&str> = info
        .cooked_textures
        .iter()
        .map(String::as_str)
        .filter(|t| class_of(t).as_deref() == Some("Engine.Texture2D"))
        .collect();
    let mut named: Vec<&str> = planar
        .iter()
        .copied()
        .filter(|t| is_diffuse_name(t))
        .collect();
    named.sort_unstable();
    named.dedup();
    if let [only] = named.as_slice() {
        return DiffuseSource::CookedDif((*only).to_string());
    }
    if named.is_empty() && matches!(info.blend, Blend::Opaque | Blend::Masked) {
        let mut plain: Vec<&str> = planar
            .iter()
            .copied()
            .filter(|t| !is_auxiliary_name(t))
            .collect();
        plain.sort_unstable();
        plain.dedup();
        if let [only] = plain.as_slice() {
            return DiffuseSource::CookedSole((*only).to_string());
        }
    }
    DiffuseSource::None
}

/// `prepare_level.py` `unconnected_diffuse_constant`, for an opaque base with an empty cooked list: the input's
/// `Expression` null and none of its masks set gives its `Constant`.
pub fn unconnected_constant(space: &ObjectSpace, base: ObjRef) -> Option<[f32; 3]> {
    let input = read(space, base, "DiffuseColor").ok()??;
    if let Some(Value::Object(Some(_))) = struct_field(space, input, "Expression") {
        return None;
    }
    for mask in ["Mask", "MaskR", "MaskG", "MaskB", "MaskA"] {
        match struct_field(space, input, mask) {
            Some(Value::Int(0) | Value::Bool(false) | Value::Byte(0)) | None => {}
            Some(_) => return None,
        }
    }
    let constant = struct_field(space, input, "Constant")?;
    let channel = |c: &str| match struct_field(space, constant, c) {
        Some(Value::Byte(b)) => Some(*b),
        _ => None,
    };
    Some(crate::lighting::linear_color([
        channel("R")?,
        channel("G")?,
        channel("B")?,
    ]))
}

/// The sky inputs of a [`SKY_MASTER`] chain, or `None` when one is missing (the tool then keeps the diffuse path).
fn sky_inputs(material: &SceneMaterial, info: &MaterialInfo) -> Option<SkyInputs> {
    let texture = |name: &str| {
        let folded = name.to_lowercase();
        let mut entries: Vec<_> = material
            .textures
            .iter()
            .filter(|t| {
                t.parameter.as_deref().map(str::to_lowercase).as_deref() == Some(folded.as_str())
            })
            .collect();
        // The nearest level wins.
        entries.sort_by_key(|t| t.depth);
        entries.first().map(|t| t.path.clone())
    };
    let finite = |v: Option<f32>| v.filter(|x| x.is_finite());
    let time = finite(info.scalar("Time_of_Day"))?;
    if !(0.0..SKY_TIME_AXIS).contains(&time) {
        return None;
    }
    Some(SkyInputs {
        strip: texture("Transition_Track")?,
        clouds: texture("clouds")?,
        time_of_day: time,
        sky_brightness: finite(info.scalar("sky_brightness"))?,
        cloud_brightness: finite(info.scalar("p_CouldBrightness"))?,
        cloud_cap_opacity: finite(info.scalar("cloud_cap_opacity"))?,
    })
}

#[cfg(test)]
mod tests;
