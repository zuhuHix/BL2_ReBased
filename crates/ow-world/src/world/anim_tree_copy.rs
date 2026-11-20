//! Owner: lane IMPL-COPY-ANIM-TREE (decision 2026-11-19-172, number pending). The animation tree's copy
//! (`CopyAnimTree`, fresh path) and its initialisation on a skeletal mesh component, in `ParityMode::Off`.
//! AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from `docs/verification/NATIVE_COPY_ANIM_TREE.md` alone (analyst lane
//! ANALYST-COPY-ANIM-TREE, PR #551, accepted at 4a363c0c) with NATIVE_ANIM_NODE_LOOKUP.md (#543 at b09720f6) and the hub's
//! rulings and port answers; this lane did not read the executable. **Every rule is UNVERIFIED in the running game.**
//! Off only; Reference binds nothing here.
//!
//! **The preflight** ([`plan`]) reads only, writes and loads nothing, and raises every refusal before the first write:
//! 1. an unloaded handle (the template, or any node or control the gather reaches) is #543's refusal, kept unchanged:
//!    `NotYet { LayerAbsent }` ([`crate::world::anim_node_lookup::refusal`]);
//! 2. the declared fields the copy and the initialisation use (a missing one is a `VmError::Script` naming it);
//! 3. the link fix-ups (the note's rule 2), each `NotYet { NotRead }`: a node class whose fix-up was not checked
//!    ([`fix_up`]: any class outside [`CHECKED`], the classes of Maya's tree the note checked); the aim-offset node
//!    ([`AIM_OFFSET`]: its fix-up's last call is unread); any morph node ([`fix_up`]). The check is by exact class: the
//!    note's inheritance rule (a subclass that does not override a checked fix-up uses it) needs to know which classes
//!    override, which the port cannot tell, so a subclass outside the set is refused;
//! 4. rule 2a, a root with more than one child slot ([`ROOT_CHILDREN`]); rule 2b, the template's
//!    `bRebuildAnimTickArray` set ([`REBUILD`]) or its `bParentNodeArrayBuilt` clear ([`PARENTS`]); all
//!    `NotYet { NotRead }`;
//! 5. the component must be a `SkeletalMeshComponent` (a `VmError::Script` otherwise, review r1 of #561);
//! 6. the initialisation runs when the component's `bAnimTreeInitialised` is clear (the call does not force it; the
//!    note's section 2). Then: a gathered node whose `NodeTickTag` already equals the component's next tick tag is
//!    `NotYet { NotRead }` ([`TAG_COLLISION`]): the copies take their templates' tags, so the tick rule would take such
//!    a node as listed (how the game avoids it was not read; hub ruling, review r1 of #561); then the initialisation's
//!    unread steps (the note's section 2, steps 4 to 6), in order: the per-node initialisation ([`PER_NODE_INIT`],
//!    every tree), the pass over the component ([`COMPONENT_PASS`], this lane's text), the per-bone control arrays
//!    (`NotYet { LayerAbsent }`, [`BONE_ARRAYS`] with `body_composition::MESH_LAYER`) and the owner's event
//!    ([`OWNER_EVENT`]). On installed data these make the write path unreachable; a test-only switch answers them with
//!    nothing to keep it covered (as #542 does). With the flag set, the initialisation is skipped and none of these
//!    apply.
//!
//! **The writes** ([`instance`]), in the note's order, all in the VM heap, under #518's counters (a `NotYet` after a
//! counter moved is marked "stopped after partial writes"):
//! 1. the copy (section 1.2, fresh path; the port has no pool): the tree, then each node in gather order, each with the
//!    component as outer and its template object's values (the port's archetype: the heap has no archetype link, so a
//!    copy takes its template's slots at copy time); `AnimTreeTemplate` names the original; the links through the maps
//!    (`Children[i].Anim` of blend nodes, the root's first child, `ParentNodes`, the cached `AnimTickArray`); the
//!    `AnimGroups` run-time lists emptied; `bRebuildAnimTickArray` copied; then the controls (section 1.4) with
//!    `NextControl` and the `SkelControlLists` heads mapped;
//! 2. the component's `Animations` = the copy: the `SetAnimTreeTemplate` path's write (#495), made before the
//!    initialisation (the hub's port answer). The #495 integration must not write `Animations` a second time;
//! 3. the initialisation (section 2): the tick counter (`TickTag`) up by one, the tick list built by the note's rule
//!    (always built: the hub's port answer; the unidentified gate field counts as clear), stored on the component and
//!    in the tree's cache, each node's `SkelComponent`, `TickArrayIndex` and `NodeTickTag`; the component's
//!    `bAnimTreeInitialised` set. Skipped when that flag was already set.
//!
//! **Left out, port answers, UNVERIFIED:** the post-construction call each new copy gets (the note's open question 5;
//! its effect was not read), and the engine-wide "duplicating" flag set around the copies (its effect was not read).
//!
//! **The root in the map.** The note's map holds the nodes after the root is removed; the port also maps the template
//! root to the new tree, so a child's `ParentNodes` entry for the root names the copy. Without it the tick rule could
//! never list the root's children (their parent would be the untagged template). A port reading, UNVERIFIED.

