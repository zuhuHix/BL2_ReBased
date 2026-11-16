//! Owner: lane W6-SKILLS. Skill trees: the runtime `PlayerSkillTree` built from a `SkillTreeDefinition`, the tier and
//! branch unlock rule, spending and setting grades, the reset, the state readers, and the tree natives
//! (NATIVE_SKILLS.md sections 2 and 3). AI-assisted (Claude Code).
//!
//! **Evidence.** Read from the executable (NATIVE_SKILLS.md, lane G7); every rule is **UNVERIFIED in the running game**
//! except where a line says otherwise. The host rule this replaces (a tier opens when its branch holds the sum of
//! the lower tiers' `PointsToUnlockNextTier`) matched 55 traced spends on 2026-09-27 (PHASELOCK_STOCK_DATA.md), which
//! this rule agrees with for Maya's data.
//!
//! **The tree's memory is the native class's own** (the record describes what it holds, not its layout), so a
//! [`SkillTree`] is Rust state: the natives keep one per `PlayerSkillTree` object in the VM service [`Trees`].
//!
//! **Script restated, not run** (architecture R2, a second implementation until a differential test exists):
//! [`can_upgrade`] is `SkillTreeGFxObject.CanUpgradeSkill`, [`upgrade_cost`] is
//! `WillowPlayerController.GetSkillUpgradeCost`, and [`server_upgrade`] is the point half of `ServerUpgradeSkill`.
//! The installed script calls the natives below; these restatements exist for callers that have no VM yet. A
//! differential test against the installed script is Open (it needs the skill manager's behavior-kernel natives).
//!
//! **Choices where the record is silent, each UNVERIFIED.** The branch's points (unlock rule, `GetBranchState`) count
//! every skill of its tiers; the tier's points (`GetTierState`) count only as many of its skills as the tier's layout
//! row has occupied cells (3 when the branch has no layout). For Maya's three trees every tier has exactly as many
//! skills as occupied cells, so the two readings agree there. After the walk that opens at most one tier, the child
//! branches are checked in the same call. A skill outside every tier (a proficiency) has no tier to be locked, and
//! upgrading or testing it is `NotYet { NotRead }`.

use std::cell::RefCell;
use std::rc::Rc;

use ow_attr::api::EvalCx;
use ow_attr::eval::{elements, ensure_loaded, field, flag, object, required};
use ow_combat::hit::bind;
use ow_core::NotYet;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativePack, Vm};
use ow_vm::error::VmError;

use crate::skills::{DEPTH_BUDGET, SkillError, SkillType, int, read_skill};

/// The record of the tree rules.
pub const RECORD: &str = "NATIVE_SKILLS.md sections 2-3";

/// The level below which the skill tree UI refuses a spend (`CanUpgradeSkill`, a constant in the script).
pub const TRAINING_LEVEL: i32 = 5;

/// The cells of a tier row when a branch has no layout object (NATIVE_SKILLS.md 2.3).
pub const DEFAULT_CELLS: usize = 3;

/// One tier of a branch definition.
#[derive(Debug, Clone, PartialEq)]
pub struct TierDef {
    /// `Skills`, in data order.
    pub skills: Vec<ObjRef>,
    /// `PointsToUnlockNextTier`.
    pub points_to_unlock_next: i32,
    /// The sum of `PointsToUnlockNextTier` of every lower tier of the branch (0 for the first).
    pub cumulative: i32,
    /// How many of `skills` count toward the tier's points: the layout row's occupied cells, else 3.
    pub cells: usize,
}

/// One `SkillTreeBranchDefinition`.
#[derive(Debug, Clone, PartialEq)]
pub struct BranchDef {
    pub object: ObjRef,
    /// The index of the parent branch (None for the root).
    pub parent: Option<usize>,
    /// The indices of `Children`.
    pub children: Vec<usize>,
    pub tiers: Vec<TierDef>,
    /// The sum of every tier's `PointsToUnlockNextTier` (the points that open the children).
    pub unlock_total: i32,
}

