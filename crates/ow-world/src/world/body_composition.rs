//! Owner: lane IMPL-BODY-COMPOSITION (decision 2026-11-19-NNN, number pending). `InstanceDataHelper.InitializeBodyComposition`
//! in `ParityMode::Off`, and the pawn wrapper's use of it (`WillowPawn.InitializePawnMeshes`, crate::world::pawn_meshes).
//! AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from `docs/verification/NATIVE_INITIALIZE_BODY_COMPOSITION.md` alone (analyst lane
//! ANALYST-BODY-COMPOSITION, PR #529 at cdf199be) and the hub's three rulings on its section 7 choices; this lane did
//! not read the executable. **Every rule is UNVERIFIED in the running game.** Off only; Reference keeps the stub policy.
//!
//! **Hub rulings** (2026-11-19, the note's "(hub)" items):
//! 1. a component made from a template counts as valid (the validity check is not modelled);
//! 2. the special parent identifier's refresh stays `NotYet { NotRead }` ([`SPECIAL_REFRESH`]);
//! 3. a custom head or skin entry is appended and nothing is applied (render-only).
//!
//! **The preflight** ([`plan`]) reads and writes nothing: it decides which attachments the walk makes (the note's
//! section 3 rules, base and switch included) and raises every refusal the note names, in walk order, before the first
//! write. Call-wide checks come first: a mode other than 0, and a client-side apply of a replicated state. Then each
//! attachment that would be made, in list order: object and template types, a removed attachment that an earlier build
//! made, a collision type other than 0, the attach target (an actor's own named socket components, a skeletal parent's
//! socket and bone tables, a target the note does not cover), and the special parent identifier.
//!
//! **Collision types.** The note names the flags types 1, 2, 3, 6 and 9 set "by fit only", so writing them would be a
//! guess: every type other than 0 is refused with the note's text "InitializeBodyComposition: collision type <n>"
//! (`NotRead`), a narrower answer than the note's proposal (decision file).
//!
//! **The writes** ([`build`]), in the note's order: per made attachment, in list order, its entry is appended (a
//! component is made from its template first, its entry appended, then attached); after the walk, on authority, the
//! replicated switch fields (zeroed, then packed), then `bNetDirty` and `bForceNetUpdate`.
//!
//! **The write tracker** (#518's counters: `Heap::committed_writes`, `GameRng::draws`, `DefStore::load_transitions`;
//! a lazy load is a write): a `NotYet` returned after any counter moved is marked "stopped after partial writes", its
//! kind and record kept ([`after_writes`]); one returned before is plain.

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
/// The special parent identifier, named by fit (the note's open question 4). UNVERIFIED.
pub const SPECIAL_PARENT: &str = "Mesh";
/// The skeletal component class (the note's section 4.1, step 6).
pub const SKELETAL: &str = "Engine.SkeletalMeshComponent";

/// The note's refusal texts (section 7).
pub const OBJECT_TEMPLATES: &str = "InitializeBodyComposition: object and template attachments";
pub const TEARDOWN: &str = "InitializeBodyComposition: tearing down a removed attachment";
pub const MENU_MODES: &str = "InitializeBodyComposition: the menu modes' visibility toggles";
pub const CLIENT_APPLY: &str = "InitializeBodyComposition: the client-side apply of a replicated state";
pub const ACTOR_SOCKETS: &str = "InitializeBodyComposition: an actor's own named socket components";
pub const SPECIAL_REFRESH: &str = "InitializeBodyComposition: the special parent identifier's refresh";
/// The layer the socket and bone tables need.
pub const MESH_LAYER: &str = "a skeletal mesh data reader (sockets and the reference skeleton)";

/// `"InitializeBodyComposition: collision type <n>"`.
pub fn collision_type(n: u8) -> String {
    format!("InitializeBodyComposition: collision type {n}")
}

/// `"InitializeBodyComposition: the socket and bone tables of <mesh>"`.
pub fn socket_tables(mesh: &str) -> String {
    format!("InitializeBodyComposition: the socket and bone tables of {mesh}")
}

/// A target the note does not cover (a component parent that is not a skeletal component). This lane's addition.
pub fn other_target(class: &str) -> String {
    format!("InitializeBodyComposition: attaching to a {class}")
}

