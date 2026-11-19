//! Owner: lane IMPL-BODY-COMPOSITION (decision 2026-11-19-NNN, number pending). `InstanceDataHelper.InitializeBodyComposition`
//! in `ParityMode::Off`, for the pawn wrapper (`WillowPawn.InitializePawnMeshes`, crate::world::pawn_meshes).
//! AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from `docs/verification/NATIVE_INITIALIZE_BODY_COMPOSITION.md` alone (analyst lane
//! ANALYST-BODY-COMPOSITION, PR #529 at its revised head 87c8b9cd) and the hub's rulings; this lane did not read the
//! executable. **Every rule is UNVERIFIED in the running game.** Off only; Reference keeps the stub policy.
//!
//! **Scope** (the note's section 7): the pawn wrapper only, mode 0, the pawn's own `InstanceDataState` and
//! `ReplicatedInstanceDataState` as the destinations; the owner is a `WillowPawn`, so it has the replayer
//! (`PostInitBodyComposition`). The script native, for the other callers, is `NotYet { NotPorted }` ([`SCRIPT_CALL`]).
//!
//! **Hub rulings** (2026-11-19):
//! 1. a component made from a template counts as valid (the validity check is not modelled);
//! 2. a custom head or skin entry is appended and nothing is applied; the attachment counts as not made;
//! 3. the `Parent` link stays `NotYet { NotRead }` ([`PARENT_LINK`]);
//! 4. the render-side hooks of the pawn's `PostInitBodyComposition` answer nothing where they write no field the
//!    script or a slice step reads: the gear-likeness step and the mode-0 rendering setting (both not read, named
//!    render-side by the note) do nothing here. The light environment is a named field write, so it is written.
//!
//! **The preflight** ([`plan`]) reads and writes nothing, loads included: the call-wide checks (shapes, a rebuild, a
//! client-side apply, a mode other than 0), then the walk (the note's section 3: bases, switches, records), and for
//! each attachment the walk makes, in list order: object and template types, a hit region, both attach attempts, the
//! `Parent` link, and the pawn's `PostInitBodyComposition` (the new `Mesh`'s animation nodes are `LayerAbsent`).
//!
//! **The writes** ([`build`]), in the note's order: per made attachment, in list order: a component is made from its
//! template, its entry appended, its collision set, attached (the parent's attach, else the owner retry), then the
//! pawn's `PostInitBodyComposition` writes; a switch's entry; a custom head or skin's entry. After the walk, on
//! authority, the switch packing (section 5), then `bNetDirty` and `bForceNetUpdate`.
//!
//! **The write tracker** (#518's counters, [`Counters`]; a lazy load is a write): a `NotYet` returned
//! after any counter moved is marked "stopped after partial writes", its kind and record kept; one returned before is
//! plain.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::heap::Origin;
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;


/// The analyst note.
pub const RECORD: &str = "docs/verification/NATIVE_INITIALIZE_BODY_COMPOSITION.md";
/// The script function, and the function named in `VmError::Script` errors.
pub const FUNCTION: &str = "Engine.InstanceDataHelper.InitializeBodyComposition";
pub const UNIT: &str = "InstanceDataHelper.InitializeBodyComposition";

/// The identifiers and names the note compares against (section 4).
pub const PARENT_ID: &str = "Parent";
pub const MESH_ID: &str = "Mesh";
pub const CYLINDER_ID: &str = "CylinderComponent";
pub const CUSTOM_HEAD_NAME: &str = "CustomHead";

/// The engine classes the rules test.
pub const SKELETAL: &str = "Engine.SkeletalMeshComponent";
pub const CYLINDER: &str = "Engine.CylinderComponent";
pub const PRIMITIVE: &str = "Engine.PrimitiveComponent";
pub const MESH_COMPONENT: &str = "Engine.MeshComponent";

/// The note's refusal texts (section 7).
pub const REBUILD: &str =
    "InitializeBodyComposition: rebuilding an existing body composition (the old entries' teardown and replay)";