/// A `SkillTreeDefinition`: the root branch first, then its children depth first, in data order.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeDef {
    pub object: ObjRef,
    pub branches: Vec<BranchDef>,
}

fn checked_add(a: i32, b: i32) -> Result<i32, SkillError> {
    a.checked_add(b)
        .ok_or_else(|| SkillError::bad("a point total overflows"))
}

/// Reads the tree definition `tree` (its `Root` and, recursively, the `Children`; a branch reached twice is refused).
pub fn read_tree(cx: &mut EvalCx<'_>, tree: ObjRef) -> Result<TreeDef, SkillError> {
    ensure_loaded(cx, tree)?;
    let root = object("Root", &required(cx, tree, "Root")?)?
        .ok_or_else(|| SkillError::bad("a skill tree without a root"))?;
    let mut branches = Vec::new();
    read_branch(cx, root, None, &mut branches, 0)?;
    Ok(TreeDef {
        object: tree,
        branches,
    })
}

fn read_branch(
    cx: &mut EvalCx<'_>,
    obj: ObjRef,
    parent: Option<usize>,
    out: &mut Vec<BranchDef>,
    depth: usize,
) -> Result<usize, SkillError> {
    if depth >= DEPTH_BUDGET || out.len() >= DEPTH_BUDGET * DEPTH_BUDGET {
        return Err(SkillError::bad(
            "a skill tree deeper or larger than the budget",
        ));
    }
    if out.iter().any(|b| b.object == obj) {
        return Err(SkillError::bad(
            "a skill tree branch reached twice (a cycle?)",
        ));
    }
    ensure_loaded(cx, obj)?;
    let layout_rows = match object("Layout", &required(cx, obj, "Layout")?)? {
        Some(layout) => {
            ensure_loaded(cx, layout)?;
            let rows = required(cx, layout, "Tiers")?;
            let space = cx.world.parts().space;
            let mut cells = Vec::new();
            for row in elements("Tiers", &rows)? {
                // A static array of bools, one entry per cell: a struct value holds its entries consecutively, and a
                // decoded package struct holds them as one array value (both forms are read).
                let mut n = 0usize;
                for v in static_field(space, row, "bCellIsOccupied")? {
                    let cells: &[Value] = match v {
                        Value::Array(a) => a,
                        one => std::slice::from_ref(one),
                    };
                    for c in cells {
                        if flag("bCellIsOccupied", c)? {
                            n += 1;
                        }
                    }
                }
                cells.push(n);
            }
            Some(cells)
        }
        None => None,
    };
    let tiers_value = required(cx, obj, "Tiers")?;
    let mut tiers = Vec::new();
    let mut cumulative = 0i32;
    {
        let space = cx.world.parts().space;
        for (i, t) in elements("Tiers", &tiers_value)?.iter().enumerate() {
            let skills = elements("Skills", field(space, t, "Skills")?)?
                .iter()
                .map(|s| object("Skills", s))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect();
            let points = int(
                "PointsToUnlockNextTier",
                field(space, t, "PointsToUnlockNextTier")?,
            )?;
            let cells = match &layout_rows {
                Some(rows) => *rows.get(i).ok_or_else(|| {
                    SkillError::bad(format!("the branch layout has no row for tier {i}"))
                })?,
                None => DEFAULT_CELLS,
            };
            tiers.push(TierDef {
                skills,
                points_to_unlock_next: points,
                cumulative,
                cells,
            });
            cumulative = checked_add(cumulative, points)?;
        }
    }
    let index = out.len();
    out.push(BranchDef {
        object: obj,
        parent,
        children: Vec::new(),
        tiers,
        unlock_total: cumulative,
    });
    let children = elements("Children", &required(cx, obj, "Children")?)?
        .iter()
        .map(|c| object("Children", c))
        .collect::<Result<Vec<_>, _>>()?;
    for c in children.into_iter().flatten() {
        let child = read_branch(cx, c, Some(index), out, depth + 1)?;
        out[index].children.push(child);
    }
    Ok(index)
}