/// A component under a switch base that would be made: its parent is not read. This lane's addition.
pub const SWITCHED_PARENT: &str = "InitializeBodyComposition: the parent of a component made under a switch";

/// An instance-data type outside the note's list. This lane's addition.
pub fn unknown_type(n: u8) -> String {
    format!("InitializeBodyComposition: instance data type {n}")
}

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message }
}

fn not_read(what: impl Into<String>) -> VmError {
    NotYet::not_read(what.into(), RECORD).into()
}

/// The write tracker's three counters (#518's design).
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
fn field(vm: &Vm, v: &Value, name: &str, what: &str) -> Result<Value, VmError> {
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
fn set_field(vm: &Vm, v: &mut Value, name: &str, x: Value, what: &str) -> Result<(), VmError> {
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

fn name_text(vm: &Vm, v: &Value) -> String {
    match v {
        Value::Name(n) => vm.space.names.text(*n).to_string(),
        _ => String::new(),
    }
}

fn is_none_name(vm: &Vm, v: &Value) -> bool {
    let t = name_text(vm, v);
    t.is_empty() || t.eq_ignore_ascii_case("None")
}

fn class_path(vm: &Vm, class: ClassId) -> String {
    vm.space.classes.get_layout(class).map_or_else(|| format!("{class:?}"), |l| l.path().as_str().to_string())
}

/// One attachment, as the plan reads it.
#[derive(Debug, Clone)]
struct Attachment {
    data: Value,
    kind: Kind,
    base: Option<usize>,
    parent_id: Value,
    switch_id: u8,
}

/// What the walk makes for one attachment.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Made {
    No,
    /// A component from `template`, attached to the owner (an actor attach with no socket).
    Component { template: ObjRef },
    Switch,
    Custom,
}

/// The preflight's result: the attachments and what the walk makes of each.
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

/// The preflight (module documentation): reads only, writes and loads nothing.
pub fn plan(vm: &Vm, i: &Inputs) -> Result<Option<Plan>, VmError> {
    let Some(owner) = i.owner else {
        return Ok(None);
    };
    let list = field(vm, &i.composition, "Attachments", "BodyComposition")?;
    let items = list.elements().ok_or_else(|| script("BodyComposition.Attachments is not an array".into()))?.to_vec();
    if items.is_empty() {
        return Ok(None);
    }
    let old = field(vm, &i.set, "Data", "InstanceDataSet")?;
    let old = old.elements().ok_or_else(|| script("InstanceDataSet.Data is not an array".into()))?.to_vec();
    let entry_struct = match vm.space.classes.struct_by_path("Engine.IInstanceData.InstanceDataUnion") {
        Some(s) => s,
        None => return Err(script("no Engine.IInstanceData.InstanceDataUnion struct".into())),
    };
    if i.mode != 0 {
        return Err(not_read(MENU_MODES));
    }
    let role = crate::actors::byte(&vm.space, owner, "Role")?;
    let authority = role == 3;
    let removed_bits = int(&field(vm, &i.replicated, "RemovedBodyCompositionPartsBitField", "ReplicatedInstanceDataState")?);
    let need_apply = int(&field(vm, &i.replicated, "bNeedToApplyThisState", "ReplicatedInstanceDataState")?) != 0;
    if !authority && need_apply {
        return Err(not_read(CLIENT_APPLY));
    }
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
        let base = usize::try_from(base).ok().filter(|&b| b < n);
        attachments.push(Attachment {
            data,
            kind,
            base,
            parent_id: field(vm, item, "ParentAttachmentIdentifier", "BodyInstanceDataUnion")?,
            switch_id: byte(&field(vm, item, "SwitchIdentifier", "BodyInstanceDataUnion")?),
        });
    }
    let skeletal = vm.space.classes.by_path(SKELETAL);
    let mut made: Vec<Made> = Vec::with_capacity(n);
    for (idx, a) in attachments.iter().enumerate() {
        let removed = idx < 32 && (removed_bits >> idx) & 1 == 1;
        if removed {
            let had = old.iter().any(|e| field(vm, e, "BodyCompositionIndex", "InstanceDataUnion").is_ok_and(|v| int(&v) == idx as i64 + 1));
            if had {
                return Err(not_read(TEARDOWN));
            }
            made.push(Made::No);
            continue;
        }
        // The parent: the owner with no base; the base's made object; nothing when the base is not made.
        let mut parent_is_owner = true;
        let mut parent_class: Option<ClassId> = None;
        if let Some(b) = a.base {
            match (&made.get(b).cloned().unwrap_or(Made::No), attachments[b].kind) {
                (Made::No, _) => {
                    made.push(Made::No);
                    continue;
                }
                (Made::Switch, _) => {
                    // The switch entry is a copy of the switch's Data with Float 0: made only when its Int is this
                    // attachment's SwitchIdentifier (the Float test always passes on a fresh entry).
                    let entry = &attachments[b].data;
                    let value = int(&field(vm, entry, "Int", "InstanceDataUnion")?);
                    if value != i64::from(a.switch_id) {
                        made.push(Made::No);
                        continue;
                    }
                    if a.kind == Kind::Component {
                        return Err(not_read(SWITCHED_PARENT));
                    }
                }
                (Made::Component { template }, _) => {
                    parent_is_owner = false;
                    parent_class = Some(vm.space.heap.get(*template)?.class);
                }
                (Made::Custom, _) => {
                    made.push(Made::No);
                    continue;
                }
            }
        }
        match a.kind {
            Kind::Plain => made.push(Made::No),
            Kind::ObjectOrTemplate => return Err(not_read(OBJECT_TEMPLATES)),
            Kind::Unknown(t) => return Err(not_read(unknown_type(t))),
            Kind::Switch => made.push(Made::Switch),
            Kind::Custom => made.push(Made::Custom),
            Kind::Component => {
                let component_data = field(vm, &a.data, "ComponentData", "InstanceDataUnion")?;
                let Some(template) = object_of(&field(vm, &component_data, "Component", "ComponentData")?) else {
                    made.push(Made::No);
                    continue;
                };
                let collision = byte(&field(vm, &component_data, "CollisionType", "ComponentData")?);
                if collision != 0 {
                    return Err(not_read(collision_type(collision)));
                }
                let socket = field(vm, &component_data, "MeshSocketName", "ComponentData")?;
                let has_socket = !is_none_name(vm, &socket);
                if !parent_is_owner {
                    let pc = parent_class.expect("a component parent has a class");
                    if skeletal.is_some_and(|s| vm.space.classes.is_child_of(pc, s)) {
                        if has_socket {
                            let mesh = object_of(&field_of_object(vm, made_template(&made, a.base), "SkeletalMesh")?)
                                .map(|m| crate::pools::object_path(&vm.space, m))
                                .unwrap_or_else(|| "None".to_string());
                            return Err(NotYet::layer_absent(socket_tables(&mesh), MESH_LAYER, RECORD).into());
                        }
                        // No socket: the skeletal parent answers false and the owner takes it (an actor attach).
                    } else {
                        return Err(not_read(other_target(&class_path(vm, pc))));
                    }
                } else if has_socket {
                    return Err(not_read(ACTOR_SOCKETS));
                }
                if name_text(vm, &a.parent_id).eq_ignore_ascii_case(SPECIAL_PARENT) {
                    return Err(not_read(SPECIAL_REFRESH));
                }
                made.push(Made::Component { template });
            }
        }
    }
    Ok(Some(Plan { attachments, made, authority, entry_struct }))
}