use ow_core::{DetMap, DetSet, NotYet};
use ow_object::heap::Origin;
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::world::body_composition::{Counters, after_writes, field, set_field};

/// The analyst note.
pub const RECORD: &str = "docs/verification/NATIVE_COPY_ANIM_TREE.md";
const UNIT: &str = "CopyAnimTree";

pub const TREE: &str = "Engine.AnimTree";
pub const NODE: &str = "Engine.AnimNode";
pub const BLEND: &str = "Engine.AnimNodeBlendBase";
pub const SEQUENCE: &str = "Engine.AnimNodeSequence";
pub const SKELETAL: &str = "Engine.SkeletalMeshComponent";

/// The node classes whose link fix-up the note checked (its section 1.2 step 4: `AnimNode`, `AnimNodeBlendBase`,
/// `AnimNodeSequence`, `AnimNodeSlot` and every node class of Maya's tree, all doing nothing), the aim-offset node
/// excepted ([`AIM_OFFSET_CLASS`]). The tree's classes are the class names of `AnimTree_Player`'s node exports in the
/// installed packages (names only).
pub const CHECKED: [&str; 18] = [
    NODE,
    BLEND,
    SEQUENCE,
    "Engine.AnimNodeSlot",
    "Engine.AnimNodeAdditiveBlending",
    "Engine.AnimNodeBlendByPhysics",
    "Engine.AnimNodeBlendByPosture",
    "Engine.AnimNodeBlendBySpeed",
    "Engine.AnimNodeBlendDirectional",
    "Engine.AnimNodeBlendPerBone",
    "Engine.AnimNodeScaleRateBySpeed",
    "GearboxFramework.AnimNodeSpecialMoveBlend",
    "WillowGame.WillowAnimNodeBlendInjured",
    "WillowGame.WillowAnimNodeBlendThirdPersonMenu",
    "WillowGame.WillowAnimNodeFeatherBlend",
    "WillowGame.WillowAnimNodeSequenceAdditiveBlend",
    "WillowGame.WillowAnimNode_ClimbLadder",
    "WillowGame.WillowAnimNode_Condition",
];
/// The aim-offset node, whose fix-up sets the copy's `TemplateNode` and then makes an unread call.
pub const AIM_OFFSET_CLASS: &str = "WillowGame.WillowAnimNodeAimOffset";
pub const AIM_OFFSET: &str = "CopyAnimTree: the aim-offset node's refresh after its TemplateNode is set";
/// Rules 2a and 2b (#551 r1), the note's texts.
pub const ROOT_CHILDREN: &str = "CopyAnimTree: a root with more than one child";
pub const REBUILD: &str = "the animation tree's rebuild step (the unidentified class whose extra children it drops)";
pub const PARENTS: &str = "the animation tree's parent node array build";
/// A gathered node whose `NodeTickTag` equals the component's next tick tag (hub ruling, review r1 of #561).
pub const TAG_COLLISION: &str =
    "CopyAnimTree: a template node whose NodeTickTag equals the component's next tick tag (how the game keeps the \
     copies' tags apart was not read)";
/// The initialisation's unread steps (the note's section 5, rules 3 and 4), raised in this order; while
/// [`PER_NODE_INIT`] refuses every tree, the other three are reached only under the test switch.
pub const PER_NODE_INIT: &str = "the animation tree's per-node initialisation";
pub const COMPONENT_PASS: &str = "the animation tree initialisation's pass over the component";
pub const BONE_ARRAYS: &str = "CopyAnimTree: the component's per-bone control arrays";
pub const OWNER_EVENT: &str = "the owner's event after the tree initialisation";