/// One skill the tree holds.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeSkill {
    pub definition: ObjRef,
    pub skill_type: SkillType,
    pub max_grade: i32,
    pub default_grade: i32,
    /// `(branch, tier)`; `None` for a proficiency appended from the globals.
    pub place: Option<(usize, usize)>,
    pub grade: i32,
}

/// `SkillTreeSkillStateData`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillStateData {
    pub definition: ObjRef,
    /// `ParentBranchDefinition`'s branch index.
    pub branch: usize,
    pub tier: usize,
    pub grade: i32,
    pub unlocked: bool,
}

/// `SkillTreeBranchStateData`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BranchStateData {
    pub points: i32,
    /// The sum of the maximum grades of the branch's skills.
    pub max_points: i32,
    /// The first tier's flag.
    pub unlocked: bool,
}

/// `SkillTreeTierStateData`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TierStateData {
    pub points: i32,
    pub unlocked: bool,
}

/// The runtime tree of one player (the native's private memory, as the record describes it).
#[derive(Debug, Clone, PartialEq)]
pub struct SkillTree {
    pub def: TreeDef,
    /// Every skill: the branches' skills in tree order, then the proficiencies.
    pub skills: Vec<TreeSkill>,
    /// One flag per tier, per branch.
    pub unlocked: Vec<Vec<bool>>,
    /// The first skill of type Action found at build time.
    pub action_skill: Option<ObjRef>,
}

impl SkillTree {
    /// `PlayerSkillTree.Initialize(Definition)`: builds the records from `def`, appends `proficiencies` (the globals'
    /// proficiency skills), records the action skill, then resets without proficiency preservation (every grade at
    /// its `DefaultStartingGrade`, only the root's first tier unlocked).
    pub fn initialize(
        cx: &mut EvalCx<'_>,
        def: TreeDef,
        proficiencies: &[ObjRef],
    ) -> Result<SkillTree, SkillError> {
        let mut skills = Vec::new();
        for (b, branch) in def.branches.iter().enumerate() {
            for (t, tier) in branch.tiers.iter().enumerate() {
                for &s in &tier.skills {
                    skills.push(tree_skill(cx, s, Some((b, t)))?);
                }
            }
        }
        for &p in proficiencies {
            skills.push(tree_skill(cx, p, None)?);
        }
        let action_skill = skills
            .iter()
            .find(|s| s.skill_type == SkillType::Action)
            .map(|s| s.definition);
        let unlocked = def
            .branches
            .iter()
            .map(|b| vec![false; b.tiers.len()])
            .collect();
        let mut tree = SkillTree {
            def,
            skills,
            unlocked,
            action_skill,
        };
        tree.reset(false)?;
        Ok(tree)
    }

    fn index(&self, skill: ObjRef) -> Option<usize> {
        self.skills.iter().position(|s| s.definition == skill)
    }

    /// The internal reset (NATIVE_SKILLS.md 3.1): every grade to its `DefaultStartingGrade` (proficiencies keep
    /// theirs when `ignore_proficiencies`), every tier locked, then the root's first tier unlocked. Returns the refund:
    /// the points spent before minus after.
    pub fn reset(&mut self, ignore_proficiencies: bool) -> Result<i32, SkillError> {
        let before = self.points_spent()?;
        for s in &mut self.skills {
            if ignore_proficiencies && s.skill_type == SkillType::Proficiency {
                continue;
            }
            s.grade = s.default_grade.clamp(0, s.max_grade.max(0));
        }
        for tiers in &mut self.unlocked {
            tiers.iter_mut().for_each(|t| *t = false);
        }
        if let Some(first) = self.unlocked.first_mut().and_then(|t| t.first_mut()) {
            *first = true;
        }
        let after = self.points_spent()?;
        before
            .checked_sub(after)
            .ok_or_else(|| SkillError::bad("a refund overflows"))
    }

    /// The points in branch `b`: the grades of every skill of its tiers.
    pub fn branch_points(&self, b: usize) -> Result<i32, SkillError> {
        let mut sum = 0i32;
        for s in &self.skills {
            if matches!(s.place, Some((sb, _)) if sb == b) {
                sum = checked_add(sum, s.grade)?;
            }
        }
        Ok(sum)
    }