/// The template of the made component attachment `base` names (for the socket refusal's mesh name).
fn made_template(made: &[Made], base: Option<usize>) -> Option<ObjRef> {
    match base.and_then(|b| made.get(b)) {
        Some(Made::Component { template }) => Some(*template),
        _ => None,
    }
}

/// Property `name` of `o` (None when `o` is None or the class does not declare it). A read only: an unloaded handle
/// is not loaded (its slots are read as they stand).
fn field_of_object(vm: &Vm, o: Option<ObjRef>, name: &str) -> Result<Value, VmError> {
    let Some(o) = o else { return Ok(Value::Object(None)) };
    let class = vm.space.heap.get(o)?.class;
    match vm.space.prop(class, name) {
        Some(p) if !vm.space.heap.get(o)?.slots.is_empty() => Ok(vm.space.read(o, PropAddr::scalar(p))?.clone()),
        _ => Ok(Value::Object(None)),
    }
}

/// Where the build's two struct results go: the owner's own properties (the pawn wrapper) or the script call's out
/// arguments.
pub enum Sink {
    Properties { owner: ObjRef, set: &'static str, replicated: &'static str },
    Out { set: usize, replicated: usize },
}

impl Sink {
    fn write_set(&self, c: &mut NativeCall<'_>, v: Value) -> Result<(), VmError> {
        match self {
            Sink::Properties { owner, set, .. } => write_prop(c.vm, *owner, set, v),
            Sink::Out { set, .. } => c.set_out(*set, v),
        }
    }