pub const CLIENT_APPLY: &str = "InitializeBodyComposition: the client-side apply of a replicated state";
pub const OBJECT_TEMPLATES: &str = "InitializeBodyComposition: object and template attachments";
pub const HIT_REGION: &str = "InitializeBodyComposition: registering a hit region";
pub const ACTOR_SOCKETS: &str = "InitializeBodyComposition: an actor's own named socket components";
pub const PARENT_LINK: &str = "InitializeBodyComposition: the Parent link's parent-bone map";
pub const SCRIPT_CALL: &str = "InitializeBodyComposition called from script (its destinations)";
pub const ANIM_NODES: &str = "InitializeBodyComposition: the new Mesh's animation nodes (ArmBoneBlend, AimOffset, \
                              LeftHandGrip, the head look-at, WillowFootIK and the slot nodes)";
/// The layers.
pub const MESH_LAYER: &str = "a skeletal mesh data reader (sockets and the reference skeleton)";
pub const ANIM_LAYER: &str = "an animation-tree instance for the skeletal mesh";

/// This lane's additions, for cases the note's scope does not reach.
pub const MENU_MODES: &str = "InitializeBodyComposition: the menu modes' visibility toggles";
pub const MESH_NOT_SKELETAL: &str =
    "InitializeBodyComposition: a Mesh attachment whose component is not a skeletal mesh component";

/// `"InitializeBodyComposition: the socket and bone tables of <mesh>"`.
pub fn socket_tables(mesh: &str) -> String {
    format!("InitializeBodyComposition: the socket and bone tables of {mesh}")
}

/// A parent component that is not a skeletal component (the note reads only the actor and skeletal targets).
pub fn other_target(class: &str) -> String {
    format!("InitializeBodyComposition: attaching to a {class}")
}

/// A skeletal parent whose template is not loaded: its mesh asset cannot be read without a load.
pub fn lazy_template(path: &str) -> String {
    format!("InitializeBodyComposition: the mesh of the unloaded component template {path}")
}

/// An instance-data type outside the note's list.
pub fn unknown_type(n: u8) -> String {
    format!("InitializeBodyComposition: instance data type {n}")
}

/// The write tracker's three counters (#518's design; the same as #528's `crate::pools::Counters`, kept here because
/// this lane does not stack on #528).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counters {
    writes: u64,
    draws: u64,
    loads: u64,
}

impl Counters {
    pub fn read(vm: &Vm) -> Self {
        Counters { writes: vm.space.heap.committed_writes(), draws: vm.rng.draws(), loads: vm.defs.load_transitions() }
    }
}

/// A `NotYet` returned after a counter moved since `before` is marked "stopped after partial writes", its kind and
/// record kept; any other error, and a `NotYet` with no write since, is returned as it is.
pub fn after_writes(vm: &Vm, before: Counters, e: VmError) -> VmError {
    if Counters::read(vm) != before { crate::pools::after_partial_writes(e) } else { e }
}

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message }
}

fn not_read(what: impl Into<String>) -> VmError {
    NotYet::not_read(what.into(), RECORD).into()
}

/// What an attachment is, by its `Data.Type` enum name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Component,
    Switch,
    Custom,
    Plain,
    ObjectOrTemplate,
    Unknown(u8),
}

fn kind_of(vm: &Vm, type_enum: Option<ow_object::ids::EnumId>, n: u8) -> Kind {
    let name = type_enum
        .and_then(|e| vm.space.classes.enum_layout(e))
        .and_then(|l| l.values().get(usize::from(n)).copied())
        .map(|v| vm.space.names.text(v).to_string());
    match name.as_deref() {
        Some("IDT_ActorComponent") => Kind::Component,
        Some("IDT_Switch") => Kind::Switch,
        Some("IDT_CustomHead" | "IDT_CustomSkin") => Kind::Custom,
        Some("IDT_Bool" | "IDT_Int" | "IDT_Float" | "IDT_Vector" | "IDT_String" | "IDT_Actor") => Kind::Plain,
        Some("IDT_Object" | "IDT_ActorTemplate" | "IDT_PickupTemplate") => Kind::ObjectOrTemplate,
        _ => Kind::Unknown(n),
    }
}