    /// The points in tier `t` of branch `b`: the grades of its first `cells` skills (module documentation).
    pub fn tier_points(&self, b: usize, t: usize) -> Result<i32, SkillError> {
        let tier = self
            .def
            .branches
            .get(b)
            .and_then(|x| x.tiers.get(t))
            .ok_or_else(|| SkillError::bad("no such tier"))?;
        let mut sum = 0i32;
        for d in tier.skills.iter().take(tier.cells) {
            if let Some(i) = self.index(*d) {
                sum = checked_add(sum, self.skills[i].grade)?;
            }
        }
        Ok(sum)
    }

    /// `GetSkillPointsSpentInTree()`: the sum of every branch's points (proficiencies excluded).
    pub fn points_spent(&self) -> Result<i32, SkillError> {
        let mut sum = 0i32;
        for b in 0..self.def.branches.len() {
            sum = checked_add(sum, self.branch_points(b)?)?;
        }
        Ok(sum)
    }

    /// The unlock rule of NATIVE_SKILLS.md 2.3 for branch `b`: at the first locked tier, unlock it when the branch's
    /// points reach its cumulative requirement, and stop either way (one tier per call); then, when every tier is
    /// unlocked and the branch holds its unlock total, unlock the first tier of every child.
    ///
    /// A branch whose first tier is locked opens nothing itself: that tier opens only by the reset (the root) or by
    /// its parent (the record), so the walk starts once it is open. (Read so because a first tier's cumulative
    /// requirement is 0, and a walk from it would open it on the first grade the load path sets.)
    pub fn run_unlock(&mut self, b: usize) -> Result<(), SkillError> {
        if !self.unlocked[b].first().copied().unwrap_or(false) {
            return Ok(());
        }
        let points = self.branch_points(b)?;
        let branch = &self.def.branches[b];
        if let Some(t) = self.unlocked[b].iter().position(|u| !u)
            && points >= branch.tiers[t].cumulative
        {
            self.unlocked[b][t] = true;
        }
        if self.unlocked[b].iter().all(|u| *u) && points >= branch.unlock_total {
            for &c in &branch.children.clone() {
                if let Some(first) = self.unlocked[c].first_mut() {
                    *first = true;
                }
            }
        }
        Ok(())
    }

    /// `SetSkillGrade(Skill, Grade)`: clamped to `0..=MaxGrade`; when it changed, the unlock rule of the skill's
    /// branch runs and the answer is true. False for an unknown skill or an unchanged grade. No tier check (the load
    /// and replay path).
    pub fn set_grade(&mut self, skill: ObjRef, grade: i32) -> Result<bool, SkillError> {
        let Some(i) = self.index(skill) else {
            return Ok(false);
        };
        let s = &mut self.skills[i];
        let new = grade.clamp(0, s.max_grade.max(0));
        if new == s.grade {
            return Ok(false);
        }
        s.grade = new;
        if let Some((b, _)) = s.place {
            self.run_unlock(b)?;
        }
        Ok(true)
    }

    /// `UpgradeSkill(Skill)`: false (no change) for a skill not in the tree, a locked tier or a maxed skill;
    /// otherwise grade + 1 through [`SkillTree::set_grade`]. A proficiency (no tier) is `NotYet { NotRead }`.
    pub fn upgrade(&mut self, skill: ObjRef) -> Result<bool, SkillError> {
        let Some(i) = self.index(skill) else {
            return Ok(false);
        };
        let s = &self.skills[i];
        let Some((b, t)) = s.place else {
            return Err(NotYet::not_read(
                "UpgradeSkill on a skill outside every tier (a proficiency)",
                RECORD,
            )
            .into());
        };
        if !self.unlocked[b][t] || s.grade >= s.max_grade {
            return Ok(false);
        }
        let next = s
            .grade
            .checked_add(1)
            .ok_or_else(|| SkillError::bad("a grade overflows"))?;
        self.set_grade(skill, next)
    }

