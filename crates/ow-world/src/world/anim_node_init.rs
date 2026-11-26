//! Owner: lane IMPL-ANIM-NODE-INIT (decision 2026-11-19-193, number pending). The animation tree's per-node
//! initialisation (the copy's initialisation, section 2 step 4 of NATIVE_COPY_ANIM_TREE.md), in `ParityMode::Off`.
//! AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from `docs/verification/NATIVE_ANIM_NODE_INIT.md` alone (analyst lane
//! ANALYST-AIMOFFSET-REFRESH-AND-NODE-INIT, PR #566, accepted at 07b2dd68) and the hub's rulings in it; this lane did
//! not read the executable. **Every rule is UNVERIFIED in the running game.** Off only; Reference binds nothing.
//!
//! **The class table** ([`kind_of`], the note's section 2 and section 3): each node's **exact** class maps to one of
//! the note's eight implementations, A to H, and `AnimNodeSlot` to H's slot part. A class not in the table is
//! `NotYet { NotRead }` ([`unknown_class`]). The port cannot see C++ inheritance, so a subclass outside the table is
//! refused even if it would inherit an implementation.
//!
//! **The preflight** ([`plan`]) reads only, writes, loads and draws nothing. Over every node in tick order (the
//! children H initialises again are among them), the note's refusals 1 to 7 with its texts:
//! 1. `bCallScriptEventOnInit` set: [`ON_INIT`];
//! 2. D: `AnimSeq` set where the reset runs (see below): [`SEQUENCE_CHANGE`]; `ActiveCameraAnimInstance` set:
//!    [`CAMERA`]. For `WillowAnimNodeSequenceAdditiveBlend`, the override's refusals too: [`ADDITIVE_AUTO_NAME_ISSUING`]
//!    and the additive lookup's ([`ADDITIVE_LINKUP`], [`ADDITIVE_NOT_LOADED`]);
//! 3. C, or H's slot part, whose non-additive child weights sum to at most 1e-5 (H after its step 4):
//!    [`set_active_child`];
//! 4. E: the component's class must declare `bUpdateComposeSkeletonPasses` as a bool (`VmError::Script` otherwise);
//!    the write is the hub's ruling, **fitted, UNVERIFIED**;
//! 5. F: its weight builder's preflight (crate::world::per_bone_weights, NATIVE_PER_BONE_WEIGHTS.md, lane
//!    IMPL-PER-BONE-WEIGHTS): a null mesh skips the build; #569's layer refusals pass through; more than 256 bones is
//!    `NotYet { NotRead }`; a missing or mistyped field is a `VmError::Script`;
//! 6. H with an `AnimParameters` entry past the first naming a `CustomAnimDef` (after its step 7): [`ENTRY_WEIGHTS`];
//! 7. H with an owner: the owner test, read-only, one outcome per case of NATIVE_SPECIALMOVE_OWNER.md section 4.1
//!    ([`owner_implements`], lane IMPL-SPECIALMOVE-OWNER): a yes writes `SMI`, a settled no leaves it as it is, and
//!    each other case is its own refusal. No owner leaves `SMI` as it is.
//!
//! Every declared field an implementation reads or writes is checked there too (a missing one is a `VmError::Script`).
//!
//! **D's corrections** (lane ANIM-NODE-INIT-D-CORRECTIONS, from NATIVE_UPDATEANIMATIONS_TREE_TICKS.md section 3,
//! #747, the hub's ruling; NATIVE_ANIM_NODE_INIT.md section 2 D's correction of 2026-11-26). **UNVERIFIED.**
//! - **The notifies guard:** the "set animation" with None skips its reset only when `bIsIssuingNotifies` is set
//!   **and** `AnimSeqName` is not None ([`resets`]). A node issuing notifies with no name is reset.
//! - **The additive override:** on `WillowAnimNodeSequenceAdditiveBlend`, after the base step, whatever the guard: with
//!   `bAutoGenerateAdditiveSeqName` set, `AddAnimSeqName` := `Add_None` (the text "Add_" and the asked name None, a
//!   name intern, made only at the write; the preflight matches by text). Then the re-set: `AddAnimSeqName` := that
//!   name (or the node's own), `AddAnimSeq` := None, `AddAnimLinkupIndex` := -1. When the name is not None and the
//!   component has a `SkeletalMesh` and a non-empty `AnimSets`, the component's sequence lookup ([`additive_lookup`]),
//!   read only: a match is [`ADDITIVE_LINKUP`] (the link-up is not read). With the auto-name flag set while the base
//!   step is skipped (issuing under a name), [`ADDITIVE_AUTO_NAME_ISSUING`], as the tree-tick note refuses it.
//!
//! **The writes** ([`write_head`], [`write_tail`]) replay the planned [`Step`]s, iteratively: per node in tick order,
//! the copy's two writes (`SkelComponent`, `NodeTickTag`), then the note's section 2 steps for that node's
//! implementation; a slot's children past the first are initialised again between its head and its tail. The preflight
//! plans the same steps with the same tick rule, and checks every node they initialise (review r1 of #575).

