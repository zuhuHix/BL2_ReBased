# Native rules of the game's skill manager: how it is made, and the first call on it (draft, 2026-11-19)

AI-assisted (Claude), analyst lane FIX-SKILLMGR-A (decision 2026-11-19-11). Read from the game executable with Ghidra
(`tools/ghidra/run.ps1 -ReadOnly`), written in our own words. No decompiler output, pseudo-code, addresses or listing
structure is reproduced here. Field names come from the cooked packages (`tools/ghidra/class_layout.py`); the script
callers come from `research/script_disasm.py`. **Every rule is UNVERIFIED in the running game**. Each section ends with
the live observation that would confirm it.

**Clean room.** This lane implements nothing. In the Rust port both natives stay unbound in Off, so the gameplay stub
policy refuses them, until a fresh implementer codes them from this note alone.

**Why it matters.** Off `--combat-run maya:8` stops on `WillowGameInfo.GetSkillManager()` (installed, main 86c28d00).
That is link 1 (`spawn_maya`) of the slice gate, and links 3, 4, 5, 13 and 14 wait on it. The `maya` step runs the
controller's script `ApplyCharacterClassDefaults`. Its first statement is
`WillowGameInfo(WorldInfo.Game).GetSkillManager().DeactivateAllSkillsForInstigator(Self)`, so the second native here is
the next one that path reaches.

## Summary

| Native | Script signature (from the package) | Reached on the slice | Confidence | Status |
|---|---|---|---|---|
| `WillowGameInfo.GetSkillManager` | `()` returning `SkillEffectManager` | first statement of `ApplyCharacterClassDefaults` (the `maya` step); about 60 other script sites (skills, weapons, death, injured state) | high | UNVERIFIED |
| `SkillEffectManager.DeactivateAllSkillsForInstigator` | `(Controller)` | right after it, in the same statement | medium-high | UNVERIFIED |

## WillowGameInfo.GetSkillManager

- **What it returns:** the game info's `GlobalSkillManager` (an object property of `WillowGameInfo`, class
  `SkillEffectManager`). There is one manager per game info, shared by every player and every AI.
- **When it is made: lazily, on the first call that needs one.** If `GlobalSkillManager` is None *and* the game info's
  `Role` is `ROLE_Authority` (3), the native spawns a new `SkillEffectManager` actor into the world through the
  engine's ordinary actor spawn. It stores the result in `GlobalSkillManager` and returns it. The spawn uses:
  - the class `WillowGame.SkillEffectManager` itself (no archetype or template was seen);
  - no name;
  - location (0, 0, 0) and rotation (0, 0, 0);
  - the game info as one of the spawn's actor arguments, read as the **owner**. The argument position was not proven.
- **The stored value is type-checked:** the spawn's result is kept only if it is a `SkillEffectManager` (or a
  subclass); otherwise None is stored.
- **Later calls** return the stored manager and spawn nothing.
- **Null cases** (the result is None):
  - the game info is not the authority (`Role` other than 3) and has no manager yet. In the shipped game only the
    server has a game info at all, so a client's script reaches `WorldInfo.Game` = None before this native;
  - the spawn failed, or gave an object of another class. None is stored, so the next call tries again.
- **Not checked by the native:** whether the stored manager was destroyed since. It hands back whatever the field
  holds.
- **What a script sees:** an object of class `SkillEffectManager` (an `Actor`), the same one on every call. The class
  has no spawn-time script of its own; its spawn runs only what `Actor` provides. Its script functions are
  `ActivateSkill`, `DeactivateSkill`, `UpdateSkillGrade` and `DeactivateAllSkillTreeSkillsForPlayer`; the rest are
  natives (listed in the package, not read here except the one below). A fresh manager's `ActiveSkills` is empty.
- **For an implementer, in the port's terms:** the world needs a game info that answers it. The first call spawns the
  manager actor through the world's spawn path (with the game info as owner) and caches it on the game info. In Off,
  a world that cannot spawn should answer `NotYet` for the spawn, not None.
- **Live confirmation:** with bl-sdk on the host:
  - read `WillowGameInfo.GlobalSkillManager` right after a level load (expected None before any skill use, if no early
    caller runs) and after the player spawns;
  - check the manager's `Owner`, `Location` and `Rotation`;
  - call `GetSkillManager()` twice and compare the objects.

## SkillEffectManager.DeactivateAllSkillsForInstigator

- **Inputs:** a controller (the skills' instigator).
- **Does:** walks the manager's `ActiveSkills` (an array of `Skill` objects) in index order. For every entry that is
  set and whose `SkillInstigator` is that controller, it calls one script function of the skill by name, with no
  arguments. The name sits in a global name variable that this read did not resolve. It is read as the skill's own
  deactivation event, which is UNVERIFIED and needs a name.
- **The array is not compacted here:** removing an entry, if it happens, is the called function's business. The walk
  re-reads the array's length on every step, so an entry removed by that call shifts the next one into the slot just
  visited, and that entry is skipped. This is an edge case and not read further.
- **No result, no other side effects.**
- **On the maya path:** the manager was spawned a moment earlier, so `ActiveSkills` is empty and the call does nothing.
- **Live confirmation:** with one skill active for the player (an action skill or a passive), hook the skill's
  functions, call `DeactivateAllSkillsForInstigator(PC)` and note which function runs. Repeat with two skills of two
  instigators: only the controller's own should get the call.

## Not read

- The name the skill-walk calls (see above).
- Which engine actor arguments the spawn passes beyond class, location, rotation and the game info.
- The `SkillEffectManager` natives other than `DeactivateAllSkillsForInstigator`.