    fn write_replicated(&self, c: &mut NativeCall<'_>, v: Value) -> Result<(), VmError> {
        match self {
            Sink::Properties { owner, replicated, .. } => write_prop(c.vm, *owner, replicated, v),
            Sink::Out { replicated, .. } => c.set_out(*replicated, v),
        }
    }
}

fn write_prop(vm: &mut Vm, o: ObjRef, name: &str, v: Value) -> Result<(), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{} has no {name}", class_path(vm, class))))?;
    vm.space.write(o, PropAddr::scalar(p), v)?;
    Ok(())
}

/// The build (module documentation): the preflight, then the writes in the note's order, under the write tracker.
pub fn build(c: &mut NativeCall<'_>, i: Inputs, sink: &Sink) -> Result<(), VmError> {
    let before = Counters::read(c.vm);
    let Some(plan) = plan(c.vm, &i)? else {
        return Ok(());
    };
    let owner = i.owner.expect("a plan has an owner");
    writes(c, owner, i.set, i.replicated, &plan, sink).map_err(|e| after_writes(c.vm, before, e))
}

fn writes(c: &mut NativeCall<'_>, owner: ObjRef, set: Value, replicated: Value, plan: &Plan, sink: &Sink) -> Result<(), VmError> {
    let mut set = set;
    let mut entries: Vec<(usize, Value)> = Vec::new();
    for (idx, (a, m)) in plan.attachments.iter().zip(&plan.made).enumerate() {
        let index_value = Value::Int(i32::try_from(idx + 1).unwrap_or(i32::MAX));
        let entry = match m {
            Made::No => continue,
            Made::Switch => {
                let mut e = a.data.clone();
                set_field(c.vm, &mut e, "Float", Value::Float(0.0), "InstanceDataUnion")?;
                set_field(c.vm, &mut e, "BodyCompositionIndex", index_value, "InstanceDataUnion")?;
                e
            }
            Made::Custom => {
                let mut e = c.vm.space.classes.struct_defaults(plan.entry_struct).map_err(|e| script(e.to_string()))?;
                for name in ["Name", "Replication", "Type"] {
                    let v = field(c.vm, &a.data, name, "InstanceDataUnion")?;
                    set_field(c.vm, &mut e, name, v, "InstanceDataUnion")?;
                }
                set_field(c.vm, &mut e, "BodyCompositionIndex", index_value, "InstanceDataUnion")?;
                e
            }
            Made::Component { template } => {
                let made = make_component(c.vm, owner, *template)?;
                let mut e = c.vm.space.classes.struct_defaults(plan.entry_struct).map_err(|e| script(e.to_string()))?;
                for name in ["Name", "Type", "DeleteByOwner"] {
                    let v = field(c.vm, &a.data, name, "InstanceDataUnion")?;
                    set_field(c.vm, &mut e, name, v, "InstanceDataUnion")?;
                }
                let mut data = field(c.vm, &a.data, "ComponentData", "InstanceDataUnion")?;
                set_field(c.vm, &mut data, "Component", Value::Object(Some(made)), "ComponentData")?;
                set_field(c.vm, &mut e, "ComponentData", data, "InstanceDataUnion")?;
                set_field(c.vm, &mut e, "BodyCompositionIndex", index_value, "InstanceDataUnion")?;
                // The entry before the attach, as in the game.
                append(c, &mut set, e.clone(), sink)?;
                attach_to_actor(c.vm, owner, made)?;
                entries.push((idx, e));
                continue;
            }
        };
        append(c, &mut set, entry.clone(), sink)?;
        entries.push((idx, entry));
    }
    if plan.authority {
        let mut rep = replicated;
        let mut bits: i64 = 0;
        let mut values: i64 = 0;
        let mut bit = 0u32;
        let mut offset: i64 = 0;
        for (idx, a) in plan.attachments.iter().enumerate() {
            if a.kind != Kind::Switch {
                continue;
            }
            if bit >= 32 {
                break;
            }
            if let Some((_, e)) = entries.iter().find(|(i, _)| *i == idx) {
                bits |= 1i64 << bit;
                let width_bits = i64::from(byte(&field(c.vm, e, "BitsRequiredForMaxSwitchValue", "InstanceDataUnion")?));
                let width = if width_bits != 0 { width_bits } else { float(&field(c.vm, e, "Float", "InstanceDataUnion")?) as i64 };
                if offset + width <= 32 {
                    values |= int(&field(c.vm, e, "Int", "InstanceDataUnion")?) << offset;
                }
                offset += width;
            }
            bit += 1;
        }
        let to_i32 = |x: i64| Value::Int(x as u32 as i32);
        set_field(c.vm, &mut rep, "SwitchStateBitField", to_i32(bits), "ReplicatedInstanceDataState")?;
        set_field(c.vm, &mut rep, "ActiveSwitchValues", to_i32(values), "ReplicatedInstanceDataState")?;
        sink.write_replicated(c, rep)?;
        write_prop(c.vm, owner, "bNetDirty", Value::Bool(true))?;
        write_prop(c.vm, owner, "bForceNetUpdate", Value::Bool(true))?;
    }
    Ok(())
}