use ow_core::NotYet;
use ow_object::ids::{ClassId, ObjRef};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::anim_tree_copy::{class_of, class_path, elements, object_of, read, truth, write};
use super::body_composition::{field, set_field};
use super::per_bone_weights::Build;

/// The analyst note.
pub const RECORD: &str = "docs/verification/NATIVE_ANIM_NODE_INIT.md";
const UNIT: &str = "the per-node initialisation";

/// The note's refusal texts (its section 3).
pub const ON_INIT: &str = "the node's OnInit script event (the event's name was not read)";
pub const SEQUENCE_CHANGE: &str = "AnimNodeSequence: the steps after its sequence changes";
pub const CAMERA: &str = "AnimNodeSequence: stopping its camera animation";
/// The additive override's refusals (D's corrections, NATIVE_UPDATEANIMATIONS_TREE_TICKS.md section 5).
pub const ADDITIVE_LINKUP: &str = "WillowAnimNodeSequenceAdditiveBlend: an additive sequence's mesh link-up";
pub const ADDITIVE_AUTO_NAME_ISSUING: &str =
    "WillowAnimNodeSequenceAdditiveBlend: the additive auto-name while issuing notifies";
pub const ADDITIVE_NOT_LOADED: &str =
    "WillowAnimNodeSequenceAdditiveBlend: an anim set or sequence not loaded at initialisation (a load is a write)";
/// The additive class, and the name its auto-name builds from the asked name None.
const ADDITIVE: &str = "WillowGame.WillowAnimNodeSequenceAdditiveBlend";
const AUTO_NAME: &str = "Add_None";
pub const ENTRY_WEIGHTS: &str = "AnimNodeSpecialMoveBlend: the per-bone weight build of an entry";
/// The interface H's owner test names.
pub const SPECIAL_MOVE_INTERFACE: &str = "GearboxFramework.SpecialMoveInterface";

/// `"the per-node initialisation of <class>"`.
pub fn unknown_class(class: &str) -> String {
    format!("the per-node initialisation of {class}")
}

/// `"<class>: set active child during initialisation"`.
pub fn set_active_child(class: &str) -> String {
    format!("{class}: set active child during initialisation")
}


/// The note's eight implementations, and H's slot part on its own (`AnimNodeSlot`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    Slot,
    H,
}

/// The exact-class table (the note's section 2 table and section 3).
pub fn kind_of(path: &str) -> Option<Kind> {
    Some(match path {
        "Engine.AnimNode" => Kind::A,
        "Engine.AnimNodeBlendBase"
        | "Engine.AnimNodeScaleRateBySpeed"
        | "Engine.AnimNodeBlendDirectional"
        | "Engine.AnimNodeAimOffset"
        | "WillowGame.WillowAnimNodeAimOffset"
        | "WillowGame.WillowAnimNode_Condition"
        | "WillowGame.WillowAnimNode_ClimbLadder" => Kind::B,
        "Engine.AnimNodeBlendList"
        | "Engine.AnimNodeBlendBySpeed"
        | "Engine.AnimNodeBlendByPhysics"
        | "Engine.AnimNodeBlendByPosture"
        | "WillowGame.WillowAnimNodeBlendInjured"
        | "WillowGame.WillowAnimNodeBlendThirdPersonMenu" => Kind::C,
        "Engine.AnimNodeSequence" | "WillowGame.WillowAnimNodeSequenceAdditiveBlend" => Kind::D,
        "Engine.AnimTree" | "WillowGame.WillowAnimTree" => Kind::E,
        "Engine.AnimNodeBlendPerBone" | "WillowGame.WillowAnimNodeFeatherBlend" => Kind::F,
        "Engine.AnimNodeAdditiveBlending" => Kind::G,
        "Engine.AnimNodeSlot" => Kind::Slot,
        "GearboxFramework.AnimNodeSpecialMoveBlend" => Kind::H,
        _ => return None,
    })
}

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message }
}

fn not_read(what: impl Into<String>) -> VmError {
    NotYet::not_read(what.into(), RECORD).into()
}

fn float(v: &Value) -> f32 {
    match v {
        Value::Float(f) => *f,
        _ => 0.0,
    }
}

/// `node`'s implementation, by its exact class.
fn kind(vm: &Vm, node: ObjRef) -> Result<(Kind, String), VmError> {
    let path = class_path(vm, class_of(vm, node)?);
    kind_of(&path).map(|k| (k, path.clone())).ok_or_else(|| not_read(unknown_class(&path)))
}

