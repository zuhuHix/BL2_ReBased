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
//!    every tree), the init tag, the two per-node arrays, the pass over the component and the per-bone control arrays
//!    (planned by `crate::world::control_arrays`, lane IMPL-COPY-COMPONENT-PASS) and the owner's event
//!    (`PostInitAnimTree`, lane IMPL-TREE-INIT-OWNER-EVENT, NATIVE_TREE_INIT_OWNER_EVENT.md: [`owner_event`]).
//!    With the flag set, the initialisation is skipped and none of these apply.
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
//!    in the tree's cache, each node's `SkelComponent`, `TickArrayIndex` and `NodeTickTag`; the owner's
//!    `PostInitAnimTree(component)` run through the VM when the preflight planned it; the component's
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
/// The aim-offset classes, whose fix-up (NATIVE_ANIM_NODE_INIT.md section 1, lane IMPL-ANIM-NODE-INIT) sets the
/// copy's `TemplateNode` to its source node and empties the copy's own `Profiles`, when the outers differ.
pub const AIM_OFFSET_CLASS: &str = "WillowGame.WillowAnimNodeAimOffset";
pub const AIM_OFFSET_CLASSES: [&str; 2] = ["Engine.AnimNodeAimOffset", AIM_OFFSET_CLASS];
/// Rules 2a and 2b (#551 r1), the note's texts.
pub const ROOT_CHILDREN: &str = "CopyAnimTree: a root with more than one child";
pub const REBUILD: &str = "the animation tree's rebuild step (the unidentified class whose extra children it drops)";
pub const PARENTS: &str = "the animation tree's parent node array build";
/// A gathered node whose `NodeTickTag` equals the component's next tick tag (hub ruling, review r1 of #561).
pub const TAG_COLLISION: &str =
    "CopyAnimTree: a template node whose NodeTickTag equals the component's next tick tag (how the game keeps the \
     copies' tags apart was not read)";
/// The owner event's note (lane IMPL-TREE-INIT-OWNER-EVENT; #612).
pub const EVENT_RECORD: &str = "docs/verification/NATIVE_TREE_INIT_OWNER_EVENT.md";
/// The event the initialisation sends to the component's owner, with the component as its argument.
pub const EVENT: &str = "PostInitAnimTree";
pub const ACTOR_EVENT: &str = "Engine.Actor.PostInitAnimTree";
pub const PAWN_EVENT: &str = "Engine.Pawn.PostInitAnimTree";
pub const PLAYER_EVENT: &str = "WillowGame.WillowPlayerPawn.PostInitAnimTree";
/// The three versions the note read (compared by path).
pub const READ_EVENTS: [&str; 3] = [ACTOR_EVENT, PAWN_EVENT, PLAYER_EVENT];
/// A chain class of the owner whose code is not loaded (finding the event would be a load).
pub const EVENT_NOT_LOADED: &str = "the owner's PostInitAnimTree on a class whose code is not loaded";
/// `Pawn`'s branch on the owner's own `Mesh`.
pub const OWN_MESH: &str = "the owner's PostInitAnimTree on its own Mesh: Pawn.CacheAnimNodes (native) is not read";
/// An owner in a state (rule 3): `find_loaded_method` reads the state's own table only, the game's lookup the state
/// chain (by fit).
pub const IN_STATE: &str =
    "the owner's PostInitAnimTree while the owner is in a state (the state chain lookup is not planned read-only)";
/// The two lookups `WillowPlayerPawn`'s version calls, each unbound in Off (rule 7).
pub const ANIM_NODE_UNBOUND: &str = "SkeletalMeshComponent.FindAnimNode in Off";
pub const SKEL_CONTROL_UNBOUND: &str = "SkeletalMeshComponent.FindSkelControl in Off";
/// The three classes `WillowPlayerPawn`'s version casts its finds to, with the field each find fills and the name it
/// looks up (`None`: the body class's `AimOffsetNodeName`).
pub const PER_BONE_CLASS: &str = "Engine.AnimNodeBlendPerBone";
pub const LEFT_HAND_CLASS: &str = "WillowGame.SkelControlLeftHandGripWeapon";
pub const CAST_CLASSES: [&str; 3] = [PER_BONE_CLASS, AIM_OFFSET_CLASS, LEFT_HAND_CLASS];

/// `"the cast class <path> is not defined at set-up"` (rule 7).
pub fn cast_undefined(path: &str) -> String {
    format!("the cast class {path} is not defined at set-up")
}