/// Reads struct field `name` of `v`.
pub(crate) fn field(vm: &Vm, v: &Value, name: &str, what: &str) -> Result<Value, VmError> {
    let s = v.as_struct().ok_or_else(|| script(format!("{what} is not a struct")))?;
    let id = vm.space.names.find(name).ok_or_else(|| script(format!("{what} has no {name}")))?;
    let (index, _) = vm
        .space
        .classes
        .struct_layout(s.ty)
        .and_then(|l| l.field(id))
        .ok_or_else(|| script(format!("{what} has no {name}")))?;
    usize::try_from(index).ok().and_then(|i| s.fields.get(i).cloned()).ok_or_else(|| script(format!("{what} has no {name}")))
}

/// Writes struct field `name` of `v`.
pub(crate) fn set_field(vm: &Vm, v: &mut Value, name: &str, x: Value, what: &str) -> Result<(), VmError> {
    let ty = v.as_struct().ok_or_else(|| script(format!("{what} is not a struct")))?.ty;
    let id = vm.space.names.find(name).ok_or_else(|| script(format!("{what} has no {name}")))?;
    let (index, _) = vm
        .space
        .classes
        .struct_layout(ty)
        .and_then(|l| l.field(id))
        .ok_or_else(|| script(format!("{what} has no {name}")))?;
    let s = v.struct_mut().ok_or_else(|| script(format!("{what} is not a struct")))?;
    let slot = usize::try_from(index).ok().and_then(|i| s.fields.get_mut(i)).ok_or_else(|| script(format!("{what} has no {name}")))?;
    *slot = x;
    Ok(())
}

fn object_of(v: &Value) -> Option<ObjRef> {
    match v {
        Value::Object(o) => *o,
        _ => None,
    }
}

fn int(v: &Value) -> i64 {
    match v {
        Value::Int(i) => i64::from(*i),
        Value::Byte(b) => i64::from(*b),
        Value::Bool(b) => i64::from(*b),
        _ => 0,
    }
}

fn byte(v: &Value) -> u8 {
    u8::try_from(int(v)).unwrap_or(0)
}

fn float(v: &Value) -> f32 {
    match v {
        Value::Float(f) => *f,
        _ => 0.0,
    }
}

fn name_is(vm: &Vm, v: &Value, want: &str) -> bool {
    matches!(v, Value::Name(n) if vm.space.names.text(*n).eq_ignore_ascii_case(want))
}

fn is_none_name(vm: &Vm, v: &Value) -> bool {
    match v {
        Value::Name(n) => {
            let t = vm.space.names.text(*n);
            t.is_empty() || t.eq_ignore_ascii_case("None")
        }
        _ => true,
    }
}

fn class_path(vm: &Vm, class: ClassId) -> String {
    vm.space.classes.get_layout(class).map_or_else(|| format!("{class:?}"), |l| l.path().as_str().to_string())
}

fn is_a(vm: &Vm, class: ClassId, path: &str) -> bool {
    vm.space.classes.by_path(path).is_some_and(|p| vm.space.classes.is_child_of(class, p))
}

/// Property `name` of `o` as it stands, without loading anything.
fn read_prop(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<Value>, VmError> {
    let class = vm.space.heap.get(o)?.class;
    match vm.space.prop(class, name) {
        Some(p) => Ok(Some(vm.space.read(o, PropAddr::scalar(p))?.clone())),
        None => Ok(None),
    }
}

/// `o`'s class must declare `name`.
fn require_prop(vm: &Vm, class: ClassId, name: &str) -> Result<(), VmError> {
    vm.space.prop(class, name).map(|_| ()).ok_or_else(|| script(format!("{} has no {name}", class_path(vm, class))))
}

/// The collision table (the note's section 4.1, step 5) for `ComponentData.CollisionType`: the component flags set,
/// the component flags cleared, and the owner's flags set. The component flags are `PrimitiveComponent`'s, named by
/// bit position against the layout tool (open question 2): UNVERIFIED.
fn collision_flags(t: u8) -> (&'static [&'static str], &'static [&'static str], &'static [&'static str]) {
    const CA: &str = "CollideActors";
    const BA: &str = "BlockActors";
    const BRB: &str = "BlockRigidBody";
    const BNZE: &str = "BlockNonZeroExtent";
    const BZE: &str = "BlockZeroExtent";
    const OCA: &str = "bCollideActors";
    const OBA: &str = "bBlockActors";
    const OPE: &str = "bPushedByEncroachers";
    match t {
        0 => (&[], &[], &[]),
        1 => (&[], &[CA, BA, BRB], &[]),
        2 => (&[CA, BA, BNZE, BRB, BZE], &[], &[OCA, OBA, OPE]),
        3 => (&[CA, BA, BZE], &[BNZE, BRB], &[OCA, OBA]),
        4 => (&[CA, BNZE, BRB, BZE], &[BA], &[OCA, OPE]),
        5 => (&[CA, BZE], &[BA, BNZE, BRB], &[OCA]),
        6 => (&[CA, BA, BNZE, BRB], &[BZE], &[OCA, OBA, OPE]),
        7 => (&[CA, BNZE, BRB], &[BA, BZE], &[OCA, OPE]),
        9 => (&[CA, BA, BNZE, BZE], &[BRB], &[OCA, OBA]),
        _ => (&[], &[CA], &[]),
    }
}

