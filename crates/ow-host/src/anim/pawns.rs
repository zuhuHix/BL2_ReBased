//! Owner: lane H-PAWNS (part 1). Third-person skinned pawns in the host: a pawn archetype's mesh, AnimSets and
//! AnimTree read through the definition store, the mesh and clips from UModel's ActorX exports under `local/` (as
//! H6 stage 1), posed by H7's AnimTree idle, skinned by H6-SKIN's GPU path, and drawn as world objects: lit by the
//! scene's `Lighting` and depth-tested inside the scene pass (`crate::render::SceneRenderer::encode_with`).
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-77-*`.
//!
//! **What is read** (all from the archetype, class defaults applied by the store): the pawn's `Mesh` component; from
//! it `SkeletalMesh`, `AnimSets`, `AnimTreeTemplate`, and its `Translation`, `Rotation`, `Scale` and `Scale3D`, which
//! place the mesh relative to the pawn ([`PawnDef`]).
//!
//! **Placement.** The caller gives the pawn's location and yaw (H-WORLD will give the spawned actor's). The mesh's
//! world transform is the component's, then the actor's (`crate::frame::component_to_world`, H3's composition, the
//! one confirmed against the running engine for static components). Applying it to a pawn's skeletal component is
//! UNVERIFIED.
//!
//! **Pose.** The tree with its default inputs plus physics set to walking (`select["physics"] = 1`), as H7's report
//! drives Maya's body tree; every `NotYet` node is passed to its first child and counted ([`PawnCensus`]). The clock
//! is H6-SKIN's: step-sized chunks of the host's frame-time clock (`super::arms::whole_steps`), shared by all pawns.
//!
//! **Look.** A flat colour per pawn (a host choice; H8 owns materials), the scene's sun and sky by the scene
//! shader's rule, no light map (UE3 lights dynamic actors with light environments, not read). Depth-tested and
//! depth-writing with the scene's opaque batches, before its blended ones.

use std::path::PathBuf;
use std::time::Duration;

use ow_assets::skeletal::{SkeletalMesh, read_psk_file};
use ow_core::units::Rotator;
use ow_core::{DetMap, ObjectPath};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;

use super::arms::whole_steps;
use super::gpu::{SkinnedMesh, bind_normals};
use super::pose::{Pose, Skeleton};
use super::skin::skin_matrices;
use super::tree::eval::DriverState;
use super::tree::report::{library, locate};
use super::tree::{ClipLibrary, Driver, Inputs, Kind, TreeDef, Unsupported, load_tree};
use crate::frame::{Affine, Mat4, Placement};
use crate::render::Lighting;

/// A pawn this host knows how to draw: its archetype and the flat colour it is drawn in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PawnKind {
    pub name: &'static str,
    pub archetype: &'static str,
    /// Linear RGB (a host choice).
    pub color: [f32; 3],
}

/// The pawns `--pawn` names: the stock adult bullymong (the archetype the Phaselock work read: its balance names it)
/// and Maya's third-person body.
pub const KINDS: [PawnKind; 2] = [
    PawnKind {
        name: "bullymong",
        archetype: "GD_PrimalBeastAdult.Character.Pawn_PrimalBeastAdult",
        color: [0.50, 0.38, 0.26],
    },
    PawnKind {
        name: "maya",
        archetype: "GD_Siren_Streaming.Pawn_Siren",
        color: [0.42, 0.40, 0.46],
    },
];

/// The kind called `name`.
pub fn kind(name: &str) -> Option<PawnKind> {
    KINDS
        .iter()
        .copied()
        .find(|k| k.name.eq_ignore_ascii_case(name))
}

/// What a pawn archetype names for its look (module notes).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct PawnDef {
    pub archetype: String,
    pub component: String,
    pub mesh: String,
    pub anim_sets: Vec<String>,
    pub tree: String,
    /// The component's placement relative to the pawn.
    pub component_placement: Placement,
}

fn bad(what: &str, why: impl std::fmt::Display) -> String {
    format!("{what}: {why}")
}

/// Property `name` of `o`, or `None` where the class declares none.
fn read(space: &ObjectSpace, o: ObjRef, name: &str) -> Option<Value> {
    let class = space.heap.get(o).ok()?.class;
    let p = space.prop(class, name)?;
    space.read(o, PropAddr::scalar(p)).ok().cloned()
}

fn field<'v>(space: &ObjectSpace, v: &'v Value, name: &str) -> Option<&'v Value> {
    let s = v.as_struct()?;
    let layout = space.classes.struct_layout(s.ty)?;
    let (entry, _) = layout.field(space.names.find(name)?)?;
    s.fields.get(usize::try_from(entry).ok()?)
}

/// The object path of a loaded or referenced object (its export's path in its file).
fn path_of(defs: &DefStore, space: &ObjectSpace, o: ObjRef) -> Option<String> {
    match space.heap.get(o).ok()?.origin {
        Origin::Export { package, index } | Origin::Resource { package, index } => {
            defs.packages().get(package).path_of(index).ok()
        }
        _ => None,
    }
}

fn object(
    defs: &mut DefStore,
    space: &mut ObjectSpace,
    o: ObjRef,
    name: &str,
) -> Result<ObjRef, String> {
    match read(space, o, name) {
        Some(Value::Object(Some(r))) => {
            defs.ensure_loaded(space, r).map_err(|e| bad(name, e))?;
            Ok(r)
        }
        Some(Value::Object(None)) => Err(format!("{name} is None")),
        other => Err(format!(
            "{name} is {:?}, not an object",
            other.map(|v| v.kind_name())
        )),
    }
}

fn vector(
    space: &ObjectSpace,
    o: ObjRef,
    name: &str,
    default: [f32; 3],
) -> Result<[f32; 3], String> {
    let Some(v) = read(space, o, name) else {
        return Ok(default);
    };
    let mut out = [0.0f32; 3];
    for (slot, axis) in out.iter_mut().zip(["X", "Y", "Z"]) {
        *slot = match field(space, &v, axis) {
            Some(Value::Float(f)) if f.is_finite() => *f,
            _ => return Err(format!("{name}.{axis} is not a finite float")),
        };
    }
    Ok(out)
}

/// Reads `archetype`'s look through `defs` (module notes).
pub fn read_pawn(
    defs: &mut DefStore,
    space: &mut ObjectSpace,
    archetype: &str,
) -> Result<PawnDef, String> {
    let path =
        ObjectPath::parse(archetype).map_err(|_| format!("{archetype} is not an object path"))?;
    let pawn = defs
        .find(space, &path)
        .map_err(|e| bad(archetype, e))?
        .ok_or_else(|| format!("no package holds {archetype}"))?;
    let component = object(defs, space, pawn, "Mesh")?;
    let mesh = object(defs, space, component, "SkeletalMesh")?;
    let tree = object(defs, space, component, "AnimTreeTemplate")?;
    let mut anim_sets = Vec::new();
    if let Some(Value::Array(items)) = read(space, component, "AnimSets") {
        for item in items.iter() {
            if let Value::Object(Some(r)) = item {
                anim_sets.push(path_of(defs, space, *r).ok_or("an AnimSet has no export path")?);
            }
        }
    }
    let rotation = match read(space, component, "Rotation") {
        Some(v) => {
            let axis = |n: &str| match field(space, &v, n) {
                Some(Value::Int(i)) => Ok(*i),
                _ => Err(format!("Rotation.{n} is not an int")),
            };
            Rotator::new(axis("Pitch")?, axis("Yaw")?, axis("Roll")?)
        }
        None => Rotator::default(),
    };
    let scale = match read(space, component, "Scale") {
        Some(Value::Float(f)) if f.is_finite() => f,
        None => 1.0,
        _ => return Err("Scale is not a finite float".into()),
    };
    let component_placement = Placement::new(
        vector(space, component, "Translation", [0.0; 3])?,
        rotation,
        scale,
        vector(space, component, "Scale3D", [1.0; 3])?,
    );
    let name = |o| path_of(defs, space, o).unwrap_or_default();
    Ok(PawnDef {
        archetype: archetype.to_string(),
        component: name(component),
        mesh: name(mesh),
        anim_sets,
        tree: name(tree),
        component_placement,
    })
}

/// The tree's classes: evaluated nodes and `NotYet` ones (passed to their first child), by class.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PawnCensus {
    pub nodes: usize,
    pub evaluated: usize,
    pub not_yet: DetMap<String, usize>,
}

pub fn census(tree: &TreeDef) -> PawnCensus {
    let mut c = PawnCensus {
        nodes: tree.nodes.len(),
        ..PawnCensus::default()
    };
    for n in tree.nodes.values() {
        if matches!(n.kind, Kind::NotYet { .. }) {
            *c.not_yet.entry(n.class.clone()).or_insert(0) += 1;
        } else {
            c.evaluated += 1;
        }
    }
    c
}

/// One pawn, loaded: its look, its tree and clips, and its pose state.
pub struct Pawn {
    pub kind: PawnKind,
    pub def: PawnDef,
    pub mesh: SkeletalMesh,
    pub skeleton: Skeleton,
    pub tree: TreeDef,
    pub clips: ClipLibrary,
    pub normals: Vec<[f32; 3]>,
    pub census: PawnCensus,
    /// Where the caller put it: location (uu) and rotation.
    pub actor: Placement,
    state: Option<DriverState>,
    /// `NotYet` nodes passed through, summed over the evaluations so far.
    pub passed_through: u64,
}

/// The driver's inputs for a pawn (module notes).
pub fn inputs() -> Inputs {
    let mut inputs = Inputs::default();
    inputs.select.insert("physics".into(), 1);
    inputs
}

/// The mesh's short name: the last part of its path (`Char_PrimalBeast.Mesh.Skel_PrimalBeast` -> `Skel_PrimalBeast`).
fn short(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or(path)
}

/// Loads pawn `kind` at `actor` from the exports under `roots` and the store.
pub fn load(
    defs: &mut DefStore,
    space: &mut ObjectSpace,
    kind: PawnKind,
    roots: &[PathBuf],
    actor: Placement,
) -> Result<Pawn, String> {
    let def = read_pawn(defs, space, kind.archetype)?;
    let mesh_file = locate(roots, &format!("SkeletalMesh3/{}.psk", short(&def.mesh)))
        .map_err(|e| e.to_string())?;
    let mesh = read_psk_file(&mesh_file).map_err(|e| bad(&def.mesh, e))?;
    let skeleton = Skeleton::new(&mesh).map_err(|e| bad(&def.mesh, e))?;
    let tree = load_tree(defs, space, &def.tree).map_err(|e| bad(&def.tree, e))?;
    let sets: Vec<&str> = def.anim_sets.iter().map(String::as_str).collect();
    let clips = library(defs, space, &mesh, &sets, roots).map_err(|e| e.to_string())?;
    let normals = bind_normals(&mesh);
    let census = census(&tree);
    let mut pawn = Pawn {
        kind,
        def,
        mesh,
        skeleton,
        tree,
        clips,
        normals,
        census,
        actor,
        state: None,
        passed_through: 0,
    };
    // Evaluate once at load, so a broken set-up fails here and not on the first frame.
    pawn.advance_steps(0, 0.0)?;
    Ok(pawn)
}

impl Pawn {
    /// Ticks the tree `steps` times by `step` seconds and returns the pose.
    pub fn advance_steps(&mut self, steps: u32, step: f64) -> Result<Pose, String> {
        let mut d = match self.state.take() {
            Some(s) => Driver::resume(&self.tree, &self.clips, &self.skeleton, inputs(), s),
            None => Driver::new(&self.tree, &self.clips, &self.skeleton, inputs()),
        }
        .map_err(|e| e.to_string())?;
        d.unsupported = Unsupported::PassFirstChild;
        for _ in 0..steps {
            d.tick(step).map_err(|e| e.to_string())?;
        }
        let locals = d.locals().map_err(|e| e.to_string())?;
        self.passed_through = d.log.passed_through.values().sum();
        // Notifies are reported, not applied (H7); a long session must not keep them.
        d.log.notifies.clear();
        self.state = Some(d.into_state());
        self.skeleton
            .pose_from_locals(locals)
            .map_err(|e| e.to_string())
    }

    /// The mesh's world placement: the component's, then the actor's. `None` when it is not finite.
    pub fn placement(&self) -> Option<Affine> {
        crate::frame::component_to_world(&self.def.component_placement, &self.actor)
    }

    /// The palette into `out`: each bone's skin matrix in model space, in the engine's form.
    pub fn palette_into(&self, pose: &Pose, out: &mut Vec<Mat4>) {
        out.clear();
        out.extend(
            skin_matrices(&self.skeleton, pose)
                .iter()
                .map(Affine::to_engine),
        );
    }
}

/// An actor placement from a location (uu) and a yaw in degrees (the `--pawn` form). The yaw is rounded to the
/// nearest rotator unit; `None` for a non-finite input.
pub fn actor_placement(location: [f32; 3], yaw_degrees: f64) -> Option<Placement> {
    if !(location.iter().all(|c| c.is_finite()) && yaw_degrees.is_finite()) {
        return None;
    }
    let units = ow_core::num::round_half_even(yaw_degrees.rem_euclid(360.0) * 65536.0 / 360.0);
    let yaw = ow_core::num::trunc_i64_wrap_i32(units);
    Some(Placement::new(
        location,
        Rotator::new(0, yaw, 0),
        1.0,
        [1.0; 3],
    ))
}

/// The pawns in the play window: loaded pawns, their GPU meshes, and the shared clock.
pub struct PawnLayer {
    pawns: Vec<(Pawn, Option<SkinnedMesh>)>,
    palette: Vec<Mat4>,
    sim_time: Duration,
    drawn_at: Duration,
    carry: f64,
}

impl PawnLayer {
    pub fn new(pawns: Vec<Pawn>) -> PawnLayer {
        PawnLayer {
            pawns: pawns.into_iter().map(|p| (p, None)).collect(),
            palette: Vec::new(),
            sim_time: Duration::ZERO,
            drawn_at: Duration::ZERO,
            carry: 0.0,
        }
    }

    pub fn len(&self) -> usize {
        self.pawns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pawns.is_empty()
    }

    pub fn attach(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> Result<(), String> {
        for (pawn, gpu) in &mut self.pawns {
            let mut mesh = SkinnedMesh::new(device, format, &pawn.mesh, &pawn.normals)?;
            mesh.set_color(pawn.kind.color);
            *gpu = Some(mesh);
        }
        Ok(())
    }

    pub fn snapshot(&mut self, sim_time: Duration) {
        self.sim_time = sim_time;
    }

    /// Advances every pose, skins each pawn and writes its view; call before the scene pass begins, then
    /// [`PawnLayer::draw`] inside it.
    pub fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view_proj: &Mat4,
        lighting: &Lighting,
    ) -> Result<(), String> {
        let delta = self.sim_time.saturating_sub(self.drawn_at).as_secs_f64();
        self.drawn_at = self.sim_time;
        let step = crate::play::step_seconds();
        let steps = whole_steps(&mut self.carry, delta, step);
        for (pawn, gpu) in &mut self.pawns {
            let pose = pawn.advance_steps(steps, step)?;
            let placement = pawn.placement().ok_or("a pawn's placement is not finite")?;
            pawn.palette_into(&pose, &mut self.palette);
            let gpu = gpu.as_ref().ok_or("the pawns are not attached")?;
            gpu.skin(queue, encoder, &self.palette, &placement.to_engine())?;
            gpu.set_view(queue, view_proj, lighting);
        }
        Ok(())
    }

    /// Draws every pawn into the scene pass.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        for (_, gpu) in &self.pawns {
            if let Some(gpu) = gpu {
                gpu.draw_in(pass);
            }
        }
    }

    /// One line per pawn: what it is, where, and its tree's census.
    pub fn summary(&self) -> Vec<String> {
        self.pawns
            .iter()
            .map(|(p, _)| {
                let not_yet: Vec<String> = p.census.not_yet.iter().map(|(c, n)| format!("{c} x{n}")).collect();
                format!(
                    "pawn {}: {} ({} bones, {} vertices), tree {} ({} nodes, {} evaluated, NotYet passed to the first \
                     child: {})",
                    p.kind.name,
                    p.def.mesh,
                    p.skeleton.bone_count(),
                    p.mesh.vertices.len(),
                    p.def.tree,
                    p.census.nodes,
                    p.census.evaluated,
                    if not_yet.is_empty() { "none".to_string() } else { not_yet.join(", ") }
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