/// `"an override of PostInitAnimTree that was not read (<path>)"`.
pub fn event_override(path: &str) -> String {
    format!("an override of PostInitAnimTree that was not read ({path})")
}

/// Whose event the preflight plans: the component's own `Owner` (the instance), or the actor the attach attaches the
/// template to (`attach_check`; the template's own `Owner` is not it).
#[derive(Debug, Clone, Copy)]
pub enum EventOwner {
    Component,
    Given(Option<ObjRef>),
}

/// `"CopyAnimTree: the link fix-up of <class>"` (the note's section 5, rule 2).
pub fn fix_up(class: &str) -> String {
    format!("CopyAnimTree: the link fix-up of {class}")
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

pub(crate) fn object_of(v: &Value) -> Option<ObjRef> {
    match v {
        Value::Object(o) | Value::Interface(o) => *o,
        _ => None,
    }
}

pub(crate) fn class_of(vm: &Vm, o: ObjRef) -> Result<ClassId, VmError> {
    Ok(vm.space.heap.get(o)?.class)
}

pub(crate) fn class_path(vm: &Vm, class: ClassId) -> String {
    vm.space.classes.get_layout(class).map_or_else(|| format!("{class:?}"), |l| l.path().as_str().to_string())
}

fn is_a(vm: &Vm, class: ClassId, path: &str) -> bool {
    vm.space.classes.by_path(path).is_some_and(|p| vm.space.classes.is_child_of(class, p))
}

pub(crate) fn read(vm: &Vm, o: ObjRef, name: &str) -> Result<Value, VmError> {
    let class = class_of(vm, o)?;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{} has no {name}", class_path(vm, class))))?;
    Ok(vm.space.read(o, PropAddr::scalar(p))?.clone())
}

pub(crate) fn write(vm: &mut Vm, o: ObjRef, name: &str, v: Value) -> Result<(), VmError> {
    let class = class_of(vm, o)?;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{} has no {name}", class_path(vm, class))))?;
    vm.space.write(o, PropAddr::scalar(p), v)?;
    Ok(())
}

pub(crate) fn elements(v: &Value, what: &str) -> Result<Vec<Value>, VmError> {
    Ok(v.elements().ok_or_else(|| script(format!("{what} is not an array")))?.to_vec())
}

pub(crate) fn truth(v: &Value) -> bool {
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
///
/// `owner` is the actor the component is attached to, whose `PostInitAnimTree` the initialisation will send
/// (NATIVE_TREE_INIT_OWNER_EVENT.md section 3, step 1).
pub fn attach_check(vm: &Vm, template: ObjRef, owner: Option<ObjRef>) -> Result<(), VmError> {
    attach_plan(vm, template, owner).map(|_| ())
}

/// [`attach_check`] with its plan: `None` when there is no tree to copy (or the check is switched off in tests). The
/// body composition's lookups search the planned copy (lane FIX-BODY-COMPOSITION-LOOKUP-STALE); the plan's owner
/// event is planned for `owner` (lane IMPL-TREE-INIT-OWNER-EVENT).
pub fn attach_plan(vm: &Vm, template: ObjRef, owner: Option<ObjRef>) -> Result<Option<Plan>, VmError> {
    if ATTACH_CHECK_SKIPPED.with(std::cell::Cell::get) {
        return Ok(None);
    }
    unloaded(vm, template)?;
    let Some(tree) = object_of(&read(vm, template, "AnimTreeTemplate")?) else {
        return Ok(None);
    };
    plan_with(vm, template, tree, EventOwner::Given(owner)).map(Some)
}

/// The `Anim` of each set child slot of node `o` (none when its class is not a blend node), in slot order.
fn children(vm: &Vm, o: ObjRef) -> Result<Vec<ObjRef>, VmError> {
    if !is_a(vm, class_of(vm, o)?, BLEND) {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for slot in elements(&read(vm, o, "Children")?, "Children")? {
        // Only an object-typed link: the copy remaps object links, so an interface-typed one would be planned as a
        // copied node and walked as its source (review r2 of #575, item 2). The preflight's gather meets it first.
        match field(vm, &slot, "Anim", "AnimBlendChild")? {
            Value::Object(Some(a)) => out.push(a),
            Value::Object(None) => {}
            other => {
                return Err(script(format!(
                    "{}: a child link holds {}, not an object reference",
                    class_path(vm, class_of(vm, o)?),
                    other.kind_name()
                )));
            }
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
    /// The initialisation's planned steps, on template nodes (lane IMPL-ANIM-NODE-INIT, review r1 of #575).
    pub steps: Vec<crate::world::anim_node_init::Step>,
    /// The planned tick list, on template nodes.
    pub order: Vec<ObjRef>,
    /// The F heads' planned builds, which their steps index (review r1 of #585, item 2).
    pub builds: Vec<crate::world::per_bone_weights::Build>,
    /// Step 5's planned arrays, on template controls (lane IMPL-COPY-COMPONENT-PASS).
    pub arrays: crate::world::control_arrays::Arrays,
    /// The owner's `PostInitAnimTree` to run after step 5 (lane IMPL-TREE-INIT-OWNER-EVENT): the owner and the event.
    pub event: Option<(ObjRef, ow_object::ids::FuncId)>,
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

/// The preflight (module documentation) for copying `template` onto `component` and initialising it; the event goes to
/// the component's own `Owner`.
pub fn plan(vm: &Vm, component: ObjRef, template: ObjRef) -> Result<Plan, VmError> {
    plan_with(vm, component, template, EventOwner::Component)
}

/// [`plan`] with the event's owner chosen by `owner`.
pub fn plan_with(vm: &Vm, component: ObjRef, template: ObjRef, owner: EventOwner) -> Result<Plan, VmError> {
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
        if AIM_OFFSET_CLASSES.contains(&path.as_str()) {
            // The aim-offset fix-up's fields (lane IMPL-ANIM-NODE-INIT), and, when it will run (the copy's outer, the
            // component, differs from the source's), the note's checked cast of the source to AnimNodeAimOffset
            // (review r1 of #575, item 5).
            read(vm, n, "TemplateNode")?;
            elements(&read(vm, n, "Profiles")?, "Profiles")?;
            if vm.space.heap.get(n)?.outer != Some(component) && !is_a(vm, class_of(vm, n)?, "Engine.AnimNodeAimOffset") {
                return Err(script(format!("{path} is not an Engine.AnimNodeAimOffset (the fix-up's checked cast)")));
            }
            continue;
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
    let mut order = Vec::new();
    let mut steps = Vec::new();
    let mut builds = Vec::new();
    let mut arrays = crate::world::control_arrays::Arrays::default();
    let mut event = None;
    if initialise {
        let next = int(&read(vm, component, "TickTag")?).wrapping_add(1);
        for &n in &nodes {
            if int(&read(vm, n, "NodeTickTag")?) == next {
                return Err(not_read(TAG_COLLISION));
            }
        }
        // The per-node initialisation (lane IMPL-ANIM-NODE-INIT): the tick list by the writes' own parent rule, then the
        // planned steps and their checks (review r1 of #575, item 1).
        let tree: DetSet<ObjRef> = std::iter::once(template).chain(nodes.iter().copied()).collect();
        let planned_order = tick_order(vm, template, &tree, next)?;
        let (planned_steps, planned_builds) = crate::world::anim_node_init::plan(vm, component, &planned_order)?;
        order = planned_order;
        steps = planned_steps;
        builds = planned_builds;
        // The init tag, the per-node arrays and step 5, the component pass and the per-bone control arrays (lane
        // IMPL-COPY-COMPONENT-PASS, NATIVE_COPY_COMPONENT_PASS.md): planned on the template, the copies keeping their
        // templates' values.
        let tick_tag = int(&read(vm, component, "TickTag")?);
        arrays = crate::world::control_arrays::plan(vm, component, template, tick_tag, order.len())?;
        // The owner's event (#551 rule 4; NATIVE_TREE_INIT_OWNER_EVENT.md section 3).
        event = owner_event(vm, component, template, &order, &controls, owner)?;
    }
    Ok(Plan { template, nodes, controls, initialise, steps, order, builds, arrays, event })
}

fn event_not_read(what: impl Into<String>) -> VmError {
    NotYet::not_read(what.into(), EVENT_RECORD).into()
}

/// `name` declared on `o` as an object reference, else a `VmError::Script` naming it.
fn object_field(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    let class = class_of(vm, o)?;
    let declared = vm.space.prop(class, name).and_then(|p| vm.space.classes.decl(p)).map(|d| d.kind.clone());
    match declared {
        Some(ow_object::layout::PropKind::Object { .. }) => Ok(object_of(&read(vm, o, name)?)),
        other => Err(script(format!("{} declares {name} as {other:?}, not an object reference", class_path(vm, class)))),
    }
}

/// The owner event's preflight (NATIVE_TREE_INIT_OWNER_EVENT.md section 3, rules 1 to 8), write-free and load-free:
/// the owner and the event to send, `None` when nothing is sent. `order` is the planned tick list and `controls` the
/// planned control order, on template objects: the copies keep each one's class and name (#543 rule 3), so a find on
/// the copy answers the copy of what it answers here.
fn owner_event(
    vm: &Vm,
    component: ObjRef,
    template: ObjRef,
    order: &[ObjRef],
    controls: &[ObjRef],
    owner: EventOwner,
) -> Result<Option<(ObjRef, ow_object::ids::FuncId)>, VmError> {
    // 1. The owner: `Owner` declared as an object reference on the component (or its template).
    let own = object_field(vm, component, "Owner")?;
    let (owner, instance) = match owner {
        EventOwner::Component => (own, true),
        EventOwner::Given(o) => (o, false),
    };
    let Some(owner) = owner else {
        return Ok(None);
    };
    // 2. Resident.
    unloaded(vm, owner)?;
    // 3. The owner's state: only an owner in no state is looked up.
    let (class, state) = {
        let o = vm.space.heap.get(owner)?;
        (o.class, o.state)
    };
    if state.is_some_and(|s| s != ow_object::ids::NameId::NONE) {
        return Err(event_not_read(IN_STATE));
    }
    // 4. The event, found without loading, compared by path.
    let Some(found) = vm.find_loaded_method(class, EVENT, None) else {
        return Err(event_not_read(EVENT_NOT_LOADED));
    };
    let Some(f) = found else {
        return Ok(None);
    };
    let path = vm.function(f).map(|d| d.path.as_str().to_string()).unwrap_or_default();
    let Some(read_path) = READ_EVENTS.iter().find(|p| p.eq_ignore_ascii_case(&path)) else {
        return Err(event_not_read(event_override(&path)));
    };
    // 5. The call's entry: the same binding and entry checks as `Vm::call`, without the call.
    vm.check_call(f, vec![Value::Object(Some(component))])?;
    if *read_path == ACTOR_EVENT {
        return Ok(Some((owner, f)));
    }
    // 6. Pawn's branch: on its own Mesh it would run ClearAnimNodes and the native CacheAnimNodes.
    let mesh = object_field(vm, owner, "Mesh")?;
    if instance && mesh == Some(component) {
        return Err(event_not_read(OWN_MESH));
    }
    if *read_path == PAWN_EVENT {
        return Ok(Some((owner, f)));
    }
    // 7. WillowPlayerPawn's lookups.
    let arm = object_field(vm, owner, "ArmBoneBlend")?;
    let aim = object_field(vm, owner, "AimOffset")?;
    let left = object_field(vm, owner, "LeftHandIK")?;
    let mut aim_name = None;
    if let Some(body) = object_field(vm, owner, "BodyClass")? {
        unloaded(vm, body)?;
        let class = class_of(vm, body)?;
        let declared = vm.space.prop(class, "AimOffsetNodeName").and_then(|p| vm.space.classes.decl(p)).map(|d| d.kind.clone());
        if !matches!(declared, Some(ow_object::layout::PropKind::Name)) {
            return Err(script(format!("{} declares AimOffsetNodeName as {declared:?}, not a name", class_path(vm, class))));
        }
        aim_name = Some(name_of(vm, body, "AimOffsetNodeName")?);
    }
    use crate::world::tree_lookup::{FIND_ANIM_NODE, FIND_SKEL_CONTROL};
    if !vm.has_native(FIND_ANIM_NODE) {
        return Err(NotYet::not_ported(ANIM_NODE_UNBOUND, EVENT_RECORD).into());
    }
    if !vm.has_native(FIND_SKEL_CONTROL) {
        return Err(NotYet::not_ported(SKEL_CONTROL_UNBOUND, EVENT_RECORD).into());
    }
    // The cast classes, read only: a defined layout, else the cast would define one.
    let casts = CAST_CLASSES
        .iter()
        .map(|path| {
            vm.space
                .classes
                .by_path(path)
                .filter(|&c| vm.space.classes.get_layout(c).is_some_and(|l| l.is_defined()))
                .ok_or_else(|| event_not_read(cast_undefined(path)))
        })
        .collect::<Result<Vec<_>, VmError>>()?;
    // Each find the script makes (a cache only while it is None), planned on the template, and its cast's answer.
    let arm_name = vm.space.names.find("ArmBoneBlend").unwrap_or(ow_object::ids::NameId::NONE);
    let grip_name = vm.space.names.find("LeftHandGrip").unwrap_or(ow_object::ids::NameId::NONE);
    let tree_is_tree = is_a(vm, class_of(vm, template)?, TREE);
    let mut finds: Vec<(Option<ObjRef>, ow_object::ids::ClassId)> = Vec::new();
    if arm.is_none() {
        finds.push((first_named(vm, order, "NodeName", arm_name)?, casts[0]));
    }
    if aim.is_none()
        && let Some(n) = aim_name
    {
        finds.push((first_named(vm, order, "NodeName", n)?, casts[1]));
    }
    if left.is_none() && tree_is_tree && grip_name != ow_object::ids::NameId::NONE {
        finds.push((first_named(vm, controls, "ControlName", grip_name)?, casts[2]));
    }
    for (found, want) in finds {
        if let Some(t) = found {
            vm.is_child_checked(class_of(vm, t)?, want)?;
        }
    }
    // 8. No random number on these paths.
    Ok(Some((owner, f)))
}

/// The name `prop` of `o`, else a `VmError::Script`.
fn name_of(vm: &Vm, o: ObjRef, prop: &str) -> Result<ow_object::ids::NameId, VmError> {
    match read(vm, o, prop)? {
        Value::Name(n) => Ok(n),
        other => Err(script(format!("{}'s {prop} holds {}, not a name", class_path(vm, class_of(vm, o)?), other.kind_name()))),
    }
}

/// The first of `objects` whose `prop` is `name` (a whole name), as the lookups answer.
fn first_named(vm: &Vm, objects: &[ObjRef], prop: &str, name: ow_object::ids::NameId) -> Result<Option<ObjRef>, VmError> {
    for &o in objects {
        if name_of(vm, o, prop)? == name {
            return Ok(Some(o));
        }
    }
    Ok(None)
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
fn copy(vm: &mut Vm, component: ObjRef, plan: &Plan) -> Result<(ObjRef, DetMap<ObjRef, ObjRef>), VmError> {
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
        // The aim-offset fix-up (NATIVE_ANIM_NODE_INIT.md section 1): when the outers differ, the copy's
        // TemplateNode is its source node and its own Profiles are emptied.
        let path = class_path(vm, class_of(vm, c)?);
        if AIM_OFFSET_CLASSES.contains(&path.as_str()) && vm.space.heap.get(c)?.outer != vm.space.heap.get(n)?.outer {
            write(vm, c, "TemplateNode", Value::Object(Some(n)))?;
            write(vm, c, "Profiles", Value::array(Vec::new()))?;
        }
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
    // The controls' copies join the map, so the initialisation's step 5 writes them (lane IMPL-COPY-COMPONENT-PASS).
    for (t, c) in cmap {
        map.insert(t, c);
    }
    Ok((root, map))
}

/// The tick order of tree `root` by the same rule as [`build_tick_list`], read only (the preflight's order). A node of
/// `tree` counts as listed once this walk lists it (its copy's tag is `next_tag` only then: the tag collision check
/// rules out an earlier match); a parent outside `tree` is not copied, so the writes read its own tag, and it counts as
/// listed exactly when that tag is `next_tag` (review r1 of #575, item 1).
pub(crate) fn tick_order(vm: &Vm, root: ObjRef, tree: &DetSet<ObjRef>, next_tag: i32) -> Result<Vec<ObjRef>, VmError> {
    let mut list = vec![root];
    let mut listed = DetSet::new();
    listed.insert(root);
    let mut stack: Vec<std::vec::IntoIter<ObjRef>> = vec![children(vm, root)?.into_iter()];
    while let Some(top) = stack.last_mut() {
        let Some(c) = top.next() else {
            stack.pop();
            continue;
        };
        if listed.contains(&c) {
            continue;
        }
        let mut ready = true;
        for p in elements(&read(vm, c, "ParentNodes")?, "ParentNodes")? {
            let Some(p) = object_of(&p) else { continue };
            let in_list = if tree.contains(&p) { listed.contains(&p) } else { int(&read(vm, p, "NodeTickTag")?) == next_tag };
            if !in_list {
                ready = false;
            }
        }
        if !ready {
            continue;
        }
        listed.insert(c);
        list.push(c);
        stack.push(children(vm, c)?.into_iter());
    }
    Ok(list)
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
fn initialise(
    vm: &mut Vm,
    component: ObjRef,
    root: ObjRef,
    plan: &Plan,
    map: &DetMap<ObjRef, ObjRef>,
) -> Result<(), VmError> {
    let tag = int(&read(vm, component, "TickTag")?).wrapping_add(1);
    write(vm, component, "TickTag", Value::Int(tag))?;
    write(vm, component, "AnimTickArray", Value::array(Vec::new()))?;
    let list = build_tick_list(vm, component, root, tag)?;
    let values: Vec<Value> = list.iter().map(|o| Value::Object(Some(*o))).collect();
    write(vm, root, "AnimTickArray", Value::array(values.clone()))?;
    write(vm, component, "AnimTickArray", Value::array(values))?;
    // The built list is the planned one (the same rule; review r1 of #575, item 1).
    let copy_of = |t: &ObjRef| map.get(t).copied().unwrap_or(*t);
    let planned: Vec<ObjRef> = plan.order.iter().map(copy_of).collect();
    if planned != list {
        return Err(script("the built tick list differs from the planned one".into()));
    }
    // The init tag and the two per-node arrays, before the per-node loop (lane IMPL-COPY-COMPONENT-PASS).
    crate::world::control_arrays::write_init(vm, component, &plan.arrays)?;
    // Step 4, the planned steps replayed in order, iteratively (lane IMPL-ANIM-NODE-INIT).
    use crate::world::anim_node_init::{Step, write_head, write_tail};
    for step in &plan.steps {
        match *step {
            Step::Tick(t) => {
                let n = copy_of(&t);
                write(vm, n, "SkelComponent", Value::Object(Some(component)))?;
                write(vm, n, "NodeTickTag", Value::Int(tag))?;
            }
            Step::Child(t) => write(vm, copy_of(&t), "SkelComponent", Value::Object(Some(component)))?,
            Step::Head(t, b) => write_head(vm, component, copy_of(&t), b.map(|i| &plan.builds[i]))?,
            Step::Tail(t) => write_tail(vm, component, copy_of(&t))?,
        }
    }
    // Step 5 (lane IMPL-COPY-COMPONENT-PASS): the component pass writes nothing for its empty list; the control arrays.
    crate::world::control_arrays::write_arrays(vm, component, &plan.arrays, |t| copy_of(&t))?;
    // Step 6, the owner's event, as planned: an ordinary event call, the owner the receiver, the component the
    // argument (NATIVE_TREE_INIT_OWNER_EVENT.md section 1). Its script writes on the owner only.
    if let Some((owner, f)) = plan.event {
        vm.call(f, Some(owner), vec![Value::Object(Some(component))])?;
    }
    write(vm, component, "bAnimTreeInitialised", Value::Bool(true))
}

/// Copies `template` onto `component`, sets the component's `Animations` and initialises the tree (module
/// documentation); the new tree. The preflight first, then the writes under #518's counters.
pub fn instance(vm: &mut Vm, component: ObjRef, template: ObjRef) -> Result<ObjRef, VmError> {
    instance_with_map(vm, component, template).map(|(root, _)| root)
}

/// [`instance`] with the copy's map from each template object (the root, its nodes and its controls) to its copy, for
/// a caller that planned on the template (the body composition's lookups, lane FIX-BODY-COMPOSITION-LOOKUP-STALE).
pub fn instance_with_map(
    vm: &mut Vm,
    component: ObjRef,
    template: ObjRef,
) -> Result<(ObjRef, DetMap<ObjRef, ObjRef>), VmError> {
    let before = Counters::read(vm);
    let plan = plan(vm, component, template)?;
    let run = |vm: &mut Vm| -> Result<(ObjRef, DetMap<ObjRef, ObjRef>), VmError> {
        let (root, map) = copy(vm, component, &plan)?;
        write(vm, component, "Animations", Value::Object(Some(root)))?;
        if plan.initialise {
            initialise(vm, component, root, &plan, &map)?;
        }
        Ok((root, map))
    };
    run(vm).map_err(|e| after_writes(vm, before, e))
}

#[cfg(test)]
#[path = "anim_tree_copy_tests.rs"]
mod tests;