/// One attachment, as the plan reads it.
#[derive(Debug, Clone)]
struct Attachment {
    data: Value,
    kind: Kind,
    base: Option<usize>,
    parent_id: Value,
    switch_id: i64,
}

/// A made attachment's parent: the owner, or attachment `i`'s new component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Parent {
    Owner,
    Component(usize),
}

/// The pawn's `PostInitBodyComposition` writes for one made component (section 4.5), decided in the preflight.
#[derive(Debug, Clone, Default)]
struct PostInit {
    /// Identifier `CylinderComponent`: `Pawn.CylinderComponent` = the component as a `CylinderComponent`, and
    /// `CollisionComponent` too when the body class's `bUseMeshForCollision` is clear.
    cylinder: Option<bool>,
    /// `Data.Name` `CustomHead`: `HeadMesh` = the component as a `SkeletalMeshComponent`.
    head: bool,
    /// The pawn's `LightEnvironment`, given to a mesh component.
    light: Option<ObjRef>,
}

/// What the walk makes for one attachment (its record).
#[derive(Debug, Clone)]
enum Made {
    No,
    Component { template: ObjRef, class: ClassId, collision: u8, post: PostInit },
    /// A switch's record holds its own parent.
    Switch { parent: Parent },
    /// A custom head or skin: an entry, but not made.
    Custom,
}

/// The preflight's result.
#[derive(Debug)]
pub struct Plan {
    attachments: Vec<Attachment>,
    made: Vec<Made>,
    authority: bool,
    entry_struct: StructId,
}

/// The call's inputs.
pub struct Inputs {
    pub owner: Option<ObjRef>,
    pub composition: Value,
    pub set: Value,
    pub replicated: Value,
    pub mode: u8,
}

/// The two destinations (the pawn's own properties).
pub const SET: &str = "InstanceDataState";
pub const REPLICATED: &str = "ReplicatedInstanceDataState";