    /// `GetSkillState(Skill, out State)`; `None` for a skill the tree does not know. A proficiency is
    /// `NotYet { NotRead }` (no branch or tier to report).
    pub fn skill_state(&self, skill: ObjRef) -> Result<Option<SkillStateData>, SkillError> {
        let Some(i) = self.index(skill) else {
            return Ok(None);
        };
        let s = &self.skills[i];
        let Some((b, t)) = s.place else {
            return Err(NotYet::not_read(
                "GetSkillState of a skill outside every tier (a proficiency)",
                RECORD,
            )
            .into());
        };
        Ok(Some(SkillStateData {
            definition: s.definition,
            branch: b,
            tier: t,
            grade: s.grade,
            unlocked: self.unlocked[b][t],
        }))
    }

    /// `GetBranchState(Branch, out State)`; `None` for a branch the tree does not know.
    pub fn branch_state(&self, branch: ObjRef) -> Result<Option<BranchStateData>, SkillError> {
        let Some(b) = self.def.branches.iter().position(|x| x.object == branch) else {
            return Ok(None);
        };
        let mut max_points = 0i32;
        for s in &self.skills {
            if matches!(s.place, Some((sb, _)) if sb == b) {
                max_points = checked_add(max_points, s.max_grade)?;
            }
        }
        Ok(Some(BranchStateData {
            points: self.branch_points(b)?,
            max_points,
            unlocked: self.unlocked[b].first().copied().unwrap_or(false),
        }))
    }

    /// `GetTierState(Branch, Tier, out State)`; `None` for an unknown branch or tier.
    pub fn tier_state(
        &self,
        branch: ObjRef,
        tier: usize,
    ) -> Result<Option<TierStateData>, SkillError> {
        let Some(b) = self.def.branches.iter().position(|x| x.object == branch) else {
            return Ok(None);
        };
        if tier >= self.def.branches[b].tiers.len() {
            return Ok(None);
        }
        Ok(Some(TierStateData {
            points: self.tier_points(b, tier)?,
            unlocked: self.unlocked[b][tier],
        }))
    }

    /// `HasTrainedASkillOfType(Type)`: a skill of that type with grade above 0 whose tier is unlocked. A skill outside
    /// every tier of that type is `NotYet { NotRead }`.
    pub fn has_trained_of_type(&self, t: SkillType) -> Result<bool, SkillError> {
        for s in self
            .skills
            .iter()
            .filter(|s| s.skill_type == t && s.grade > 0)
        {
            match s.place {
                Some((b, tier)) if self.unlocked[b][tier] => return Ok(true),
                Some(_) => {}
                None => {
                    return Err(NotYet::not_read(
                        "HasTrainedASkillOfType for a skill outside every tier",
                        RECORD,
                    )
                    .into());
                }
            }
        }
        Ok(false)
    }

    /// The tree's grade of `skill` (`None` when the tree does not hold it).
    pub fn grade(&self, skill: ObjRef) -> Option<i32> {
        self.index(skill).map(|i| self.skills[i].grade)
    }
}

