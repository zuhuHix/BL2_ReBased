//! Owner: lane IMPL-MAPLOAD-NAVLIST (decision 2026-11-19-NNN, number pending). A level's navigation, cover and pylon
//! lists joined onto the world's, in `ParityMode::Off`, coded from docs/verification/NATIVE_MAP_LOAD.md sections 5 and 6
//! alone (#536 at 43a75d9c; clean room: lane ANALYST-MAPLOAD-NAVLIST-PLAYACTOR read the executable, this lane did not).
//! AI-assisted (Claude Code). **Every rule here is UNVERIFIED in the running game.**
//!
//! **Nothing calls it yet.** The persistent level's join belongs in the bring-up's begin-play-once step, after the game
//! info's spawn and before the actor initialisation pass (SLICE_MAPLOAD.md section 3, step 6); the bring-up's lane
//! (lane 2, then lane 4 for the drivers) calls [`join_level`] there. This lane does not edit the bring-up.
//!
//! **The level's pairs** ([`level_list_pairs`]): each level's untagged tail holds the start and end of its navigation,
//! cover and pylon lists, but where they sit after the URL is not decoded (note 5.1), so reading them is always
//! `NotYet { NotRead }` ([`PAIRS`]). No offset is guessed (AGENTS.md rule 4). A caller (or a test) that has the pairs
//! another way hands them to [`join_level_lists`].
//!
//! **The join** ([`join_level_lists`], note 5.3), on the world's one `WorldInfo`:
//! 1. **A write-free preflight of all three kinds** before any splice. Per kind:
//!    - a null pair (both ends None) is a no-op for that kind ([`KindOutcome::Empty`]);
//!    - a half-null pair is refused ([`HALF`]). The note reads the game's join as skipping such a kind; the port
//!      refuses it as damaged data (the note's proposed rule);
//!    - the world's list of the kind is walked from its head; one that loops, or holds an entry not of the kind's
//!      class, is refused ([`WORLD_LOOP`], [`NOT_OF_KIND`]);
//!    - a level whose start is already in the world's list of that kind was joined before: a no-op for that kind
//!      ([`KindOutcome::AlreadyJoined`]; a port answer, the game guards it with a per-level flag on the streaming
//!      path). A level some of whose set kinds are joined and others not is refused ([`PARTLY_JOINED`]);
//!    - otherwise the level's chain is walked from its start along the kind's link. It must reach the end and find None
//!      after it ([`MISSES_END`], [`PAST_END`]), never revisit an entry ([`LOOP`]), never pass through an entry of the
//!      world's list of the same kind ([`SHARED`]), and hold only entries of the kind's class ([`NOT_OF_KIND`]).
//!
//!    The membership tests are per kind: one actor may be in two kinds (a pylon is in the navigation list and the
//!    pylon list).
//! 2. **The splices**, only once every kind has passed, in the order navigation, cover, pylon: the level's end's link
//!    becomes the world's head, then the world's head becomes the level's start ([`ListKind`] names the fields).
//!
//! Every refusal is `NotYet { NotRead }` with the texts below, raised before anything is written, so none is ever
//! marked "stopped after partial writes: ". Malformed shapes (a field a class does not declare, a field that does not
//! hold an object reference, no `WorldInfo`) are `VmError::Script`. Under Reference the join is
//! `NotYet { NotPorted }`: the C++ reference has no join.
//!
//! **Not done (note 5.2 and 5.3, items 6 and 7):** the per-point engine call the game's join makes first (not read; it
//! is left out, and a later stop that needs it raises it), the streamed levels' joins and their order relative to the
//! persistent level's (open), and taking a level's lists out again when it is removed.

use ow_core::{DetSet, NotYet, ParityMode};
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

/// The record the rules come from.
pub const RECORD: &str = "NATIVE_MAP_LOAD.md";
const UNIT: &str = "the level navigation join";

/// The refusal texts (module documentation).
pub const PAIRS: &str =
    "the level tail's navigation, cover and pylon list pairs (their position after the URL is not decoded)";
pub const HALF: &str = "a level list pair with one end None";
pub const MISSES_END: &str = "a level list whose walk from its start does not reach its end";
pub const PAST_END: &str = "a level list whose end links on to another entry";
pub const LOOP: &str = "a level list that loops back to an entry already walked";
pub const SHARED: &str = "a level list that passes through an entry already in the world's list of that kind";
pub const NOT_OF_KIND: &str = "a list entry that is not of the list kind's class";
pub const WORLD_LOOP: &str = "a world list that loops back to an entry already walked";
pub const PARTLY_JOINED: &str = "a level some of whose lists are already in the world's and others not";
pub const KIND_CLASS: &str = "a list kind's class that is not in the loaded classes";