/// The preflight (module documentation): reads only, writes and loads nothing.
pub fn plan(vm: &Vm, i: &Inputs) -> Result<Option<Plan>, VmError> {
    let Some(owner) = i.owner else {
        return Ok(None);
    };
    // 1. Shapes.
    let list = field(vm, &i.composition, "Attachments", "BodyComposition")?;
    let items = list.elements().ok_or_else(|| script("BodyComposition.Attachments is not an array".into()))?.to_vec();
    let old = field(vm, &i.set, "Data", "InstanceDataSet")?;
    let old = old.elements().ok_or_else(|| script("InstanceDataSet.Data is not an array".into()))?.len();
    let Some(entry_struct) = vm.space.classes.struct_by_path("Engine.IInstanceData.InstanceDataUnion") else {
        return Err(script("no Engine.IInstanceData.InstanceDataUnion struct".into()));
    };
    let removed_bits = int(&field(vm, &i.replicated, "RemovedBodyCompositionPartsBitField", "ReplicatedInstanceDataState")?);
    let need_apply = int(&field(vm, &i.replicated, "bNeedToApplyThisState", "ReplicatedInstanceDataState")?) != 0;
    // 2. A rebuild.
    if old != 0 {
        return Err(not_read(REBUILD));
    }
    if items.is_empty() {
        return Ok(None);
    }
    // 3. Authority.
    let authority = crate::actors::byte(&vm.space, owner, "Role")? == 3;
    if !authority && need_apply {
        return Err(not_read(CLIENT_APPLY));
    }
    if i.mode != 0 {
        return Err(not_read(MENU_MODES));
    }
    let owner_class = vm.space.heap.get(owner)?.class;
    for name in [SET, REPLICATED, "Components"] {
        require_prop(vm, owner_class, name)?;
    }
    if authority {
        require_prop(vm, owner_class, "bNetDirty")?;
        require_prop(vm, owner_class, "bForceNetUpdate")?;
    }
    let body_class = read_prop(vm, owner, "BodyClass")?.and_then(|v| object_of(&v));
    let mesh_for_collision = match body_class {
        Some(b) if !vm.defs.is_unloaded(&vm.space, b) => read_prop(vm, b, "bUseMeshForCollision")?.is_some_and(|v| int(&v) != 0),
        _ => false,
    };
    let light = read_prop(vm, owner, "LightEnvironment")?.and_then(|v| object_of(&v));
    let type_enum = vm
        .space
        .classes
        .struct_layout(entry_struct)
        .and_then(|l| vm.space.names.find("Type").and_then(|n| l.field(n)))
        .and_then(|(_, d)| match &d.kind {
            PropKind::Byte { enum_ } => *enum_,
            _ => None,
        });
    let n = items.len();
    let mut attachments = Vec::with_capacity(n);
    for item in &items {
        let data = field(vm, item, "Data", "BodyInstanceDataUnion")?;
        let kind = kind_of(vm, type_enum, byte(&field(vm, &data, "Type", "InstanceDataUnion")?));
        let base = int(&field(vm, item, "AttachmentBaseInstanceDataIndex", "BodyInstanceDataUnion")?);
        attachments.push(Attachment {
            data,
            kind,
            base: usize::try_from(base).ok().filter(|&b| b < n),
            parent_id: field(vm, item, "ParentAttachmentIdentifier", "BodyInstanceDataUnion")?,
            switch_id: int(&field(vm, item, "SwitchIdentifier", "BodyInstanceDataUnion")?),
        });
    }
    // 4. The walk (section 3), each attachment once, in list order; a base's record is read as it stands.
    let mut made: Vec<Made> = Vec::with_capacity(n);
    for (idx, a) in attachments.iter().enumerate() {
        // A first build: no record is made yet, so a removed attachment is skipped.
        if idx < 32 && (removed_bits >> idx) & 1 == 1 {
            made.push(Made::No);
            continue;
        }
        let parent = match a.base {
            None => Parent::Owner,
            Some(b) => match made.get(b) {
                None | Some(Made::No | Made::Custom) => {
                    made.push(Made::No);
                    continue;
                }
                Some(Made::Switch { parent }) => {
                    // The switch's entry: Data copied, Float 0 (section 4.2), so |Float| < 0.00000001 holds.
                    let entry = &attachments[b].data;
                    if int(&field(vm, entry, "Int", "InstanceDataUnion")?) != a.switch_id {
                        made.push(Made::No);
                        continue;
                    }
                    *parent
                }
                Some(Made::Component { .. }) => Parent::Component(b),
            },
        };
        match a.kind {
            Kind::Plain => made.push(Made::No),
            Kind::ObjectOrTemplate => return Err(not_read(OBJECT_TEMPLATES)),
            Kind::Unknown(t) => return Err(not_read(unknown_type(t))),
            Kind::Switch => made.push(Made::Switch { parent }),
            Kind::Custom => made.push(Made::Custom),
            Kind::Component => {
                let cd = field(vm, &a.data, "ComponentData", "InstanceDataUnion")?;
                let Some(template) = object_of(&field(vm, &cd, "Component", "ComponentData")?) else {
                    made.push(Made::No);
                    continue;
                };
                let class = vm.space.heap.get(template)?.class;
                let collision = byte(&field(vm, &cd, "CollisionType", "ComponentData")?);
                if is_a(vm, class, PRIMITIVE) {
                    let (set, clear, owner_set) = collision_flags(collision);
                    for f in set.iter().chain(clear) {
                        require_prop(vm, class, f)?;
                    }
                    for f in owner_set {
                        require_prop(vm, owner_class, f)?;
                    }
                }
                if object_of(&field(vm, &cd, "HitRegionDefinition", "ComponentData")?).is_some() {
                    return Err(not_read(HIT_REGION));
                }
                // The attach: the parent's first, then the owner retry with no socket name (an actor attach, which
                // answers true). Only the actor attach is ever written.
                require_prop(vm, class, "Owner")?;
                let socket = field(vm, &cd, "MeshSocketName", "ComponentData")?;
                let has_socket = !is_none_name(vm, &socket);
                let skeletal_parent = match parent {
                    Parent::Owner => {
                        if has_socket {
                            return Err(not_read(ACTOR_SOCKETS));
                        }
                        false
                    }
                    Parent::Component(b) => {
                        let Made::Component { template: pt, class: pc, .. } = &made[b] else {
                            unreachable!("a component parent's record is a component")
                        };
                        if !is_a(vm, *pc, SKELETAL) {
                            return Err(not_read(other_target(&class_path(vm, *pc))));
                        }
                        if has_socket {
                            if vm.defs.is_unloaded(&vm.space, *pt) {
                                return Err(not_read(lazy_template(&crate::world::pawn_meshes::handle_path(vm, *pt))));
                            }
                            // No mesh asset: the skeletal attach answers false and the owner retry takes it.
                            if let Some(mesh) = read_prop(vm, *pt, "SkeletalMesh")?.and_then(|v| object_of(&v)) {
                                let path = crate::world::pawn_meshes::handle_path(vm, mesh);
                                return Err(NotYet::layer_absent(socket_tables(&path), MESH_LAYER, RECORD).into());
                            }
                        }
                        true
                    }
                };
                let skeletal = is_a(vm, class, SKELETAL);
                if name_is(vm, &a.parent_id, PARENT_ID) && skeletal_parent && skeletal {
                    return Err(not_read(PARENT_LINK));
                }
                // The pawn's PostInitBodyComposition (section 4.5), mode 0.
                let mut post = PostInit::default();
                if name_is(vm, &a.parent_id, MESH_ID) {
                    if skeletal {
                        return Err(NotYet::layer_absent(ANIM_NODES, ANIM_LAYER, RECORD).into());
                    }
                    return Err(not_read(MESH_NOT_SKELETAL));
                }
                if name_is(vm, &a.parent_id, CYLINDER_ID) {
                    require_prop(vm, owner_class, "CylinderComponent")?;
                    if !mesh_for_collision {
                        require_prop(vm, owner_class, "CollisionComponent")?;
                    }
                    post.cylinder = Some(!mesh_for_collision);
                }
                if name_is(vm, &field(vm, &a.data, "Name", "InstanceDataUnion")?, CUSTOM_HEAD_NAME) {
                    require_prop(vm, owner_class, "HeadMesh")?;
                    post.head = true;
                }
                if let Some(l) = light
                    && is_a(vm, class, MESH_COMPONENT)
                {
                    require_prop(vm, class, "LightEnvironment")?;
                    post.light = Some(l);
                }
                made.push(Made::Component { template, class, collision, post });
            }
        }
    }
    Ok(Some(Plan { attachments, made, authority, entry_struct }))
}