fn tree_skill(
    cx: &mut EvalCx<'_>,
    s: ObjRef,
    place: Option<(usize, usize)>,
) -> Result<TreeSkill, SkillError> {
    let def = read_skill(cx, s)?;
    Ok(TreeSkill {
        definition: s,
        skill_type: def.skill_type,
        max_grade: def.max_grade,
        default_grade: def.default_starting_grade,
        place,
        grade: 0,
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Script restated (module documentation)
// ---------------------------------------------------------------------------------------------------------------

/// `ESkillTreeFailureReason` (the UI's answer).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeFailure {
    NoSkillPoints,
    SkillLocked,
    SkillMaxed,
    DataIssue,
    NoFailure,
}

/// `SkillTreeGFxObject.CanUpgradeSkill`, in its order: no state for the skill, a data issue; a locked tier or a
/// player below level 5, locked; at `MaxGrade`, maxed; no unspent points, none; otherwise no failure.
pub fn can_upgrade(
    tree: &SkillTree,
    skill: ObjRef,
    exp_level: i32,
    points: i32,
) -> Result<UpgradeFailure, SkillError> {
    let Some(state) = tree.skill_state(skill)? else {
        return Ok(UpgradeFailure::DataIssue);
    };
    if !state.unlocked || exp_level < TRAINING_LEVEL {
        return Ok(UpgradeFailure::SkillLocked);
    }
    let max = tree
        .skills
        .iter()
        .find(|s| s.definition == skill)
        .map(|s| s.max_grade)
        .unwrap_or(0);
    if state.grade >= max {
        return Ok(UpgradeFailure::SkillMaxed);
    }
    if points <= 0 {
        return Ok(UpgradeFailure::NoSkillPoints);
    }
    Ok(UpgradeFailure::NoFailure)
}

/// `GetSkillUpgradeCost`: 0 for a Proficiency skill, otherwise 1.
pub fn upgrade_cost(t: SkillType) -> i32 {
    if t == SkillType::Proficiency { 0 } else { 1 }
}

/// The point half of `ServerUpgradeSkill(Skill)`: with at least the cost unspent, `UpgradeSkill`; only on true is the
/// cost subtracted. Returns the new grade, or `None` when nothing changed. The grade's effects and the client update
/// that follow in the script are the skill manager's ([`crate::action_skill`]).
pub fn server_upgrade(
    tree: &mut SkillTree,
    skill: ObjRef,
    points: &mut i32,
) -> Result<Option<i32>, SkillError> {
    let Some(i) = tree.index(skill) else {
        return Ok(None);
    };
    let cost = upgrade_cost(tree.skills[i].skill_type);
    if *points < cost || !tree.upgrade(skill)? {
        return Ok(None);
    }
    *points = points
        .checked_sub(cost)
        .ok_or_else(|| SkillError::bad("the points underflow"))?;
    Ok(tree.grade(skill))
}

// ---------------------------------------------------------------------------------------------------------------
// The natives
// ---------------------------------------------------------------------------------------------------------------

/// The trees of the world's `PlayerSkillTree` objects (a VM service).
#[derive(Debug, Default)]
pub struct Trees {
    trees: Vec<(ObjRef, SkillTree)>,
}

impl Trees {
    pub fn get(&self, tree: ObjRef) -> Option<&SkillTree> {
        self.trees.iter().find(|(o, _)| *o == tree).map(|(_, t)| t)
    }
    pub fn get_mut(&mut self, tree: ObjRef) -> Option<&mut SkillTree> {
        self.trees
            .iter_mut()
            .find(|(o, _)| *o == tree)
            .map(|(_, t)| t)
    }
    /// Sets the tree of `object` (replacing one there).
    pub fn put(&mut self, object: ObjRef, tree: SkillTree) {
        match self.get_mut(object) {
            Some(t) => *t = tree,
            None => self.trees.push((object, tree)),
        }
    }
}

/// The VM's [`Trees`] service, added empty on first use.
pub fn install(vm: &mut Vm) -> Result<Rc<RefCell<Trees>>, VmError> {
    match vm.service::<Trees>() {
        Some(t) => Ok(t),
        None => vm.add_service(Trees::default()),
    }
}

/// The function paths this module binds, by path (`ow_combat::hit::bind`: the key is the loaded declaration's, a
/// function the VM does not declare is skipped, and the first binding of a key wins).
pub const NATIVE_PATHS: [&str; 6] = [
    "WillowGame.PlayerSkillTree.Initialize",
    "WillowGame.PlayerSkillTree.UpgradeSkill",
    "WillowGame.PlayerSkillTree.SetSkillGrade",
    "WillowGame.PlayerSkillTree.GetSkillPointsSpentInTree",
    "WillowGame.PlayerSkillTree.GetActionSkill",
    "WillowGame.PlayerSkillTree.HasTrainedASkillOfType",
];

fn this_tree(c: &NativeCall<'_>, key: &str) -> Result<ObjRef, VmError> {
    c.this.ok_or_else(|| VmError::Script {
        function: key.to_string(),
        message: "called without a skill tree".to_string(),
    })
}

/// Runs `f` on the tree of `this`; a tree object that was never initialized is `NotYet { NotRead }` (what an
/// uninitialized native tree answers is not read).
fn with_tree<R>(
    c: &mut NativeCall<'_>,
    key: &str,
    f: impl FnOnce(&mut SkillTree) -> Result<R, SkillError>,
) -> Result<R, VmError> {
    let this = this_tree(c, key)?;
    let trees = install(c.vm)?;
    let mut trees = trees
        .try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("ow_skills::tree::Trees"))?;
    match trees.get_mut(this) {
        Some(t) => Ok(f(t)?),
        None => Err(NotYet::not_read(
            format!("{key} on a PlayerSkillTree that was never initialized"),
            RECORD,
        )
        .into()),
    }
}

fn obj_arg(c: &NativeCall<'_>, i: usize) -> Result<Option<ObjRef>, VmError> {
    match c.arg(i)? {
        Value::Object(o) => Ok(*o),
        Value::None => Ok(None),
        other => Err(VmError::Script {
            function: "ow-skills".to_string(),
            message: format!(
                "argument {i}: expected an object, found {}",
                other.kind_name()
            ),
        }),
    }
}

/// The skill type of enum byte `b` (`ESkillType` Passive 0 .. Proficiency 4, NATIVE_SKILLS.md "Enums").
fn skill_type_of_byte(b: u8) -> Option<SkillType> {
    Some(match b {
        0 => SkillType::Passive,
        1 => SkillType::Action,
        2 => SkillType::ActionAugment,
        3 => SkillType::Kill,
        4 => SkillType::Proficiency,
        _ => return None,
    })
}

/// The tree natives as a [`NativePack`]: `Initialize` builds the [`SkillTree`] of the object (a second call is a
/// no-op, as the record says) with no proficiencies (reading the globals' list is Open: the field is not named in the
/// record); the others read or change it.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        install(vm)?;
        register_player_skill_query(vm)?;
        let [init, upgrade, set, spent, action, trained] = NATIVE_PATHS;
        bind(
            vm,
            init,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let this = this_tree(c, init)?;
                let trees = install(c.vm)?;
                if trees
                    .try_borrow()
                    .map_err(|_| VmError::ServiceBorrowed("ow_skills::tree::Trees"))?
                    .get(this)
                    .is_some()
                {
                    return Ok(Value::None);
                }
                let Some(def) = obj_arg(c, 0)? else {
                    return Err(NotYet::not_read(
                        "PlayerSkillTree.Initialize with no definition",
                        RECORD,
                    )
                    .into());
                };
                let tree = {
                    let mut cx = EvalCx::new(&mut *c.vm);
                    let d = read_tree(&mut cx, def)?;
                    SkillTree::initialize(&mut cx, d, &[])?
                };
                trees
                    .try_borrow_mut()
                    .map_err(|_| VmError::ServiceBorrowed("ow_skills::tree::Trees"))?
                    .put(this, tree);
                Ok(Value::None)
            }),
        )?;
        bind(
            vm,
            upgrade,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let skill = obj_arg(c, 0)?;
                with_tree(c, upgrade, |t| {
                    Ok(Value::Bool(match skill {
                        Some(s) => t.upgrade(s)?,
                        None => false,
                    }))
                })
            }),
        )?;
        bind(
            vm,
            set,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let skill = obj_arg(c, 0)?;
                let grade = match c.arg(1)? {
                    Value::Int(i) => *i,
                    other => {
                        return Err(VmError::Script {
                            function: set.to_string(),
                            message: format!(
                                "SkillGrade: expected an int, found {}",
                                other.kind_name()
                            ),
                        });
                    }
                };
                with_tree(c, set, |t| {
                    Ok(Value::Bool(match skill {
                        Some(s) => t.set_grade(s, grade)?,
                        None => false,
                    }))
                })
            }),
        )?;
        bind(
            vm,
            spent,
            Rc::new(move |c: &mut NativeCall<'_>| {
                with_tree(c, spent, |t| Ok(Value::Int(t.points_spent()?)))
            }),
        )?;
        bind(
            vm,
            action,
            Rc::new(move |c: &mut NativeCall<'_>| {
                with_tree(c, action, |t| Ok(Value::Object(t.action_skill)))
            }),
        )?;
        bind(
            vm,
            trained,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let b = match c.arg(0)? {
                    Value::Byte(b) => *b,
                    other => {
                        return Err(VmError::Script {
                            function: trained.to_string(),
                            message: format!(
                                "SkillType: expected a byte, found {}",
                                other.kind_name()
                            ),
                        });
                    }
                };
                let t = skill_type_of_byte(b).ok_or_else(|| VmError::Script {
                    function: trained.to_string(),
                    message: format!("SkillType {b} is outside ESkillType"),
                })?;
                with_tree(c, trained, |tree| {
                    Ok(Value::Bool(tree.has_trained_of_type(t)?))
                })
            }),
        )?;
        Ok(())
    }
}