/// Whether the implementation runs B (every one but A and D).
fn runs_b(k: Kind) -> bool {
    !matches!(k, Kind::A | Kind::D)
}

/// The child slots of `node` (its `Children`).
fn slots(vm: &Vm, node: ObjRef) -> Result<Vec<Value>, VmError> {
    elements(&read(vm, node, "Children")?, "Children")
}

/// The sum of the child weights of the slots not marked `bIsAdditive`, over `slots`.
fn non_additive_sum(vm: &Vm, slots: &[Value]) -> Result<f32, VmError> {
    let mut sum = 0.0;
    for s in slots {
        if !truth(&field(vm, s, "bIsAdditive", "AnimBlendChild")?) {
            sum += float(&field(vm, s, "Weight", "AnimBlendChild")?);
        }
    }
    Ok(sum)
}

/// H's step 4: `1 − clamp(sum of the other slots' non-additive weights, 0, 1)`.
fn first_weight(vm: &Vm, slots: &[Value]) -> Result<f32, VmError> {
    let others = non_additive_sum(vm, slots.get(1..).unwrap_or(&[]))?;
    Ok(1.0 - others.clamp(0.0, 1.0))
}

/// The record of the owner test's cases (lane IMPL-SPECIALMOVE-OWNER).
pub const OWNER_RECORD: &str = "docs/verification/NATIVE_SPECIALMOVE_OWNER.md";

/// O1's text.
pub const OWNER_INTERFACE_NOT_LOADED: &str = "AnimNodeSpecialMoveBlend: the interface class \
    GearboxFramework.SpecialMoveInterface is not loaded (a load is a write)";
/// O3''s text.
pub const OWNER_INTERFACE_NO_KEY: &str = "AnimNodeSpecialMoveBlend: the interface \
    GearboxFramework.SpecialMoveInterface has no code key (its table cannot be read)";

/// O4's text.
pub fn owner_not_decoded(class: &str) -> String {
    format!(
        "AnimNodeSpecialMoveBlend: the class chain of {class} did not decode (the structural fallback is not \
         taken)"
    )
}

/// O5's text.
pub fn owner_not_resolved(class: &str) -> String {
    format!(
        "AnimNodeSpecialMoveBlend: the interface table of {class} has an entry not resolved at set-up (resolving it is \
         a load)"
    )
}

fn owner_not_read(what: String) -> VmError {
    NotYet::not_read(what, OWNER_RECORD).into()
}

/// The failed-target refusal's text (review r1 of #667, item 3; a port policy, UNVERIFIED).
pub fn owner_target_missing(class: &str) -> String {
    format!(
        "AnimNodeSpecialMoveBlend: the interface table of {class} has an entry whose class lookup found nothing (its \
         target was never read, so it is not taken as a no)"
    )
}

/// Rule 7's owner test, read-only (NATIVE_SPECIALMOVE_OWNER.md section 4.1; lane IMPL-SPECIALMOVE-OWNER): whether the
/// owner's class implements [`SPECIAL_MOVE_INTERFACE`], one outcome per case, in the note's order. Nothing is
/// written, loaded, drawn or interned, and no interface entry is resolved.
/// - **O1**, the interface class not loaded: `NotRead` ([`OWNER_INTERFACE_NOT_LOADED`]);
/// - **O2**, no code layer: `LayerAbsent`, the text `Vm::plain_interface_read_only` gives, before any ancestry is
///   consulted;
/// - **O3**, the owner's class has no code key: a class derived from the interface in the class table is a yes;
///   otherwise it implements nothing, a settled no, unless the chain meets an engine-native stand-in
///   (`Vm::is_child_checked`'s `NotYet`; its boolean is not taken as an interface answer);
/// - **O3'**, only the interface has no code key: `NotRead` ([`OWNER_INTERFACE_NO_KEY`]);
/// - **O4**, a body on the owner's chain or the interface's not decoded, tested directly: `NotRead`
///   ([`owner_not_decoded`]); the structural fallback is not taken;
/// - then `implements_resolved_chain` alone: **O5**, an unresolved entry and no resolved yes, `NotRead`
///   ([`owner_not_resolved`], naming the class it names); **O6**, a resolved yes; **O7**, a resolved no. Before O7's
///   no: an entry whose lookup found no class (cached as "resolved to nothing") is `NotRead`
///   ([`owner_target_missing`], review r1 of #667, a port policy), then the stand-in check as in O3.
///
/// `Ok(true)` is a yes (O3's ancestry, O6); `Ok(false)` a settled no (O3, O7). Every outcome is UNVERIFIED in the
/// running game.
pub fn owner_implements(vm: &Vm, owner: ObjRef) -> Result<bool, VmError> {
    #[cfg(test)]
    if REFUSE_OWNER.with(std::cell::Cell::get) == Some(owner) {
        return Err(owner_not_read(REFUSED_FOR_TESTS.to_string()));
    }
    #[cfg(test)]
    if let Some(answer) = OWNER_ANSWER.with(std::cell::Cell::get) {
        return Ok(answer);
    }
    let class = class_of(vm, owner)?;
    let Some(iface) = vm.space.classes.by_path(SPECIAL_MOVE_INTERFACE) else {
        return Err(owner_not_read(OWNER_INTERFACE_NOT_LOADED.to_string()));
    };
    let code = vm.defs.code();
    if code.classes().next().is_none() {
        // O2, with the plain read-only answer's own text and record.
        return Err(NotYet::layer_absent(
            format!(
                "the interface table of {}: the world has no code layer (no class export is loaded)",
                class_path(vm, class)
            ),
            "the code layer (package class exports)",
            ow_vm::api::DISPATCH_RECORD,
        )
        .into());
    }
    let Some(c) = code.key_of(class) else {
        // O3: a hand-defined class implements nothing, unless it derives from the interface in the class table.
        if vm.space.classes.is_child_of(class, iface) {
            return Ok(true);
        }
        vm.is_child_checked(class, iface)?;
        return Ok(false);
    };
    let Some(i) = code.key_of(iface) else {
        return Err(owner_not_read(OWNER_INTERFACE_NO_KEY.to_string()));
    };
    let decoded = code.class(i).body_decoded() && code.super_chain(c).all(|k| code.class(k).body_decoded());
    if !decoded {
        return Err(owner_not_read(owner_not_decoded(&class_path(vm, class))));
    }
    match code.implements_resolved_chain(c, i) {
        Ok(true) => Ok(true),
        Ok(false) => {
            if let Some(at) = code.interface_target_missing(c) {
                return Err(owner_not_read(owner_target_missing(code.class(at).path.as_str())));
            }
            // O7: the stand-in check only (its boolean is not the interface answer).
            vm.is_child_checked(class, iface)?;
            Ok(false)
        }
        Err(at) => Err(owner_not_read(owner_not_resolved(code.class(at).path.as_str()))),
    }
}