/// `"CopyAnimTree: the link fix-up of <class>"` (the note's section 5, rule 2).
pub fn fix_up(class: &str) -> String {
    format!("CopyAnimTree: the link fix-up of {class}")
}

thread_local! {
    static INIT_STEPS_ANSWERED: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

/// The initialisation's four unread steps.
#[cfg(test)]
const INIT_STEPS: u8 = 4;

/// For the tests only: answer the initialisation's unread steps with nothing on this thread, to keep the write path
/// covered while production refuses it.
#[cfg(test)]
pub(crate) fn answer_init_steps_for_tests(on: bool) {
    INIT_STEPS_ANSWERED.with(|c| c.set(if on { INIT_STEPS } else { 0 }));
}

/// For the tests only: answer the first `n` of the four unread steps, so the next one's refusal shows.
#[cfg(test)]
pub(crate) fn answer_init_steps_up_to_for_tests(n: u8) {
    INIT_STEPS_ANSWERED.with(|c| c.set(n.min(INIT_STEPS)));
}

fn init_steps_answered() -> u8 {
    INIT_STEPS_ANSWERED.with(std::cell::Cell::get)
}

thread_local! {
    static ATTACH_CHECK_SKIPPED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// For the tests only: skip [`attach_check`] on this thread, so the body composition's lookup tests (#557, whose
/// fixture tree has no copy fields) reach the lookups the check now comes before.
#[cfg(test)]
pub(crate) fn skip_attach_check_for_tests(on: bool) {
    ATTACH_CHECK_SKIPPED.with(|c| c.set(on));
}

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message }
}

fn not_read(what: impl Into<String>) -> VmError {
    NotYet::not_read(what.into(), RECORD).into()
}

fn object_of(v: &Value) -> Option<ObjRef> {
    match v {
        Value::Object(o) | Value::Interface(o) => *o,
        _ => None,
    }
}

fn class_of(vm: &Vm, o: ObjRef) -> Result<ClassId, VmError> {
    Ok(vm.space.heap.get(o)?.class)
}

fn class_path(vm: &Vm, class: ClassId) -> String {
    vm.space.classes.get_layout(class).map_or_else(|| format!("{class:?}"), |l| l.path().as_str().to_string())
}

fn is_a(vm: &Vm, class: ClassId, path: &str) -> bool {
    vm.space.classes.by_path(path).is_some_and(|p| vm.space.classes.is_child_of(class, p))
}

fn read(vm: &Vm, o: ObjRef, name: &str) -> Result<Value, VmError> {
    let class = class_of(vm, o)?;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{} has no {name}", class_path(vm, class))))?;
    Ok(vm.space.read(o, PropAddr::scalar(p))?.clone())
}

fn write(vm: &mut Vm, o: ObjRef, name: &str, v: Value) -> Result<(), VmError> {
    let class = class_of(vm, o)?;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{} has no {name}", class_path(vm, class))))?;
    vm.space.write(o, PropAddr::scalar(p), v)?;
    Ok(())
}

fn elements(v: &Value, what: &str) -> Result<Vec<Value>, VmError> {
    Ok(v.elements().ok_or_else(|| script(format!("{what} is not an array")))?.to_vec())
}

fn truth(v: &Value) -> bool {
    matches!(v, Value::Bool(true)) || matches!(v, Value::Int(i) if *i != 0) || matches!(v, Value::Byte(b) if *b != 0)
}

fn int(v: &Value) -> i32 {
    match v {
        Value::Int(i) => *i,
        _ => 0,
    }
}

/// #543's refusal for an unloaded (or failed) handle, raised with nothing loaded: the lookup lane's own
/// ([`crate::world::anim_node_lookup::refusal`]), so both lanes refuse with one text, layer and record.
fn unloaded(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) {
        return Err(crate::world::anim_node_lookup::refusal());
    }
    Ok(())
}