/// The three list kinds a level carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    Navigation,
    Cover,
    Pylon,
}

impl ListKind {
    /// The kinds in splice order.
    pub const ALL: [ListKind; 3] = [ListKind::Navigation, ListKind::Cover, ListKind::Pylon];

    /// The class every entry of the kind belongs to.
    pub fn class_path(self) -> &'static str {
        match self {
            ListKind::Navigation => "Engine.NavigationPoint",
            ListKind::Cover => "Engine.CoverLink",
            ListKind::Pylon => "Engine.Pylon",
        }
    }

    /// The entry's link to the next entry.
    pub fn link(self) -> &'static str {
        match self {
            ListKind::Navigation => "nextNavigationPoint",
            ListKind::Cover => "NextCoverLink",
            ListKind::Pylon => "NextPylon",
        }
    }

    /// The `WorldInfo` field holding the world's head.
    pub fn head(self) -> &'static str {
        match self {
            ListKind::Navigation => "NavigationPointList",
            ListKind::Cover => "CoverList",
            ListKind::Pylon => "PylonList",
        }
    }

    fn name(self) -> &'static str {
        match self {
            ListKind::Navigation => "navigation",
            ListKind::Cover => "cover",
            ListKind::Pylon => "pylon",
        }
    }
}

/// A level's list ends; both None for an empty list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ListPair {
    pub start: Option<ObjRef>,
    pub end: Option<ObjRef>,
}

/// A level's three list pairs, as its tail holds them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LevelLists {
    pub navigation: ListPair,
    pub cover: ListPair,
    pub pylon: ListPair,
}

impl LevelLists {
    pub fn pair(&self, kind: ListKind) -> ListPair {
        match kind {
            ListKind::Navigation => self.navigation,
            ListKind::Cover => self.cover,
            ListKind::Pylon => self.pylon,
        }
    }
}

/// What the join did with one kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindOutcome {
    /// A null pair: nothing to join.
    Empty,
    /// Spliced in front of the world's list.
    Joined,
    /// The level's start was already in the world's list: a second join, nothing done.
    AlreadyJoined,
}

/// What the join did, per kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JoinOutcome {
    pub navigation: KindOutcome,
    pub cover: KindOutcome,
    pub pylon: KindOutcome,
}

fn refuse(kind: Option<ListKind>, what: &str) -> VmError {
    let text = match kind {
        Some(k) => format!("{UNIT} ({} list): {what}", k.name()),
        None => format!("{UNIT}: {what}"),
    };
    NotYet::not_read(text, RECORD).into()
}

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message: format!("{UNIT}: {message}") }
}

/// Object field `name` of `o`, read only.
fn object_field(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    let class = space.heap.get(o)?.class;
    let Some(p) = space.prop(class, name) else {
        return Err(script(format!("{} declares no {name}", space.classes.layout(class).path())));
    };
    match space.read(o, PropAddr::scalar(p))? {
        Value::Object(r) => Ok(*r),
        other => Err(script(format!("{name} holds {}, not an object reference", other.kind_name()))),
    }
}

fn write_field(space: &mut ObjectSpace, o: ObjRef, name: &str, v: Option<ObjRef>) -> Result<(), VmError> {
    let class = space.heap.get(o)?.class;
    let p = space.prop(class, name).ok_or_else(|| script(format!("{name} is not declared")))?;
    space.write(o, PropAddr::scalar(p), Value::Object(v))?;
    Ok(())
}

/// The level's three list pairs, from its untagged tail: always `NotYet { NotRead }` ([`PAIRS`]) until the tail is
/// decoded (module documentation). Writes nothing.
pub fn level_list_pairs(vm: &Vm, level: ObjRef) -> Result<LevelLists, VmError> {
    vm.space.heap.get(level)?;
    Err(refuse(None, PAIRS))
}

/// The join of `level` onto the world whose `WorldInfo` is `world_info`: the level's pairs ([`level_list_pairs`],
/// refused today), then [`join_level_lists`]. The entry point the bring-up calls (module documentation).
pub fn join_level(vm: &mut Vm, world_info: ObjRef, level: ObjRef) -> Result<JoinOutcome, VmError> {
    let lists = level_list_pairs(vm, level)?;
    join_level_lists(vm, world_info, &lists)
}