/// The build (module documentation): the preflight, then the writes in the note's order, under the write tracker.
pub fn build(vm: &mut Vm, i: Inputs) -> Result<(), VmError> {
    let before = Counters::read(vm);
    let Some(plan) = plan(vm, &i)? else {
        return Ok(());
    };
    let owner = i.owner.expect("a plan has an owner");
    writes(vm, owner, i.set, i.replicated, &plan).map_err(|e| after_writes(vm, before, e))
}

/// A zero entry with `copied` fields taken from `data` (the note's entry contract).
fn entry_from(vm: &Vm, plan: &Plan, data: &Value, copied: &[&str]) -> Result<Value, VmError> {
    let mut e = vm.space.classes.zero_value(&PropKind::Struct(plan.entry_struct));
    for name in copied {
        let v = field(vm, data, name, "InstanceDataUnion")?;
        set_field(vm, &mut e, name, v, "InstanceDataUnion")?;
    }
    Ok(e)
}

fn writes(vm: &mut Vm, owner: ObjRef, set: Value, replicated: Value, plan: &Plan) -> Result<(), VmError> {
    let mut set = set;
    for (a, m) in plan.attachments.iter().zip(&plan.made) {
        match m {
            Made::No => {}
            Made::Switch { .. } => {
                let mut e = a.data.clone();
                set_field(vm, &mut e, "Float", Value::Float(0.0), "InstanceDataUnion")?;
                append(vm, owner, &mut set, e)?;
            }
            Made::Custom => {
                let e = entry_from(vm, plan, &a.data, &["Name", "Type", "BodyCompositionIndex"])?;
                append(vm, owner, &mut set, e)?;
            }
            Made::Component { template, class, collision, post } => {
                let made = make_component(vm, owner, *template)?;
                let mut e = entry_from(vm, plan, &a.data, &["Name", "Type", "DeleteByOwner", "ComponentData", "BodyCompositionIndex"])?;
                let mut cd = field(vm, &e, "ComponentData", "InstanceDataUnion")?;
                set_field(vm, &mut cd, "Component", Value::Object(Some(made)), "ComponentData")?;
                set_field(vm, &mut e, "ComponentData", cd, "InstanceDataUnion")?;
                append(vm, owner, &mut set, e)?;
                if is_a(vm, *class, PRIMITIVE) {
                    let (on, off, owner_on) = collision_flags(*collision);
                    for f in on {
                        write_prop(vm, made, f, Value::Bool(true))?;
                    }
                    for f in off {
                        write_prop(vm, made, f, Value::Bool(false))?;
                    }
                    for f in owner_on {
                        write_prop(vm, owner, f, Value::Bool(true))?;
                    }
                }
                attach_to_actor(vm, owner, made)?;
                if let Some(collision_too) = post.cylinder {
                    let as_cylinder = is_a(vm, *class, CYLINDER).then_some(made);
                    write_prop(vm, owner, "CylinderComponent", Value::Object(as_cylinder))?;
                    if collision_too {
                        write_prop(vm, owner, "CollisionComponent", Value::Object(as_cylinder))?;
                    }
                }
                if post.head {
                    let as_skeletal = is_a(vm, *class, SKELETAL).then_some(made);
                    write_prop(vm, owner, "HeadMesh", Value::Object(as_skeletal))?;
                }
                if let Some(l) = post.light {
                    write_prop(vm, made, "LightEnvironment", Value::Object(Some(l)))?;
                }
            }
        }
    }
    if plan.authority {
        let (bits, values) = pack_switches(vm, plan, &set)?;
        let mut rep = replicated;
        set_field(vm, &mut rep, "SwitchStateBitField", Value::Int(bits as i32), "ReplicatedInstanceDataState")?;
        set_field(vm, &mut rep, "ActiveSwitchValues", Value::Int(values as i32), "ReplicatedInstanceDataState")?;
        write_prop(vm, owner, REPLICATED, rep)?;
        write_prop(vm, owner, "bNetDirty", Value::Bool(true))?;
        write_prop(vm, owner, "bForceNetUpdate", Value::Bool(true))?;
    }
    Ok(())
}