#[cfg(test)]
thread_local! {
    static OWNER_ANSWER: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    static REFUSE_OWNER: std::cell::Cell<Option<ObjRef>> = const { std::cell::Cell::new(None) };
}

/// The test-only refusal [`refuse_owner_for_tests`] gives.
#[cfg(test)]
pub(crate) const REFUSED_FOR_TESTS: &str = "AnimNodeSpecialMoveBlend: the test's refusing owner";

/// For the tests only: [`owner_implements`] refuses (`NotRead`, [`REFUSED_FOR_TESTS`]) when asked about `owner`, ahead
/// of [`answer_owner_for_tests`]; so a test can tell which owner was asked (review r2 of #667).
#[cfg(test)]
pub(crate) fn refuse_owner_for_tests(owner: Option<ObjRef>) {
    REFUSE_OWNER.with(|c| c.set(owner));
}

/// For the tests only: answer [`owner_implements`] with `answer` on this thread (a yes or a settled no), for the
/// hand-built fixtures that test H's other parts; `None` runs the real test (its cases have their own tests).
#[cfg(test)]
pub(crate) fn answer_owner_for_tests(answer: Option<bool>) {
    OWNER_ANSWER.with(|c| c.set(answer));
}

/// The component's class declares `name` as a bool (refusal 4's check).
/// Whether D's "set animation" with None runs its reset on `node`: unless `bIsIssuingNotifies` is set and
/// `AnimSeqName` is not None (D's corrections).
fn resets(vm: &Vm, node: ObjRef) -> Result<bool, VmError> {
    let issuing = truth(&read(vm, node, "bIsIssuingNotifies")?);
    let named = match read(vm, node, "AnimSeqName")? {
        Value::Name(n) => n != ow_object::ids::NameId::NONE,
        other => return Err(script(format!("AnimSeqName holds {}, not a name", other.kind_name()))),
    };
    Ok(!(issuing && named))
}

/// Whether `o` is resident: neither a handle the store has not loaded (or failed) nor a slotless stand-in.
fn resident(vm: &Vm, o: ObjRef) -> Result<bool, VmError> {
    let x = vm.space.heap.get(o)?;
    let slotless = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    Ok(!(vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || slotless))
}

/// The text of the name the additive override looks up on `node`, or `None` for no lookup (the name None).
fn additive_name(vm: &Vm, node: ObjRef) -> Result<Option<String>, VmError> {
    if truth(&read(vm, node, "bAutoGenerateAdditiveSeqName")?) {
        return Ok(Some(AUTO_NAME.to_string()));
    }
    match read(vm, node, "AddAnimSeqName")? {
        Value::Name(n) if n == ow_object::ids::NameId::NONE => Ok(None),
        Value::Name(n) => Ok(Some(vm.space.names.text(n).into_owned())),
        other => Err(script(format!("AddAnimSeqName holds {}, not a name", other.kind_name()))),
    }
}