/// One kind's plan, from the preflight.
enum Plan {
    Empty,
    AlreadyJoined,
    Join { start: ObjRef, end: ObjRef, head: Option<ObjRef> },
}

/// Joins `lists` onto the world's lists on `world_info` (module documentation): every kind checked first, then the
/// splices. Off only.
pub fn join_level_lists(vm: &mut Vm, world_info: ObjRef, lists: &LevelLists) -> Result<JoinOutcome, VmError> {
    if vm.parity != ParityMode::Off {
        return Err(NotYet::not_ported(format!("{UNIT} under Reference (the C++ reference has none)"), RECORD).into());
    }
    let space = &vm.space;
    let world_info_class = space.classes.by_path("Engine.WorldInfo");
    let class = space.heap.get(world_info)?.class;
    if !world_info_class.is_some_and(|w| space.classes.is_child_of(class, w)) {
        return Err(script(format!("{} is not a WorldInfo", space.classes.layout(class).path())));
    }
    // 1. The preflight, every kind, reads only.
    let mut plans = Vec::with_capacity(3);
    for kind in ListKind::ALL {
        plans.push(check(space, world_info, kind, lists.pair(kind))?);
    }
    let already = plans.iter().any(|p| matches!(p, Plan::AlreadyJoined));
    let fresh = plans.iter().any(|p| matches!(p, Plan::Join { .. }));
    if already && fresh {
        return Err(refuse(None, PARTLY_JOINED));
    }
    // 2. The splices, in kind order.
    let mut outcome = [KindOutcome::Empty; 3];
    for (i, (kind, plan)) in ListKind::ALL.into_iter().zip(plans).enumerate() {
        outcome[i] = match plan {
            Plan::Empty => KindOutcome::Empty,
            Plan::AlreadyJoined => KindOutcome::AlreadyJoined,
            Plan::Join { start, end, head } => {
                write_field(&mut vm.space, end, kind.link(), head)?;
                write_field(&mut vm.space, world_info, kind.head(), Some(start))?;
                KindOutcome::Joined
            }
        };
    }
    Ok(JoinOutcome { navigation: outcome[0], cover: outcome[1], pylon: outcome[2] })
}

/// The preflight of one kind (module documentation, step 1). Reads only.
fn check(space: &ObjectSpace, world_info: ObjRef, kind: ListKind, pair: ListPair) -> Result<Plan, VmError> {
    let (start, end) = match (pair.start, pair.end) {
        (None, None) => return Ok(Plan::Empty),
        (Some(s), Some(e)) => (s, e),
        _ => return Err(refuse(Some(kind), HALF)),
    };
    let base = space.classes.by_path(kind.class_path()).ok_or_else(|| refuse(Some(kind), KIND_CLASS))?;
    let of_kind = |o: ObjRef| -> Result<(), VmError> {
        let class: ClassId = space.heap.get(o)?.class;
        if space.classes.is_child_of(class, base) { Ok(()) } else { Err(refuse(Some(kind), NOT_OF_KIND)) }
    };
    // The world's list of this kind.
    let head = object_field(space, world_info, kind.head())?;
    let mut world: DetSet<ObjRef> = DetSet::new();
    let mut cur = head;
    while let Some(o) = cur {
        if !world.insert(o) {
            return Err(refuse(Some(kind), WORLD_LOOP));
        }
        of_kind(o)?;
        cur = object_field(space, o, kind.link())?;
    }
    if world.contains(&start) {
        return Ok(Plan::AlreadyJoined);
    }
    // The level's chain, from its start to its end.
    let mut walked: DetSet<ObjRef> = DetSet::new();
    let mut cur = start;
    loop {
        of_kind(cur)?;
        if !walked.insert(cur) {
            return Err(refuse(Some(kind), LOOP));
        }
        if world.contains(&cur) {
            return Err(refuse(Some(kind), SHARED));
        }
        let next = object_field(space, cur, kind.link())?;
        if cur == end {
            if next.is_some() {
                return Err(refuse(Some(kind), PAST_END));
            }
            break;
        }
        cur = next.ok_or_else(|| refuse(Some(kind), MISSES_END))?;
    }
    Ok(Plan::Join { start, end, head })
}

#[cfg(test)]
#[path = "nav_join_tests.rs"]
mod tests;