/// The switch packing (section 5) over the entries built: `(SwitchStateBitField, ActiveSwitchValues)`.
fn pack_switches(vm: &Vm, plan: &Plan, set: &Value) -> Result<(u32, u32), VmError> {
    let data = field(vm, set, "Data", "InstanceDataSet")?;
    let entries = data.elements().ok_or_else(|| script("InstanceDataSet.Data is not an array".into()))?;
    let mut bits: u32 = 0;
    let mut values: u32 = 0;
    let mut offset: i64 = 0;
    let mut k: u32 = 0;
    for (idx, a) in plan.attachments.iter().enumerate() {
        if a.kind != Kind::Switch {
            continue;
        }
        if k >= 32 {
            break;
        }
        let position = k;
        k += 1;
        let mut entry = None;
        for e in entries {
            if plan.kind_of_entry(vm, e)? == Kind::Switch
                && int(&field(vm, e, "BodyCompositionIndex", "InstanceDataUnion")?) - 1 == idx as i64
            {
                entry = Some(e);
                break;
            }
        }
        let Some(e) = entry else {
            continue;
        };
        bits |= 1 << position;
        let declared = i64::from(byte(&field(vm, e, "BitsRequiredForMaxSwitchValue", "InstanceDataUnion")?));
        let width = if declared != 0 { declared } else { float(&field(vm, e, "Float", "InstanceDataUnion")?) as i64 };
        if offset + width <= 32 {
            let value = int(&field(vm, e, "Int", "InstanceDataUnion")?) as u32;
            values |= value.wrapping_shl(offset.rem_euclid(32) as u32);
            offset += width;
        }
    }
    Ok((bits, values))
}