fn append(c: &mut NativeCall<'_>, set: &mut Value, entry: Value, sink: &Sink) -> Result<(), VmError> {
    let mut data = field(c.vm, set, "Data", "InstanceDataSet")?;
    let items = data.array_mut().ok_or_else(|| script("InstanceDataSet.Data is not an array".into()))?;
    items.push(entry);
    set_field(c.vm, set, "Data", data, "InstanceDataSet")?;
    sink.write_set(c, set.clone())
}

/// A new component from `template`, its outer the owner, its properties the template's (the template is loaded
/// first when it is a store handle not yet loaded: a write the tracker counts). Ruling 1: it is valid.
fn make_component(vm: &mut Vm, owner: ObjRef, template: ObjRef) -> Result<ObjRef, VmError> {
    if vm.defs.is_unloaded(&vm.space, template) {
        let r = vm.defs.ensure_loaded(&mut vm.space, template);
        vm.log.extend(vm.defs.take_log());
        r.map_err(VmError::Def)?;
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

/// The actor attach with no socket (the note's section 4.1, step 6): a zero offset leaves the component's
/// `Translation` and `Rotation` as they are, and the component joins the actor's `Components`.
fn attach_to_actor(vm: &mut Vm, owner: ObjRef, component: ObjRef) -> Result<(), VmError> {
    let class = vm.space.heap.get(owner)?.class;
    let p = vm.space.prop(class, "Components").ok_or_else(|| script(format!("{} has no Components", class_path(vm, class))))?;
    let mut list = vm.space.read(owner, PropAddr::scalar(p))?.clone();
    let Some(items) = list.array_mut() else {
        return Err(script(format!("{}.Components is not an array", class_path(vm, class))));
    };
    items.push(Value::Object(Some(component)));
    vm.space.write(owner, PropAddr::scalar(p), list)?;
    Ok(())
}

/// The script native (`InstanceDataHelper.InitializeBodyComposition(Owner, BodyComposition, InstanceDataSet,
/// ReplicatedInstanceDataState, optional Mode)`), static; the two structs are out arguments.
fn native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let owner = match c.arg(0)? {
        Value::Object(o) => *o,
        _ => None,
    };
    let inputs = Inputs {
        owner,
        composition: c.arg(1)?.clone(),
        set: c.arg(2)?.clone(),
        replicated: c.arg(3)?.clone(),
        mode: if c.arg_count() > 4 { byte(c.arg(4)?) } else { 0 },
    };
    build(c, inputs, &Sink::Out { set: 2, replicated: 3 })?;
    Ok(Value::None)
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