/// The answer to `EngineQuery::PlayerSkill` (`PlayerSkillAttributeValueResolver`), registered on a world's query
/// service when there is one: `NotYet { NotPorted }` naming the contract gap. The resolver (`ow-attr`) reads `Grade`
/// on the object the query returns, but the record's value is the grade **in the tree** (NATIVE_ATTRIBUTES.md section
/// 6), which is the tree's own memory ([`SkillTree::grade`]), not an object property. The active `Skill` object's
/// `Grade` is `max(grade, 1)` and exists only while the skill runs, so returning it would answer 0 for a trained skill
/// that is not active (an action skill between casts) and 1 for nothing at grade 0: a guessed value, refused here.
/// Contract item for the hub: a grade-valued query (decision 2026-11-16-42).
pub fn register_player_skill_query(vm: &mut Vm) -> Result<(), VmError> {
    use ow_world::query::{QueryFn, QueryKind, WorldQuery};
    let Some(q) = vm.service::<WorldQuery>() else {
        return Ok(());
    };
    let mut q = q
        .try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("ow_world::query::WorldQuery"))?;
    if q.has_handler(QueryKind::PlayerSkill) {
        return Ok(());
    }
    let handler: QueryFn = Rc::new(|query, source, _w| {
        Err(NotYet::not_ported(
            format!("{query:?} from {source:?}: the tree's grade is not an object the resolver can read (contract item, decision 2026-11-16-42)"),
            RECORD,
        )
        .into())
    });
    q.register(QueryKind::PlayerSkill, handler)
}