impl Plan {
    fn kind_of_entry(&self, vm: &Vm, e: &Value) -> Result<Kind, VmError> {
        let type_enum = vm
            .space
            .classes
            .struct_layout(self.entry_struct)
            .and_then(|l| vm.space.names.find("Type").and_then(|n| l.field(n)))
            .and_then(|(_, d)| match &d.kind {
                PropKind::Byte { enum_ } => *enum_,
                _ => None,
            });
        Ok(kind_of(vm, type_enum, byte(&field(vm, e, "Type", "InstanceDataUnion")?)))
    }
}

fn write_prop(vm: &mut Vm, o: ObjRef, name: &str, v: Value) -> Result<(), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{} has no {name}", class_path(vm, class))))?;
    vm.space.write(o, PropAddr::scalar(p), v)?;
    Ok(())
}

fn append(vm: &mut Vm, owner: ObjRef, set: &mut Value, entry: Value) -> Result<(), VmError> {
    let mut data = field(vm, set, "Data", "InstanceDataSet")?;
    let items = data.array_mut().ok_or_else(|| script("InstanceDataSet.Data is not an array".into()))?;
    items.push(entry);
    set_field(vm, set, "Data", data, "InstanceDataSet")?;
    write_prop(vm, owner, SET, set.clone())
}

/// A new component from `template`, its outer the owner, its properties the template's (the template is loaded
/// first when it is a store handle not yet loaded: a write the tracker counts). Ruling 1: it is valid.
fn make_component(vm: &mut Vm, owner: ObjRef, template: ObjRef) -> Result<ObjRef, VmError> {
    if vm.defs.is_unloaded(&vm.space, template) {
        let r = vm.defs.ensure_loaded(&mut vm.space, template);
        vm.log.extend(vm.defs.take_log());
        r.map_err(VmError::from)?;
    }
    let (class, name, slots) = {
        let t = vm.space.heap.get(template)?;
        (t.class, t.name, t.slots.clone())
    };
    let name = vm.space.names.text(name).to_string();
    let made = vm.space.alloc(class, &name, Some(owner), Origin::Runtime)?;
    vm.space.heap.get_mut(made)?.slots = slots;
    vm.space.heap.note_write();
    Ok(made)
}

/// The actor attach with no socket name (section 4.1, step 6): the offsets are zero, so `Translation` and `Rotation`
/// stay as they are; the component's `Owner` is the actor (by fit), and it joins the actor's `Components` unless it is
/// there already (by fit). The global "attaching later" state is never set in the port (section 7, item 5).
fn attach_to_actor(vm: &mut Vm, owner: ObjRef, component: ObjRef) -> Result<(), VmError> {
    write_prop(vm, component, "Owner", Value::Object(Some(owner)))?;
    let class = vm.space.heap.get(owner)?.class;
    let p = vm.space.prop(class, "Components").ok_or_else(|| script(format!("{} has no Components", class_path(vm, class))))?;
    let mut list = vm.space.read(owner, PropAddr::scalar(p))?.clone();
    let Some(items) = list.array_mut() else {
        return Err(script(format!("{}.Components is not an array", class_path(vm, class))));
    };
    if items.iter().any(|v| object_of(v) == Some(component)) {
        return Ok(());
    }
    items.push(Value::Object(Some(component)));
    vm.space.write(owner, PropAddr::scalar(p), list)?;
    Ok(())
}

/// The script native, for callers outside the pawn wrapper (nested or local destinations): outside the note's
/// contract, `NotYet { NotPorted }`.
fn native(_c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    Err(NotYet::not_ported(SCRIPT_CALL, RECORD).into())
}

/// Binds the script native (Off only; the caller checks the mode). A key bound first keeps its binding.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    let Ok(id) = vm.find_function(FUNCTION) else {
        return Ok(());
    };
    let Some(key) = vm.function(id).and_then(|d| d.native_key.clone()) else {
        return Ok(());
    };
    if !vm.has_native(&key) {
        vm.register_native(&key, Rc::new(native))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "body_composition_tests.rs"]
mod tests;
