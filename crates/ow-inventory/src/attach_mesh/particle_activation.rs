//! Owner: lane IMPL-PARTICLE-ACTIVATION. `ParticleSystemComponent.SetTemplate` as `SpawnPickupParticles` runs it, with
//! its two branches (the activation, `ActivateSystem(false)`, and the initialisation), in `ParityMode::Off`. Coded from
//! docs/verification/NATIVE_PARTICLE_ACTIVATION.md alone (#597, accepted at 3df9cd88, with the hub's rulings in its
//! section 7; clean room: lane ANALYST-PARTICLE-ACTIVATION read the executable, this lane did not). It replaces #581's
//! refusals E and F. AI-assisted (Claude Code). **Every rule here is UNVERIFIED in the running game.**
//!
//! **Planned, never applied here.** [`Setup`] runs the note's section 7.3 preflight over an overlay: each planned
//! write is checked against the heap's write rules and recorded, and later reads see it. Nothing touches the heap, the
//! random streams or the store, so the preflight is write-free, load-free and draw-free; the caller applies the
//! writes after the whole plan has passed (`pickup_particles`).
//!
//! **Engine flags** (hub ruling 1 on #597): [`ParticleEngineFlags`], a world service the harness supplies and logs as
//! UNVERIFIED. A world without it is [`FLAGS`] (`NotRead`) before any write. Only a game session is ported: the
//! editor and a non-game session are [`OUTSIDE_SESSION`] (`NotPorted`, an implementer's guard, not a note refusal).
//! The forced lowest LOD and the LOD bias are ported, as the note's section 3 steps 11 and 16 give them.
//!
//! **Kept out, as the rulings say:** `EmitterInstances` stays empty (ruling 4; 4 in the game on the slice);
//! `bUseDelayRange` is [`DELAY_RANGE`] (`NotPorted`, ruling 2: the C runtime's `rand()`, single precision, has no port
//! stream); the conditional component update runs only where the note puts it (ruling 3). An LOD level that would
//! change is [`LOD_CHANGE`] (`NotPorted`, an implementer's guard: the note's step 16 reattach and instance LOD are not
//! modelled). A template object is a class default on the component's outer chain (`Origin::Default` or a
//! `Default__` name); archetypes are not told apart (UNVERIFIED).

use ow_core::{DetMap, NotYet};
use ow_object::heap::Origin;
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{Vm, WorldTimeQuery};
use ow_vm::error::VmError;

use super::{ATTACH, RECORD, REATTACH_OWNER, REATTACH_SCENE, SCENE_LAYER, SceneLayer, declared, is_zero};
use crate::equip::{bad, field, object_of, required};

/// The record of this module's refusals.
pub const RECORD_ACTIVATION: &str = "docs/verification/NATIVE_PARTICLE_ACTIVATION.md";
/// The engine flags not supplied by the world's set-up (hub ruling 1 on #597).
pub const FLAGS: &str = "ParticleSystemComponent.SetTemplate: the engine flags (a harness input the world's set-up did not supply)";
/// The implementer's guard: engine flags this port does not take, the editor or a session that is not a game (the
/// note reads branches for them that are not coded). The forced lowest LOD and the LOD bias are ported (section 3,
/// steps 11 and 16).
pub const OUTSIDE_SESSION: &str =
    "ParticleSystemComponent.SetTemplate: engine flags outside a game session (the editor or a non-game session; not ported)";
pub const PHYSX: &str = "ParticleSystemComponent.SetTemplate: the PhysX-level template swap (the machine's PhysX level)";
pub const INSTANCES_RESET: &str = "ParticleSystemComponent.SetTemplate: the emitter instances' reset";
pub const OLD_AUDIO: &str = "ParticleSystemComponent.SetTemplate: the old template's stop audio events";
pub const LOD_LOCATION: &str =
    "ParticleSystemComponent.ActivateSystem: the LOD for location (the local players' view locations)";