/// Every entry of the static-array field `name` of the struct value `v` (a static-array field takes `array_dim`
/// consecutive entries of the struct's value).
pub(crate) fn static_field<'v>(
    space: &ow_object::heap::ObjectSpace,
    v: &'v Value,
    name: &str,
) -> Result<&'v [Value], SkillError> {
    let s = v
        .as_struct()
        .ok_or_else(|| SkillError::bad(format!("expected a struct with field {name}")))?;
    let layout = space
        .classes
        .struct_layout(s.ty)
        .ok_or_else(|| SkillError::bad("a struct without a layout"))?;
    let (entry, decl) = space
        .names
        .find(name)
        .and_then(|n| layout.field(n))
        .ok_or_else(|| {
            SkillError::bad(format!(
                "struct {} has no field {name}",
                layout.path().as_str()
            ))
        })?;
    let start =
        usize::try_from(entry).map_err(|_| SkillError::bad("a field entry out of range"))?;
    let dim = usize::try_from(decl.array_dim)
        .map_err(|_| SkillError::bad("an array size out of range"))?;
    let end = start
        .checked_add(dim)
        .ok_or_else(|| SkillError::bad("an array size out of range"))?;
    s.fields.get(start..end).ok_or_else(|| {
        SkillError::bad(format!(
            "struct {} value lacks field {name}",
            layout.path().as_str()
        ))
    })
}

#[cfg(test)]
mod tests;