/// [`ADDITIVE_NOT_LOADED`] naming the object (its store path, nothing loaded).
pub fn not_loaded(vm: &Vm, o: ObjRef) -> String {
    format!("{ADDITIVE_NOT_LOADED}: {}", super::pawn_meshes::handle_path(vm, o))
}

/// The component's sequence lookup for the additive override (D's corrections), read only: with a `SkeletalMesh` and
/// a non-empty `AnimSets`, each set in reverse order (in order with the node's `bReverseAnimSeqSearchOrder`), None
/// entries skipped, each set and each of its `Sequences` resident, matched by `SequenceName` text case-insensitively.
/// A match is [`ADDITIVE_LINKUP`]; no match answers nothing. The set's native name map is replaced by this walk
/// (UNVERIFIED, as the tree-tick note labels it).
fn additive_lookup(vm: &Vm, component: ObjRef, node: ObjRef, target: &str) -> Result<(), VmError> {
    if object_of(&read(vm, component, "SkeletalMesh")?).is_none() {
        return Ok(());
    }
    let mut sets = elements(&read(vm, component, "AnimSets")?, "AnimSets")?;
    if sets.is_empty() {
        return Ok(());
    }
    if !truth(&read(vm, node, "bReverseAnimSeqSearchOrder")?) {
        sets.reverse();
    }
    for s in sets {
        let Some(set) = object_of(&s) else { continue };
        if !resident(vm, set)? {
            return Err(not_read(not_loaded(vm, set)));
        }
        for q in elements(&read(vm, set, "Sequences")?, "Sequences")? {
            let Some(seq) = object_of(&q) else { continue };
            if !resident(vm, seq)? {
                return Err(not_read(not_loaded(vm, seq)));
            }
            match read(vm, seq, "SequenceName")? {
                Value::Name(n) if vm.space.names.text(n).eq_ignore_ascii_case(target) => {
                    return Err(not_read(ADDITIVE_LINKUP));
                }
                Value::Name(_) => {}
                other => return Err(script(format!("SequenceName holds {}, not a name", other.kind_name()))),
            }
        }
    }
    Ok(())
}

fn declares_bool(vm: &Vm, class: ClassId, name: &str) -> bool {
    vm.space.prop(class, name).and_then(|p| vm.space.classes.decl(p)).is_some_and(|d| matches!(d.kind, PropKind::Bool))
}

/// The preflight's check of one node (module documentation), reading `node` (a template node: the copy holds the same
/// values) and `component`.
fn check_node(vm: &Vm, component: ObjRef, node: ObjRef, owner: Option<ObjRef>) -> Result<(), VmError> {
    let (k, path) = kind(vm, node)?;
    // A, the base every implementation runs first.
    for name in ["bRelevant", "bJustBecameRelevant", "NodeTotalWeight", "NodeTickTag", "bCallScriptEventOnInit"] {
        read(vm, node, name)?;
    }
    read(vm, component, "TickTag")?;
    if truth(&read(vm, node, "bCallScriptEventOnInit")?) {
        return Err(not_read(ON_INIT));
    }
    let children = if runs_b(k) { slots(vm, node)? } else { Vec::new() };
    if runs_b(k) {
        for s in &children {
            for f in ["AdditiveWeightScale", "Weight", "bIsAdditive"] {
                field(vm, s, f, "AnimBlendChild")?;
            }
        }
    }
    match k {
        Kind::A | Kind::B => {}
        Kind::C => {
            read(vm, node, "TargetWeight")?;
            read(vm, node, "ActiveChildIndex")?;
            if non_additive_sum(vm, &children)? <= 1e-5 {
                return Err(not_read(set_active_child(&path)));
            }
        }
        Kind::D => {
            for name in ["AnimSeqName", "AnimLinkupIndex", "CachedNumDesiredBones"] {
                read(vm, node, name)?;
            }
            let seq = object_of(&read(vm, node, "AnimSeq")?);
            let resets = resets(vm, node)?;
            if seq.is_some() && resets {
                return Err(not_read(SEQUENCE_CHANGE));
            }
            if path == ADDITIVE {
                for name in ["AddAnimSeq", "AddAnimLinkupIndex"] {
                    read(vm, node, name)?;
                }
                if truth(&read(vm, node, "bAutoGenerateAdditiveSeqName")?) && !resets {
                    return Err(not_read(ADDITIVE_AUTO_NAME_ISSUING));
                }
                if let Some(target) = additive_name(vm, node)? {
                    additive_lookup(vm, component, node, &target)?;
                }
            }
            if object_of(&read(vm, node, "ActiveCameraAnimInstance")?).is_some() {
                return Err(not_read(CAMERA));
            }
        }
        Kind::E => {
            let class = class_of(vm, component)?;
            if !declares_bool(vm, class, "bUpdateComposeSkeletonPasses") {
                return Err(script(format!(
                    "{} does not declare bUpdateComposeSkeletonPasses as a bool",
                    class_path(vm, class)
                )));
            }
        }
        // F's weight builder (NATIVE_PER_BONE_WEIGHTS.md, lane IMPL-PER-BONE-WEIGHTS): planned, refusals typed.
        // F's builder is planned with each of its heads (see `plan`).
        Kind::F => {}
        Kind::G => {}
        Kind::Slot | Kind::H => {
            read(vm, node, "TargetWeight")?;
            read(vm, node, "SynchNode")?;
            // After step 4 the first slot's weight is fixed; then step 5's test.
            let mut after = children.clone();
            if let Some(first) = after.first_mut() {
                let w = first_weight(vm, &children)?;
                set_field(vm, first, "Weight", Value::Float(w), "AnimBlendChild")?;
            }
            if non_additive_sum(vm, &after)? <= 1e-5 {
                return Err(not_read(set_active_child(&path)));
            }
            if k == Kind::H {
                // The element struct must be defined, its defaults readable through the checked API (review r1 of
                // #575, item 4), before any write uses it.
                parameters_element(vm, node)?;
                let params = elements(&read(vm, node, "AnimParameters")?, "AnimParameters")?;
                read(vm, node, "SMI")?;
                if params.len() == children.len() {
                    for p in params.iter().skip(1) {
                        if object_of(&field(vm, p, "CustomAnimDef", "SlotAnimParameters")?).is_some() {
                            return Err(not_read(ENTRY_WEIGHTS));
                        }
                    }
                }
                // The declared field is checked on the component; the test runs on the owner the instance will have
                // (review r1 of #667, item 1: an attach passes the actor, not the template's own `Owner`).
                read(vm, component, "Owner")?;
                if let Some(owner) = owner {
                    // The owner test's refusals (O1, O2, O3', O4, O5, a stand-in); a settled no leaves `SMI`.
                    owner_implements(vm, owner)?;
                }
            }
        }
    }
    Ok(())
}