pub const DELAY_RANGE: &str = "ParticleSystemComponent.ActivateSystem: the delay range draw (the C runtime's rand)";
pub const TEMPLATE_AUDIO: &str = "ParticleSystemComponent.ActivateSystem: the template's stop audio events";
pub const WARM_UP: &str = "ParticleSystemComponent.ActivateSystem: the warm-up ticks";
pub const WORLD_TIME: &str = "ParticleSystemComponent.ActivateSystem: the world's time for LastRenderTime";
/// The implementer's guard: an LOD level that would change (step 16's reattach and the instances' LOD).
pub const LOD_CHANGE: &str = "ParticleSystemComponent.ActivateSystem: a change of LOD level (the instances' LOD and its reattach)";
/// The reset's stop-audio step met an old template that is not loaded or a stand-in (review r1 of #609): read only
/// inside that step's gate, so a component the gate skips plans as the note says.
pub const OLD_NOT_LOADED: &str =
    "ParticleSystemComponent.SetTemplate: the old template's stop audio events read a template that is not loaded (a load is a write)";
pub const PARTICLE_LAYER: &str = "a particle simulation layer (the port has none)";
pub const AUDIO_LAYER: &str = "an audio layer (the port has none)";
pub const CAMERA_LAYER: &str = "a camera layer (the port has none)";
pub const CLOCK_LAYER: &str = "a world clock (the port's world has none)";
/// #581's refusal C2.
pub const C2: &str = "the deferred transform update: the owner's component update";
/// #581's refusal D.
pub const D: &str = "the deferred transform update: the component's own transform update";
/// The record of #581's refusals C2 and D.
pub const RECORD_PARTICLES: &str = "docs/verification/NATIVE_SPAWN_PICKUP_PARTICLES.md";
const WHAT: &str = "ParticleSystemComponent.SetTemplate";
/// The raw `LODMethod` byte that is not "automatic" (value names not read; UNVERIFIED).
const LOD_METHOD_FIXED: u8 = 1;

/// The engine flags the two natives test (the note's section 1), a harness input (hub ruling 1 on #597).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticleEngineFlags {
    pub is_game: bool,
    pub is_editor: bool,
    pub particles_allowed: bool,
    pub game_thread_lod: bool,
    pub force_lowest: bool,
    pub lod_bias: i32,
    pub system_detail_mode: u8,
}

impl ParticleEngineFlags {
    /// The values the hub ruled for the slice (UNVERIFIED): a game session, particles allowed, the game-thread LOD
    /// flag set, no forced lowest LOD, LOD bias 0, system detail mode 0.
    pub const SLICE: Self = ParticleEngineFlags {
        is_game: true,
        is_editor: false,
        particles_allowed: true,
        game_thread_lod: true,
        force_lowest: false,
        lod_bias: 0,
        system_detail_mode: 0,
    };
}

/// Records the engine flags in `vm`'s world (world set-up).
pub fn add_particle_engine_flags(vm: &mut Vm, flags: ParticleEngineFlags) -> Result<(), VmError> {
    vm.add_service(flags)?;
    Ok(())
}

/// The kinds the preflight checks a declaration against (the note's section 7.1).
#[derive(Clone, Copy)]
pub(super) enum K {
    Bool,
    Float,
    Int,
    Byte,
    Name,
    /// An object reference whose declared class has this name.
    Obj(&'static str),
    /// A struct with these fields (more allowed).
    StructOf(&'static [(&'static str, K)]),
    /// An array of this kind.
    ArrayOf(&'static K),
    /// An array of anything.
    Array,
}

const POINTER: K = K::StructOf(&[]);
const VECTOR: K = K::StructOf(&[("X", K::Float), ("Y", K::Float), ("Z", K::Float)]);
const ROTATOR: K = K::StructOf(&[("Pitch", K::Int), ("Yaw", K::Int), ("Roll", K::Int)]);