/// The attach's check for a made `Mesh` from component template `template` (the body composition's preflight, before
/// the animation-node lookups): the attach instances the template's tree first (copy, then initialisation; the note's
/// section 2 and NATIVE_ANIM_NODE_LOOKUP.md section 3), so its write-free preflight ([`plan`]) runs here, with the
/// component template standing for the new component (it does not exist before the writes). An unloaded template is
/// the lookup's refusal; no `AnimTreeTemplate` has nothing to copy. Reads only. The instance's writes join the made
/// `Mesh`'s writes when the body composition's `Mesh` step has an Off answer past the lookups.
pub fn attach_check(vm: &Vm, template: ObjRef) -> Result<(), VmError> {
    if ATTACH_CHECK_SKIPPED.with(std::cell::Cell::get) {
        return Ok(());
    }
    unloaded(vm, template)?;
    let Some(tree) = object_of(&read(vm, template, "AnimTreeTemplate")?) else {
        return Ok(());
    };
    plan(vm, template, tree).map(|_| ())
}

/// The `Anim` of each set child slot of node `o` (none when its class is not a blend node), in slot order.
fn children(vm: &Vm, o: ObjRef) -> Result<Vec<ObjRef>, VmError> {
    if !is_a(vm, class_of(vm, o)?, BLEND) {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for slot in elements(&read(vm, o, "Children")?, "Children")? {
        if let Some(a) = object_of(&field(vm, &slot, "Anim", "AnimBlendChild")?) {
            out.push(a);
        }
    }
    Ok(out)
}

/// The preflight's result: the template's objects in the order the copy makes them.
#[derive(Debug, Clone)]
pub struct Plan {
    pub template: ObjRef,
    /// The nodes in gather order (section 1.3), the root excluded.
    pub nodes: Vec<ObjRef>,
    /// The controls in `SkelControlLists` order, each list's head then its `NextControl` chain (section 1.4).
    pub controls: Vec<ObjRef>,
    /// The initialisation runs (the component's `bAnimTreeInitialised` was clear).
    pub initialise: bool,
}

/// The gather order (section 1.3): depth first from the root, a node taken the first time it is reached, then its
/// children in slot order; empty slots skipped. Each reached handle must be loaded (rule 1).
fn gather(vm: &Vm, root: ObjRef) -> Result<Vec<ObjRef>, VmError> {
    let mut seen = DetSet::new();
    let mut out = Vec::new();
    // An explicit stack of child lists, so a deep tree does not recurse.
    seen.insert(root);
    out.push(root);
    let mut stack: Vec<std::vec::IntoIter<ObjRef>> = vec![children(vm, root)?.into_iter()];
    while let Some(top) = stack.last_mut() {
        let Some(c) = top.next() else {
            stack.pop();
            continue;
        };
        if !seen.insert(c) {
            continue;
        }
        unloaded(vm, c)?;
        out.push(c);
        stack.push(children(vm, c)?.into_iter());
    }
    Ok(out)
}

/// The controls of `SkelControlLists`, in order (section 1.4), by #557's chain rule (6803b3b): a repeat within one
/// chain is a loop, a `VmError::Script`; a control an earlier list already reached keeps its first place.
fn controls(vm: &Vm, tree: ObjRef) -> Result<Vec<ObjRef>, VmError> {
    let mut seen = DetSet::new();
    let mut out = Vec::new();
    for list in elements(&read(vm, tree, "SkelControlLists")?, "SkelControlLists")? {
        let mut chain = DetSet::new();
        let mut next = object_of(&field(vm, &list, "ControlHead", "SkelControlListHead")?);
        while let Some(c) = next {
            if !chain.insert(c) {
                return Err(script("a skeletal control list that loops back on itself".into()));
            }
            unloaded(vm, c)?;
            if seen.insert(c) {
                out.push(c);
            }
            next = object_of(&read(vm, c, "NextControl")?);
        }
    }
    Ok(out)
}

/// The fields a node of class `c` must declare (`Children` for a blend node).
fn node_fields(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    for name in ["ParentNodes", "SkelComponent", "NodeTickTag", "TickArrayIndex"] {
        read(vm, o, name)?;
    }
    for p in elements(&read(vm, o, "ParentNodes")?, "ParentNodes")? {
        if !matches!(p, Value::Object(_)) {
            return Err(script("ParentNodes holds a value that is not an object".into()));
        }
    }
    Ok(())
}

/// The preflight (module documentation) for copying `template` onto `component` and initialising it.
pub fn plan(vm: &Vm, component: ObjRef, template: ObjRef) -> Result<Plan, VmError> {
    // 1. The template must be loaded (#543's refusal), and an AnimTree.
    unloaded(vm, template)?;
    let tree_class = class_of(vm, template)?;
    if !is_a(vm, tree_class, TREE) {
        return Err(script(format!("the template is a {}, not an AnimTree", class_path(vm, tree_class))));
    }
    // 2. The declared fields; the component is a skeletal mesh component (review r1 of #561, item 3).
    let component_class = class_of(vm, component)?;
    if !is_a(vm, component_class, SKELETAL) {
        return Err(script(format!(
            "the component is a {}, not a SkeletalMeshComponent",
            class_path(vm, component_class)
        )));
    }
    for name in ["AnimTreeTemplate", "Animations", "AnimTickArray", "TickTag", "bAnimTreeInitialised"] {
        read(vm, component, name)?;
    }
    for name in [
        "AnimTreeTemplate",
        "bParentNodeArrayBuilt",
        "bRebuildAnimTickArray",
        "AnimGroups",
        "RootMorphNodes",
        "SkelControlLists",
        "AnimTickArray",
        "Children",
    ] {
        read(vm, template, name)?;
    }
    node_fields(vm, template)?;
    for group in elements(&read(vm, template, "AnimGroups")?, "AnimGroups")? {
        for name in ["SeqNodes", "SynchMaster", "NotifyMaster"] {
            field(vm, &group, name, "AnimGroup")?;
        }
    }
    // Rule 1 on each handle the gather reaches, then rule 2 (the fix-ups).
    let mut nodes = gather(vm, template)?;
    nodes.remove(0);
    let controls = controls(vm, template)?;
    for &n in &nodes {
        node_fields(vm, n)?;
        let path = class_path(vm, class_of(vm, n)?);
        if path == AIM_OFFSET_CLASS {
            return Err(not_read(AIM_OFFSET));
        }
        if !CHECKED.contains(&path.as_str()) {
            return Err(not_read(fix_up(&path)));
        }
    }
    for &c in &controls {
        read(vm, c, "NextControl")?;
    }
    for m in elements(&read(vm, template, "RootMorphNodes")?, "RootMorphNodes")? {
        if let Some(m) = object_of(&m) {
            unloaded(vm, m)?;
            let path = class_path(vm, class_of(vm, m)?);
            return Err(not_read(fix_up(&path)));
        }
    }
    // Rule 2a, rule 2b, and the initialisation's gate.
    if elements(&read(vm, template, "Children")?, "Children")?.len() > 1 {
        return Err(not_read(ROOT_CHILDREN));
    }
    if truth(&read(vm, template, "bRebuildAnimTickArray")?) {
        return Err(not_read(REBUILD));
    }
    if !truth(&read(vm, template, "bParentNodeArrayBuilt")?) {
        return Err(not_read(PARENTS));
    }
    // The initialisation (when the flag is clear): the tag collision, then its unread steps in order.
    let initialise = !truth(&read(vm, component, "bAnimTreeInitialised")?);
    if initialise {
        let next = int(&read(vm, component, "TickTag")?).wrapping_add(1);
        for &n in &nodes {
            if int(&read(vm, n, "NodeTickTag")?) == next {
                return Err(not_read(TAG_COLLISION));
            }
        }
        let answered = init_steps_answered();
        let steps: [VmError; 4] = [
            not_read(PER_NODE_INIT),
            not_read(COMPONENT_PASS),
            NotYet::layer_absent(BONE_ARRAYS, crate::world::body_composition::MESH_LAYER, RECORD).into(),
            not_read(OWNER_EVENT),
        ];
        if let Some(e) = steps.into_iter().nth(usize::from(answered)) {
            return Err(e);
        }
    }
    Ok(Plan { template, nodes, controls, initialise })
}

/// A copy of `t` with outer `outer`: its class and name, and its values (the port's archetype, module documentation).
fn duplicate(vm: &mut Vm, t: ObjRef, outer: ObjRef) -> Result<ObjRef, VmError> {
    let (class, name, slots) = {
        let o = vm.space.heap.get(t)?;
        (o.class, o.name, o.slots.clone())
    };
    let name = vm.space.names.text(name).to_string();
    let made = vm.space.alloc(class, &name, Some(outer), Origin::Runtime)?;
    vm.space.heap.get_mut(made)?.slots = slots;
    vm.space.heap.note_write();
    Ok(made)
}

fn mapped(map: &DetMap<ObjRef, ObjRef>, v: &Value) -> Value {
    match v {
        Value::Object(Some(o)) => Value::Object(Some(map.get(o).copied().unwrap_or(*o))),
        other => other.clone(),
    }
}

fn map_array(vm: &mut Vm, o: ObjRef, name: &str, map: &DetMap<ObjRef, ObjRef>, from: Value) -> Result<(), VmError> {
    let mut v = from;
    if let Some(items) = v.array_mut() {
        for item in items.iter_mut() {
            *item = mapped(map, item);
        }
    } else {
        return Err(script(format!("{name} is not an array")));
    }
    write(vm, o, name, v)
}

/// The `Children[i].Anim` links of `o` mapped; with `first_only`, only slot 0's.
fn map_children(vm: &mut Vm, o: ObjRef, map: &DetMap<ObjRef, ObjRef>, first_only: bool) -> Result<(), VmError> {
    let mut v = read(vm, o, "Children")?;
    let Some(items) = v.array_mut() else {
        return Err(script("Children is not an array".into()));
    };
    let mut slots = std::mem::take(items);
    for (i, slot) in slots.iter_mut().enumerate() {
        if first_only && i > 0 {
            break;
        }
        let a = field(vm, slot, "Anim", "AnimBlendChild")?;
        set_field(vm, slot, "Anim", mapped(map, &a), "AnimBlendChild")?;
    }
    *v.array_mut().expect("checked") = slots;
    write(vm, o, "Children", v)
}

/// The copy (module documentation, write step 1): the new tree.
fn copy(vm: &mut Vm, component: ObjRef, plan: &Plan) -> Result<ObjRef, VmError> {
    let t = plan.template;
    // 1. The root, and the original it names.
    let root = duplicate(vm, t, component)?;
    let original = object_of(&read(vm, t, "AnimTreeTemplate")?).unwrap_or(t);
    write(vm, root, "AnimTreeTemplate", Value::Object(Some(original)))?;
    let mut map = DetMap::new();
    map.insert(t, root);
    // 2. The nodes, then the blend nodes' child links.
    let mut made = Vec::with_capacity(plan.nodes.len());
    for &n in &plan.nodes {
        let c = duplicate(vm, n, component)?;
        map.insert(n, c);
        made.push((n, c));
    }
    for &(_, c) in &made {
        if is_a(vm, class_of(vm, c)?, BLEND) {
            map_children(vm, c, &map, false)?;
        }
    }
    // 3. The root's first child.
    map_children(vm, root, &map, true)?;
    // 4. Each node's ParentNodes from its template node's (the per-class fix-ups of these classes do nothing).
    for &(n, c) in &made {
        let parents = read(vm, n, "ParentNodes")?;
        map_array(vm, c, "ParentNodes", &map, parents)?;
    }
    // 5. The tree's fix-up: the groups' run-time state, and the cached tick list unless a rebuild is asked for.
    let mut groups = read(vm, root, "AnimGroups")?;
    if let Some(items) = groups.array_mut() {
        let mut list = std::mem::take(items);
        for g in &mut list {
            let empty = field(vm, g, "SeqNodes", "AnimGroup")?.elements().map(|_| Value::array(Vec::new()));
            set_field(vm, g, "SeqNodes", empty.unwrap_or(Value::array(Vec::new())), "AnimGroup")?;
            set_field(vm, g, "SynchMaster", Value::Object(None), "AnimGroup")?;
            set_field(vm, g, "NotifyMaster", Value::Object(None), "AnimGroup")?;
        }
        *groups.array_mut().expect("checked") = list;
    }
    write(vm, root, "AnimGroups", groups)?;
    if !truth(&read(vm, root, "bRebuildAnimTickArray")?) {
        let cached = read(vm, t, "AnimTickArray")?;
        map_array(vm, root, "AnimTickArray", &map, cached)?;
    }
    // 6. The flag.
    let flag = read(vm, t, "bRebuildAnimTickArray")?;
    write(vm, root, "bRebuildAnimTickArray", flag)?;
    // The controls (section 1.4), their chain, and the list heads.
    let mut cmap = DetMap::new();
    let mut cmade = Vec::with_capacity(plan.controls.len());
    for &c in &plan.controls {
        let n = duplicate(vm, c, component)?;
        cmap.insert(c, n);
        cmade.push(n);
    }
    for &n in &cmade {
        let next = read(vm, n, "NextControl")?;
        write(vm, n, "NextControl", mapped(&cmap, &next))?;
    }
    let mut lists = read(vm, root, "SkelControlLists")?;
    if let Some(items) = lists.array_mut() {
        let mut list = std::mem::take(items);
        for l in &mut list {
            let head = field(vm, l, "ControlHead", "SkelControlListHead")?;
            set_field(vm, l, "ControlHead", mapped(&cmap, &head), "SkelControlListHead")?;
        }
        *lists.array_mut().expect("checked") = list;
    }
    write(vm, root, "SkelControlLists", lists)?;
    Ok(root)
}

/// The tick list of the tree `root` (section 2, step 3), with `tag` the component's new counter: the root first, then
/// depth first from the root, each child that is set and not yet tagged gets the component; it is appended only when
/// every node in its `ParentNodes` already carries the tag, and then its own children are visited. Writes each node's
/// `SkelComponent`, `TickArrayIndex` and `NodeTickTag` as it goes.
pub fn build_tick_list(vm: &mut Vm, component: ObjRef, root: ObjRef, tag: i32) -> Result<Vec<ObjRef>, VmError> {
    let mut list = vec![root];
    write(vm, root, "TickArrayIndex", Value::Int(0))?;
    write(vm, root, "SkelComponent", Value::Object(Some(component)))?;
    write(vm, root, "NodeTickTag", Value::Int(tag))?;
    let mut stack: Vec<std::vec::IntoIter<ObjRef>> = vec![children(vm, root)?.into_iter()];
    while let Some(top) = stack.last_mut() {
        let Some(c) = top.next() else {
            stack.pop();
            continue;
        };
        if int(&read(vm, c, "NodeTickTag")?) == tag {
            continue;
        }
        write(vm, c, "SkelComponent", Value::Object(Some(component)))?;
        let mut ready = true;
        for p in elements(&read(vm, c, "ParentNodes")?, "ParentNodes")? {
            match object_of(&p) {
                Some(p) if int(&read(vm, p, "NodeTickTag")?) == tag => {}
                Some(_) => ready = false,
                None => {}
            }
        }
        if !ready {
            continue;
        }
        let index = i32::try_from(list.len()).map_err(|_| script("the tick list is past an int".into()))?;
        write(vm, c, "TickArrayIndex", Value::Int(index))?;
        write(vm, c, "NodeTickTag", Value::Int(tag))?;
        list.push(c);
        stack.push(children(vm, c)?.into_iter());
    }
    Ok(list)
}

/// The initialisation (module documentation, write step 3) of `component`'s tree `root`.
fn initialise(vm: &mut Vm, component: ObjRef, root: ObjRef) -> Result<(), VmError> {
    let tag = int(&read(vm, component, "TickTag")?).wrapping_add(1);
    write(vm, component, "TickTag", Value::Int(tag))?;
    write(vm, component, "AnimTickArray", Value::array(Vec::new()))?;
    let list = build_tick_list(vm, component, root, tag)?;
    let values: Vec<Value> = list.iter().map(|o| Value::Object(Some(*o))).collect();
    write(vm, root, "AnimTickArray", Value::array(values.clone()))?;
    write(vm, component, "AnimTickArray", Value::array(values))?;
    // Step 4: each node in tick order (its class's initialisation is answered with nothing only under the test switch).
    for &n in &list {
        write(vm, n, "SkelComponent", Value::Object(Some(component)))?;
        write(vm, n, "NodeTickTag", Value::Int(tag))?;
    }
    // Steps 5 and 6 are unread: refused in the preflight unless the test switch answers them.
    write(vm, component, "bAnimTreeInitialised", Value::Bool(true))
}

/// Copies `template` onto `component`, sets the component's `Animations` and initialises the tree (module
/// documentation); the new tree. The preflight first, then the writes under #518's counters.
pub fn instance(vm: &mut Vm, component: ObjRef, template: ObjRef) -> Result<ObjRef, VmError> {
    let before = Counters::read(vm);
    let plan = plan(vm, component, template)?;
    let run = |vm: &mut Vm| -> Result<ObjRef, VmError> {
        let root = copy(vm, component, &plan)?;
        write(vm, component, "Animations", Value::Object(Some(root)))?;
        if plan.initialise {
            initialise(vm, component, root)?;
        }
        Ok(root)
    };
    run(vm).map_err(|e| after_writes(vm, before, e))
}

#[cfg(test)]
#[path = "anim_tree_copy_tests.rs"]
mod tests;