/// The zero `AnimParameters` entry of H node `node`: its declared element must be a defined struct (the checked
/// `struct_layout`, review r1 of #575, item 4), and the entry is that layout's zero value (review r2, item 1).
fn parameters_element(vm: &Vm, node: ObjRef) -> Result<Value, VmError> {
    let class = class_of(vm, node)?;
    let element = vm
        .space
        .prop(class, "AnimParameters")
        .and_then(|p| vm.space.classes.decl(p))
        .and_then(|d| match &d.kind {
            PropKind::Array(inner) => match **inner {
                PropKind::Struct(s) => Some(s),
                _ => None,
            },
            _ => None,
        })
        .ok_or_else(|| script(format!("{}.AnimParameters is not an array of structs", class_path(vm, class))))?;
    // H's step 7 makes zeroed entries: the struct's zero value, not its defaults, which may hold nonzero values or a
    // CustomAnimDef the preflight's step 6 check would then have to see (review r2 of #575, item 1).
    vm.space
        .classes
        .struct_layout(element)
        .map(|l| l.zero().clone())
        .ok_or_else(|| script(format!("{}.AnimParameters: its element struct is not defined", class_path(vm, class))))
}

/// One call of the initialisation, on a **template** node (the writes map it to its copy): the steps the writes run,
/// in order (review r1 of #575, items 1 and 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// The copy's two writes on a node of the tick list (`SkelComponent`, `NodeTickTag`).
    Tick(ObjRef),
    /// H's step 2 on a child past the first: its `SkelComponent`.
    Child(ObjRef),
    /// A node's initialisation up to a slot's children (A, B, and the whole of every other implementation). For an F
    /// node, the index of its build in the plan's builds, planned in the preflight for this head on its own (review r1
    /// of #585, item 2: the writes replay it, never rerun the builder).
    Head(ObjRef, Option<usize>),
    /// A slot's steps after its children (H's steps 3 to 9).
    Tail(ObjRef),
}

/// The most steps one initialisation may plan (a guard against a tree whose slots re-initialise without end).
const MAX_STEPS: usize = 1 << 16;