/// The component's declarations (section 7.1, both lists, and #581's steps 6 to 10).
pub(super) const COMPONENT_FIELDS: [(&str, K); 41] = [
    ("bAutoActivate", K::Bool),
    ("bIsActive", K::Bool),
    ("bWasDeactivated", K::Bool),
    ("bJustAttached", K::Bool),
    ("bWasCompleted", K::Bool),
    ("bSuppressSpawning", K::Bool),
    ("bHasBeenActivated", K::Bool),
    ("bLODUpdatePending", K::Bool),
    ("bOverrideLODMethod", K::Bool),
    ("bWarmingUp", K::Bool),
    ("bSkipUpdateDynamicDataDuringTick", K::Bool),
    ("bRecacheViewRelevance", K::Bool),
    ("bSkipSpawnCountCheck", K::Bool),
    ("bIsViewRelevanceDirty", K::Bool),
    ("AudioEnabled", K::Bool),
    ("bStartEventPlayed", K::Bool),
    ("EmitterDelay", K::Float),
    ("AudioEventDelay", K::Float),
    ("AccumTickTime", K::Float),
    ("WarmupTime", K::Float),
    ("LODLevel", K::Int),
    ("LODMethod", K::Byte),
    ("LoopingAkPlayingInfo", K::StructOf(&[("SourceComponent", K::Obj("AkComponent")), ("AkPlayingId", K::Int)])),
    ("EmitterInstances", K::ArrayOf(&POINTER)),
    ("SMComponents", K::Array),
    ("SMMaterialInterfaces", K::Array),
    ("Template", K::Obj("ParticleSystem")),
    ("DetailMode", K::Byte),
    ("HiddenGame", K::Bool),
    ("bIgnoreOwnerHidden", K::Bool),
    ("LastRenderTime", K::Float),
    ("Owner", K::Obj("Actor")),
    ("bAttached", K::Bool),
    ("bNeedsReattach", K::Bool),
    ("bNeedsUpdateTransform", K::Bool),
    ("Scene", POINTER),
    ("bAcceptsLights", K::Bool),
    ("AbsoluteTranslation", K::Bool),
    ("AbsoluteRotation", K::Bool),
    ("AbsoluteScale", K::Bool),
    ("Rotation", ROTATOR),
];

/// The template's declarations (section 7.1).
pub(super) const TEMPLATE_FIELDS: [(&str, K); 18] = [
    ("PhysxParticleSystemRef", K::Obj("ParticleSystem")),
    ("bLoadIfPhysXLevel0", K::Bool),
    ("bLoadIfPhysXLevel1", K::Bool),
    ("bLoadIfPhysXLevel2", K::Bool),
    ("bUseDelayRange", K::Bool),
    ("bSkipSpawnCountCheck", K::Bool),
    ("Delay", K::Float),
    ("DelayLow", K::Float),
    ("fAudioDelaySeconds", K::Float),
    ("WarmupTime", K::Float),
    ("LODMethod", K::Byte),
    ("LODDistances", K::ArrayOf(&K::Float)),
    ("LODSettings", K::ArrayOf(&K::StructOf(&[("bLit", K::Bool)]))),
    ("Emitters", K::Array),
    ("StartAudioEvent", K::Obj("AkEvent")),
    ("StopAudioEvent", K::Obj("AkEvent")),
    ("StartLoopingAudioEvent", K::Obj("AkEvent")),
    ("StopLoopingAudioEvent", K::Obj("AkEvent")),
];

/// The owner's declarations the owner tests read.
pub(super) const OWNER_FIELDS: [(&str, K); 4] =
    [("bStatic", K::Bool), ("bTickIsDisabled", K::Bool), ("bDeleteMe", K::Bool), ("Components", K::Array)];

pub(super) const VECTOR_KIND: K = VECTOR;

fn kind_fits(vm: &Vm, kind: &PropKind, want: K) -> bool {
    let named = |c: ow_object::ids::ClassId, n: &str| vm.space.classes.get_layout(c).is_some_and(|l| l.path().name() == n);
    match (want, kind) {
        (K::Bool, PropKind::Bool) | (K::Float, PropKind::Float) | (K::Int, PropKind::Int) => true,
        (K::Byte, PropKind::Byte { .. }) | (K::Name, PropKind::Name) | (K::Array, PropKind::Array(_)) => true,
        (K::Obj(n), PropKind::Object { class }) => named(*class, n),
        (K::ArrayOf(inner), PropKind::Array(k)) => kind_fits(vm, k, *inner),
        (K::StructOf(fields), PropKind::Struct(s)) => {
            let Some(layout) = vm.space.classes.struct_layout(*s) else { return false };
            fields.iter().all(|(n, k)| {
                vm.space.names.find(n).and_then(|id| layout.field(id)).is_some_and(|(_, d)| kind_fits(vm, &d.kind, *k))
            })
        }
        _ => false,
    }
}