/// The steps of `node`'s initialisation (after its tick-list writes, or after H's child step), iteratively: a slot's
/// children past the first are initialised again, in slot order, between its head and its tail. A child already on
/// the active slot call path is a loop the writes would never leave: a `VmError::Script` (review r1 of #575, item 3).
fn expand(vm: &Vm, node: ObjRef, out: &mut Vec<Step>) -> Result<(), VmError> {
    enum Frame {
        Enter(ObjRef),
        Child(ObjRef),
        Leave(ObjRef),
    }
    let mut stack = vec![Frame::Enter(node)];
    let mut path: Vec<ObjRef> = Vec::new();
    while let Some(frame) = stack.pop() {
        if out.len() > MAX_STEPS {
            return Err(script(format!("the initialisation plans more than {MAX_STEPS} steps")));
        }
        match frame {
            Frame::Child(c) => {
                out.push(Step::Child(c));
                stack.push(Frame::Enter(c));
            }
            Frame::Enter(n) => {
                out.push(Step::Head(n, None));
                let (k, path_text) = kind(vm, n)?;
                if matches!(k, Kind::Slot | Kind::H) {
                    if path.contains(&n) {
                        return Err(script(format!(
                            "{path_text}: a slot's children lead back to it (its initialisation would not end)"
                        )));
                    }
                    path.push(n);
                    stack.push(Frame::Leave(n));
                    let again: Vec<ObjRef> = slots(vm, n)?
                        .iter()
                        .skip(1)
                        .map(|s| field(vm, s, "Anim", "AnimBlendChild").map(|v| object_of(&v)))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .flatten()
                        .collect();
                    for c in again.into_iter().rev() {
                        stack.push(Frame::Child(c));
                    }
                }
            }
            Frame::Leave(n) => {
                path.pop();
                out.push(Step::Tail(n));
            }
        }
    }
    Ok(())
}

/// The preflight (module documentation): the steps for `order` (every node of the tick list, in order) and their
/// checks. Every node a step initialises is checked, H's re-initialised children included, whether or not it is on the
/// tick list (review r1 of #575, item 1). `owner` is the owner the instance's component will have, which H's owner test
/// asks (rule 7): the component's own `Owner` for a copy, the actor attached to for an attach (review r1 of #667).
pub fn plan(vm: &Vm, component: ObjRef, order: &[ObjRef], owner: Option<ObjRef>) -> Result<(Vec<Step>, Vec<Build>), VmError> {
    let mut steps = Vec::new();
    for &n in order {
        steps.push(Step::Tick(n));
        expand(vm, n, &mut steps)?;
    }
    let mut checked = ow_core::DetSet::new();
    let mut builds = Vec::new();
    for step in &mut steps {
        if let Step::Head(n, slot) = step {
            if checked.insert(*n) {
                check_node(vm, component, *n, owner)?;
            }
            // Every F head's build, planned on its own (the note's section 6, "Where it runs").
            if kind(vm, *n)?.0 == Kind::F {
                builds.push(super::per_bone_weights::plan(vm, component, *n)?);
                *slot = Some(builds.len() - 1);
            }
        }
    }
    Ok((steps, builds))
}

/// Writes `v` into the integer property `name` of `o` by its declared kind (an int, or a byte).
fn write_int(vm: &mut Vm, o: ObjRef, name: &str, v: i32) -> Result<(), VmError> {
    let class = class_of(vm, o)?;
    let kind = vm.space.prop(class, name).and_then(|p| vm.space.classes.decl(p)).map(|d| d.kind.clone());
    let value = match kind {
        Some(PropKind::Byte { .. }) => Value::Byte(u8::try_from(v).unwrap_or(0)),
        _ => Value::Int(v),
    };
    write(vm, o, name, value)
}

/// A `TargetWeight` with one entry per child, the first 1.0 (C's and H's step), unless it already matches.
fn fix_target_weight(vm: &mut Vm, node: ObjRef, children: usize) -> Result<(), VmError> {
    let current = elements(&read(vm, node, "TargetWeight")?, "TargetWeight")?;
    if current.len() == children {
        return Ok(());
    }
    let mut fresh = vec![Value::Float(0.0); children];
    if let Some(first) = fresh.first_mut() {
        *first = Value::Float(1.0);
    }
    write(vm, node, "TargetWeight", Value::array(fresh))
}