/// Each `(name, kind)` declared on `o` with that kind, else a data error naming the field.
pub(super) fn check(vm: &Vm, o: ObjRef, fields: &[(&str, K)], what: &str) -> Result<(), VmError> {
    for &(name, want) in fields {
        let kind = declared(vm, o, name, what)?;
        if !kind_fits(vm, &kind, want) {
            return Err(bad(what, format!("{name} is declared as {kind:?}, not the kind the note reads")));
        }
    }
    Ok(())
}

/// The plan: the overlay of planned values and the writes in the native's order.
pub(super) struct Setup<'a> {
    pub vm: &'a Vm,
    /// The component as the plan reads it: a held component, or the default object a new one is made from.
    pub c: ObjRef,
    /// Whether `c` stands for a new component (never a template object).
    pub new: bool,
    pub flags: ParticleEngineFlags,
    overlay: DetMap<(ObjRef, &'static str), Value>,
    pub writes: Vec<(ObjRef, &'static str, Value)>,
}

impl<'a> Setup<'a> {
    pub fn new(vm: &'a Vm, c: ObjRef, new: bool, flags: ParticleEngineFlags) -> Self {
        Setup { vm, c, new, flags, overlay: DetMap::new(), writes: Vec::new() }
    }

    /// `name` on `o` as the plan has it so far.
    pub fn get(&self, o: ObjRef, name: &'static str) -> Result<Value, VmError> {
        match self.overlay.get(&(o, name)) {
            Some(v) => Ok(v.clone()),
            None => required(self.vm, o, name, WHAT),
        }
    }

    pub fn flag(&self, o: ObjRef, name: &'static str) -> Result<bool, VmError> {
        match self.get(o, name)? {
            Value::Bool(b) => Ok(b),
            other => Err(bad(WHAT, format!("{name} holds {}, not a bool", other.kind_name()))),
        }
    }

    fn float(&self, o: ObjRef, name: &'static str) -> Result<f32, VmError> {
        match self.get(o, name)? {
            Value::Float(f) => Ok(f),
            other => Err(bad(WHAT, format!("{name} holds {}, not a float", other.kind_name()))),
        }
    }

    fn int(&self, o: ObjRef, name: &'static str) -> Result<i32, VmError> {
        match self.get(o, name)? {
            Value::Int(i) => Ok(i),
            other => Err(bad(WHAT, format!("{name} holds {}, not an int", other.kind_name()))),
        }
    }

    fn byte(&self, o: ObjRef, name: &'static str) -> Result<u8, VmError> {
        match self.get(o, name)? {
            Value::Byte(b) => Ok(b),
            other => Err(bad(WHAT, format!("{name} holds {}, not a byte", other.kind_name()))),
        }
    }

    pub fn object(&self, o: ObjRef, name: &'static str) -> Result<Option<ObjRef>, VmError> {
        object_of(&self.get(o, name)?, WHAT)
    }

    pub fn array(&self, o: ObjRef, name: &'static str) -> Result<Vec<Value>, VmError> {
        match self.get(o, name)? {
            Value::Array(a) => Ok(a.as_ref().clone()),
            other => Err(bad(WHAT, format!("{name} holds {}, not an array", other.kind_name()))),
        }
    }

    /// Plans `o.name := v`: checked against the declaration by the heap's own write rules, and the destination's
    /// slot read, so no write is refused after an earlier one.
    pub fn set(&mut self, o: ObjRef, name: &'static str, v: Value) -> Result<(), VmError> {
        let kind = declared(self.vm, o, name, WHAT)?;
        self.vm
            .space
            .heap
            .check_value(&self.vm.space.classes, &kind, &v)
            .map_err(|e| bad(WHAT, format!("{name} cannot take the planned value: {e}")))?;
        if !self.overlay.contains_key(&(o, name)) {
            required(self.vm, o, name, WHAT)?;
        }
        self.overlay.insert((o, name), v.clone());
        self.writes.push((o, name, v));
        Ok(())
    }

    /// Records `o.name := v` as already planned elsewhere (part 1's attach plan, which checked it).
    pub fn planned(&mut self, o: ObjRef, name: &'static str, v: Value) {
        self.overlay.insert((o, name), v.clone());
        self.writes.push((o, name, v));
    }

    /// A class default (or one of its subobjects) on the component's outer chain (module documentation).
    fn is_template_object(&self) -> Result<bool, VmError> {
        if self.new {
            return Ok(false);
        }
        let mut at = Some(self.c);
        while let Some(o) = at {
            let x = self.vm.space.heap.get(o)?;
            if matches!(x.origin, Origin::Default { .. }) || self.vm.space.names.text(x.name).starts_with("Default__") {
                return Ok(true);
            }
            at = x.outer;
        }
        Ok(false)
    }

    /// The owner test of #581's C1 and C2: `bStatic` or `bTickIsDisabled`, and not `bDeleteMe`.
    fn static_owner(&self, owner: ObjRef) -> Result<bool, VmError> {
        check(self.vm, owner, &OWNER_FIELDS, WHAT)?;
        Ok((self.flag(owner, "bStatic")? || self.flag(owner, "bTickIsDisabled")?) && !self.flag(owner, "bDeleteMe")?)
    }

    /// "Attached" (hub ruling 3): `bAttached`, or in its `Owner`'s `Components`, where part 1's attach put it.
    fn attached(&self) -> Result<bool, VmError> {
        if self.flag(self.c, "bAttached")? {
            return Ok(true);
        }
        let Some(owner) = self.object(self.c, "Owner")? else { return Ok(false) };
        check(self.vm, owner, &OWNER_FIELDS[3..], WHAT)?;
        Ok(self.array(owner, "Components")?.contains(&Value::Object(Some(self.c))))
    }

    /// Part 1's conditional component update (section 2, step 4; section 7.3, step 3): planned once where the note
    /// puts it (hub ruling 3).
    fn conditional_update(&mut self, owner: ObjRef) -> Result<(), VmError> {
        let c = self.c;
        if self.attached()? {
            if self.flag(c, "bNeedsReattach")? {
                if self.object(c, "Owner")?.is_some() {
                    return Err(NotYet::not_read(REATTACH_OWNER, RECORD).into());
                }
                if !is_zero(&self.get(c, "Scene")?, WHAT)? {
                    return Err(NotYet::not_read(REATTACH_SCENE, RECORD).into());
                }
                return Ok(());
            }
            if self.flag(c, "bNeedsUpdateTransform")? {
                // The scene layer's empty transform update (by its role, not read).
                self.set(c, "bNeedsUpdateTransform", Value::Bool(false))?;
            }
            return Ok(());
        }
        if self.vm.service::<SceneLayer>().is_none() {
            return Err(NotYet::layer_absent(ATTACH, SCENE_LAYER, RECORD).into());
        }
        self.set(c, "bNeedsReattach", Value::Bool(false))?;
        self.set(c, "bNeedsUpdateTransform", Value::Bool(false))?;
        self.set(c, "Owner", Value::Object(Some(owner)))
    }

    /// The stop-audio step (section 7.2) on `template` as it is at that moment.
    fn stop_audio(&mut self, template: Option<ObjRef>, text: &'static str, record: &'static str) -> Result<(), VmError> {
        let c = self.c;
        if !self.flag(c, "AudioEnabled")? || !(self.flags.is_game || self.object(c, "Owner")?.is_none()) {
            return Ok(());
        }
        if self.vm.space.heap.get(c)?.destroyed {
            return Ok(());
        }
        // The template the step reads, only now (review r1 of #609): loaded, and its three stop fields declared.
        if let Some(t) = template {
            if self.vm.defs.is_unloaded(&self.vm.space, t) || super::pickup_particles::stand_in(self.vm, t)? {
                return Err(NotYet::not_read(OLD_NOT_LOADED, RECORD_ACTIVATION).into());
            }
            check(self.vm, t, &TEMPLATE_FIELDS[15..], WHAT)?;
        }
        // A recorded looping sound: `AkPlayingId` set (review r1 of #609). The note leaves the test unread ("by its
        // role"); the playing id is what names a playing sound, so that member is the test (UNVERIFIED).
        let info = self.get(c, "LoopingAkPlayingInfo")?;
        let holds = match field(self.vm, &info, "AkPlayingId")? {
            Value::Int(id) => id != 0,
            other => return Err(bad(WHAT, format!("LoopingAkPlayingInfo.AkPlayingId holds {}, not an int", other.kind_name()))),
        };
        let events = match template {
            Some(t) => self.object(t, "StopAudioEvent")?.is_some() || self.object(t, "StopLoopingAudioEvent")?.is_some(),
            None => false,
        };
        if events || holds {
            return Err(NotYet::layer_absent(text, AUDIO_LAYER, record).into());
        }
        let kind = declared(self.vm, c, "LoopingAkPlayingInfo", WHAT)?;
        self.set(c, "bStartEventPlayed", Value::Bool(false))?;
        self.set(c, "LoopingAkPlayingInfo", self.vm.space.classes.zero_value(&kind))
    }

    /// `SetTemplate(t)`, the note's section 2 and section 7.3 steps 1 to 6 (module documentation).
    pub fn set_template(&mut self, t: ObjRef) -> Result<(), VmError> {
        let (c, f) = (self.c, self.flags);
        check(self.vm, c, &COMPONENT_FIELDS, WHAT)?;
        check(self.vm, t, &TEMPLATE_FIELDS, WHAT)?;
        // Step 1, particles not allowed: Template := None and nothing else.
        if !f.particles_allowed {
            return self.set(c, "Template", Value::Object(None));
        }
        // Step 2, the PhysX swap (a game session).
        let level = self.flag(t, "bLoadIfPhysXLevel0")? || self.flag(t, "bLoadIfPhysXLevel1")? || self.flag(t, "bLoadIfPhysXLevel2")?;
        if self.object(t, "PhysxParticleSystemRef")?.is_some() && level {
            return Err(NotYet::not_read(PHYSX, RECORD_ACTIVATION).into());
        }
        let template_object = self.is_template_object()?;
        self.set(c, "bIsViewRelevanceDirty", Value::Bool(true))?;
        // Step 3, the conditional update.
        if !template_object && let Some(owner) = self.object(c, "Owner")? {
            self.conditional_update(owner)?;
        }
        self.set(c, "bWasCompleted", Value::Bool(false))?;
        let old = self.object(c, "Template")?;
        let changed = old != Some(t);
        let (was_active, was_deactivated) = (self.flag(c, "bIsActive")?, self.flag(c, "bWasDeactivated")?);
        // Step 4, the reset.
        if !template_object {
            if !self.array(c, "EmitterInstances")?.is_empty() {
                return Err(NotYet::layer_absent(INSTANCES_RESET, PARTICLE_LAYER, RECORD_ACTIVATION).into());
            }
            if changed {
                self.set(c, "SMComponents", Value::array(Vec::new()))?;
                self.set(c, "SMMaterialInterfaces", Value::array(Vec::new()))?;
            }
            self.set(c, "bIsActive", Value::Bool(false))?;
            self.stop_audio(old, OLD_AUDIO, RECORD_ACTIVATION)?;
        }
        self.set(c, "Template", Value::Object(Some(t)))?;
        let warm_up = self.float(t, "WarmupTime")?;
        self.set(c, "WarmupTime", Value::Float(warm_up))?;
        // Step 6, the choice (values read before the reset cleared bIsActive).
        if !template_object {
            if self.flag(c, "bAutoActivate")? || (was_active && !was_deactivated) {
                self.activate(t)?;
            } else {
                self.initialise(t)?;
            }
        }
        // bAcceptsLights from LODSettings[LODLevel], false out of range.
        let lod = self.int(c, "LODLevel")?;
        let lods = self.array(t, "LODSettings")?;
        let mut lit = Vec::with_capacity(lods.len());
        for entry in &lods {
            match field(self.vm, entry, "bLit")? {
                Value::Bool(b) => lit.push(b),
                other => return Err(bad(WHAT, format!("an LODSettings entry's bLit holds {}, not a bool", other.kind_name()))),
            }
        }
        let lit = usize::try_from(lod).ok().and_then(|i| lit.get(i).copied()).unwrap_or(false);
        self.set(c, "bAcceptsLights", Value::Bool(lit))?;
        // The deferred reattach (#581's step 5.3), when T changed; the port has no render proxy.
        if !template_object && changed {
            self.deferred_reattach()?;
        }
        Ok(())
    }

    /// #581's reattach request (steps 5.3 and 9): C1 with a static owner, `REATTACH_SCENE` with no owner and a scene,
    /// else `bNeedsReattach` := true.
    pub fn deferred_reattach(&mut self) -> Result<(), VmError> {
        let c = self.c;
        match self.object(c, "Owner")? {
            Some(o) if self.static_owner(o)? => return Err(NotYet::not_read(REATTACH_OWNER, RECORD).into()),
            Some(_) => {}
            None => {
                if !is_zero(&self.get(c, "Scene")?, WHAT)? {
                    return Err(NotYet::not_read(REATTACH_SCENE, RECORD).into());
                }
            }
        }
        self.set(c, "bNeedsReattach", Value::Bool(true))
    }

    /// #581's refusals C2 and D for a deferred transform update.
    pub fn transform_refusals(&self) -> Result<(), VmError> {
        match self.object(self.c, "Owner")? {
            Some(o) if self.static_owner(o)? => Err(NotYet::not_read(C2, RECORD_PARTICLES).into()),
            Some(_) => Ok(()),
            None if self.flag(self.c, "bAttached")? => Err(NotYet::not_read(D, RECORD_PARTICLES).into()),
            None => Ok(()),
        }
    }

    /// The LOD for location and its application (section 3, steps 2 and 16): more than one LOD distance needs the
    /// local players' view; with at most one the level is 0, and a level that would change is [`LOD_CHANGE`].
    fn lod_for_location(&mut self, t: ObjRef) -> Result<(), VmError> {
        let distances = self.array(t, "LODDistances")?;
        for d in &distances {
            if !matches!(d, Value::Float(_)) {
                return Err(bad(WHAT, format!("an LODDistances entry holds {}, not a float", d.kind_name())));
            }
        }
        if distances.len() > 1 {
            return Err(NotYet::layer_absent(LOD_LOCATION, CAMERA_LAYER, RECORD_ACTIVATION).into());
        }
        // At most one LOD distance: level 0, applied by step 16.
        self.apply_lod_level(t, 0)
    }

    /// Step 16, applying an LOD level, only when T has LOD distances: the level plus the LOD bias, clamped to 0 .. T's
    /// LOD count − 1 (the count read as the number of `LODDistances`, one per level; UNVERIFIED). A level that differs
    /// from `LODLevel` is [`LOD_CHANGE`]: the reattach and the instances' LOD are not modelled.
    fn apply_lod_level(&mut self, t: ObjRef, level: i32) -> Result<(), VmError> {
        let count = i32::try_from(self.array(t, "LODDistances")?.len()).unwrap_or(i32::MAX);
        if count == 0 {
            return Ok(());
        }
        let target = level.saturating_add(self.flags.lod_bias).clamp(0, count - 1);
        if target != self.int(self.c, "LODLevel")? {
            return Err(NotYet::not_ported(LOD_CHANGE, RECORD_ACTIVATION).into());
        }
        Ok(())
    }

    /// `LastRenderTime` := the world's time (section 3, step 15).
    fn last_render_time(&mut self) -> Result<(), VmError> {
        if self.vm.service::<WorldTimeQuery>().is_none() {
            return Err(NotYet::layer_absent(WORLD_TIME, CLOCK_LAYER, RECORD_ACTIVATION).into());
        }
        let time = self.vm.world_time_seconds()?;
        self.set(self.c, "LastRenderTime", Value::Float(time))
    }

    /// The activation, `ActivateSystem(false)` (section 3; section 7.3, step 6).
    pub fn activate(&mut self, t: ObjRef) -> Result<(), VmError> {
        let (c, f) = (self.c, self.flags);
        // The DetailMode skip: only step 15.
        if self.byte(c, "DetailMode")? > f.system_detail_mode {
            return self.last_render_time();
        }
        if f.game_thread_lod {
            self.lod_for_location(t)?;
        }
        if self.flag(t, "bUseDelayRange")? {
            return Err(NotYet::not_ported(DELAY_RANGE, RECORD_ACTIVATION).into());
        }
        let (delay, audio_delay) = (self.float(t, "Delay")?, self.float(t, "fAudioDelaySeconds")?);
        self.set(c, "EmitterDelay", Value::Float(delay))?;
        self.set(c, "AudioEventDelay", Value::Float(audio_delay))?;
        self.stop_audio(Some(t), TEMPLATE_AUDIO, RECORD_ACTIVATION)?;
        let w = self.flag(c, "bWasDeactivated")?;
        for name in ["bWasCompleted", "bSuppressSpawning", "bWasDeactivated"] {
            self.set(c, name, Value::Bool(false))?;
        }
        self.set(c, "bIsActive", Value::Bool(true))?;
        // Step 6, no render proxy: the deferred transform update.
        self.set(c, "bNeedsUpdateTransform", Value::Bool(true))?;
        self.transform_refusals()?;
        // Step 7: no instances yet (the reset refused any), so the initialisation, once.
        self.initialise(t)?;
        self.set(c, "bHasBeenActivated", Value::Bool(true))?;
        // Step 10, the owner part.
        match self.object(c, "Owner")? {
            Some(owner) => {
                if w {
                    self.set(c, "bNeedsUpdateTransform", Value::Bool(true))?;
                }
                self.conditional_update(owner)?;
            }
            None if w && self.flag(c, "bAttached")? => return Err(NotYet::not_read(D, RECORD_PARTICLES).into()),
            None => {}
        }
        // Step 11, the LOD level. With the forced lowest LOD: bOverrideLODMethod and LODMethod 1, and the level T's
        // LOD count − 1, through step 16. Otherwise "automatic" by the raw byte, and the level for location.
        if f.force_lowest {
            self.set(c, "bOverrideLODMethod", Value::Bool(true))?;
            self.set(c, "LODMethod", Value::Byte(LOD_METHOD_FIXED))?;
            let count = i32::try_from(self.array(t, "LODDistances")?.len()).unwrap_or(i32::MAX);
            self.apply_lod_level(t, count.saturating_sub(1))?;
        } else {
            let automatic = if self.flag(c, "bOverrideLODMethod")? {
                self.byte(c, "LODMethod")? != LOD_METHOD_FIXED
            } else {
                self.byte(t, "LODMethod")? != LOD_METHOD_FIXED
            };
            if automatic && f.game_thread_lod {
                self.lod_for_location(t)?;
            }
        }
        // Step 12, the warm-up.
        if self.float(c, "WarmupTime")? != 0.0 {
            return Err(NotYet::layer_absent(WARM_UP, PARTICLE_LAYER, RECORD_ACTIVATION).into());
        }
        self.set(c, "AccumTickTime", Value::Float(0.0))?;
        self.last_render_time()
    }

    /// The initialisation (section 4; section 7.3, step 6): `EmitterInstances` stays empty (hub ruling 4).
    pub fn initialise(&mut self, t: ObjRef) -> Result<(), VmError> {
        let c = self.c;
        if self.byte(c, "DetailMode")? > self.flags.system_detail_mode {
            return Ok(());
        }
        let (warm_up, skip) = (self.float(t, "WarmupTime")?, self.flag(t, "bSkipSpawnCountCheck")?);
        self.set(c, "WarmupTime", Value::Float(warm_up))?;
        self.set(c, "bSkipSpawnCountCheck", Value::Bool(skip))?;
        if self.array(c, "EmitterInstances")?.is_empty() {
            self.set(c, "SMComponents", Value::array(Vec::new()))?;
            self.set(c, "SMMaterialInterfaces", Value::array(Vec::new()))?;
        }
        self.set(c, "bWasCompleted", Value::Bool(false))?;
        if self.attached()? {
            self.set(c, "AccumTickTime", Value::Float(0.0))?;
            if !self.flag(c, "bIsActive")? && self.flag(c, "bAutoActivate")? && !self.flag(c, "bWasDeactivated")? {
                self.activate(t)?;
            }
        }
        Ok(())
    }
}

/// The engine flags from the world, else [`FLAGS`]; flags this port does not take are [`OUTSIDE_SESSION`].
pub fn engine_flags(vm: &Vm) -> Result<ParticleEngineFlags, VmError> {
    let flags = match vm.service::<ParticleEngineFlags>() {
        Some(f) => *f.try_borrow().map_err(|_| VmError::ServiceBorrowed("ParticleEngineFlags"))?,
        None => return Err(NotYet::not_read(FLAGS, RECORD_ACTIVATION).into()),
    };
    if !flags.is_game || flags.is_editor {
        return Err(NotYet::not_ported(OUTSIDE_SESSION, RECORD_ACTIVATION).into());
    }
    Ok(flags)
}