/// The head of `node`'s initialisation for `component` ([`Step::Head`]): A, B, and every implementation's steps but a
/// slot's (whose steps after its children are [`write_tail`]).
pub fn write_head(vm: &mut Vm, component: ObjRef, node: ObjRef, build: Option<&Build>) -> Result<(), VmError> {
    let (k, _) = kind(vm, node)?;
    // A.
    write(vm, node, "bRelevant", Value::Bool(false))?;
    write(vm, node, "bJustBecameRelevant", Value::Bool(false))?;
    let tag = match read(vm, component, "TickTag")? {
        Value::Int(i) => i,
        _ => 0,
    };
    write(vm, node, "NodeTickTag", Value::Int(tag))?;
    write(vm, node, "NodeTotalWeight", Value::Float(0.0))?;
    // B.
    if runs_b(k) {
        let mut list = slots(vm, node)?;
        let mut changed = false;
        for s in &mut list {
            if float(&field(vm, s, "AdditiveWeightScale", "AnimBlendChild")?) == 0.0 {
                set_field(vm, s, "AdditiveWeightScale", Value::Float(1.0), "AnimBlendChild")?;
                changed = true;
            }
        }
        if changed {
            write(vm, node, "Children", Value::array(list))?;
        }
    }
    match k {
        Kind::A | Kind::B => {}
        Kind::C => {
            let n = slots(vm, node)?.len();
            fix_target_weight(vm, node, n)?;
        }
        Kind::D => {
            let kept = read(vm, node, "AnimSeqName")?;
            if resets(vm, node)? {
                write(vm, node, "AnimSeqName", Value::Name(ow_object::ids::NameId::NONE))?;
                write(vm, node, "AnimSeq", Value::Object(None))?;
                write_int(vm, node, "AnimLinkupIndex", -1)?;
            }
            // The additive override (D's corrections): the auto-name (interned only here), then the re-set. The
            // preflight refused a lookup match, so `AddAnimSeq` is None.
            if class_path(vm, class_of(vm, node)?) == ADDITIVE {
                let name = if truth(&read(vm, node, "bAutoGenerateAdditiveSeqName")?) {
                    let n = Value::Name(vm.space.names.intern(AUTO_NAME));
                    write(vm, node, "AddAnimSeqName", n.clone())?;
                    n
                } else {
                    read(vm, node, "AddAnimSeqName")?
                };
                write(vm, node, "AddAnimSeqName", name)?;
                write(vm, node, "AddAnimSeq", Value::Object(None))?;
                write_int(vm, node, "AddAnimLinkupIndex", -1)?;
            }
            write_int(vm, node, "CachedNumDesiredBones", 0)?;
            write(vm, node, "AnimSeqName", kept)?;
        }
        Kind::E => write(vm, component, "bUpdateComposeSkeletonPasses", Value::Bool(true))?,
        Kind::F | Kind::G => {
            // F: the weight builder's two writes, after B and before the first child's weight (lane
            // IMPL-PER-BONE-WEIGHTS); a null mesh builds nothing. F and G: the first child's weight 1.0.
            if k == Kind::F {
                match build {
                    Some(Build::Write(w, l)) => {
                        write(vm, node, "Child2PerBoneWeight", Value::array(w.iter().copied().map(Value::Float).collect()))?;
                        write(vm, node, "LocalToCompReqBones", Value::array(l.iter().copied().map(Value::Byte).collect()))?;
                    }
                    Some(Build::Skip) => {}
                    None => return Err(script("an F head with no planned build".into())),
                }
            }
            let mut list = slots(vm, node)?;
            if let Some(first) = list.first_mut()
                && float(&field(vm, first, "Weight", "AnimBlendChild")?) != 1.0
            {
                set_field(vm, first, "Weight", Value::Float(1.0), "AnimBlendChild")?;
                write(vm, node, "Children", Value::array(list))?;
            }
        }
        // A slot's step 2 (its children past the first, initialised again) is the planned steps between its head and
        // its tail.
        Kind::Slot | Kind::H => {}
    }
    Ok(())
}

/// The tail of slot node `node`'s initialisation for `component` ([`Step::Tail`]): H's steps 3 to 9, after its
/// children's.
pub fn write_tail(vm: &mut Vm, component: ObjRef, node: ObjRef) -> Result<(), VmError> {
    let (k, _) = kind(vm, node)?;
    match k {
        Kind::Slot | Kind::H => {
            let n = slots(vm, node)?.len();
            fix_target_weight(vm, node, n)?;
            let mut list = slots(vm, node)?;
            let w = first_weight(vm, &list)?;
            if let Some(first) = list.first_mut() {
                set_field(vm, first, "Weight", Value::Float(w), "AnimBlendChild")?;
                write(vm, node, "Children", Value::array(list))?;
            }
            write(vm, node, "SynchNode", Value::Object(None))?;
            if k == Kind::H {
                let params = elements(&read(vm, node, "AnimParameters")?, "AnimParameters")?;
                if params.len() != n {
                    let zero = parameters_element(vm, node)?;
                    write(vm, node, "AnimParameters", Value::array(vec![zero; n]))?;
                }
                if let Some(owner) = object_of(&read(vm, component, "Owner")?)
                    && owner_implements(vm, owner)?
                {
                    // The preflight answered the test (rule 7); a settled no leaves `SMI` as it is.
                    let class = class_of(vm, node)?;
                    let interface = vm
                        .space
                        .prop(class, "SMI")
                        .and_then(|p| vm.space.classes.decl(p))
                        .is_some_and(|d| matches!(d.kind, PropKind::Interface { .. }));
                    let v = if interface { Value::Interface(Some(owner)) } else { Value::Object(Some(owner)) };
                    write(vm, node, "SMI", v)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

// Tested with the copy's fixture (anim_tree_copy_tests.rs, "The per-node initialisation").
