# Native rules of a controller's class starting values: ApplyCharacterClassStartingValues (draft, 2026-11-19)

AI-assisted (Claude), analyst lane ANALYST-CLASS-STARTING-VALUES (decision 2026-11-19-50). The natives were read
from the game executable with Ghidra (`tools/ghidra/run.ps1 -ReadOnly`, under the machine lock), and the script they
reach from the installed packages with `research/script_disasm.py`. Written in our own words, as behaviour only: no
decompiler output, pseudo-code, quoted script statements, addresses, offsets or call structure is reproduced here.
Field names come from the cooked packages and from NATIVE_COMBAT_GROUNDWORK.md section 1.10, which named them first.
**Every rule is UNVERIFIED in the running game.** The last section lists the live observations that would confirm them.

**Clean room.** This lane implements nothing. The natives stay unbound or `NotYet` in Off until a fresh implementer
codes them from this note alone.

**Why it matters.** Lane H6 (FIX-PLAYER-CLASS-B, decision 2026-11-19-46) found that the player-controller override
ends with a parameterless script call that NATIVE_COMBAT_GROUNDWORK.md section 1.10 step 2 left unnamed. This note
names it and states the whole rule in one place.

## Summary

| Native | Script signature (from the package) | Confidence | Status |
|---|---|---|---|
| `Controller.ApplyCharacterClassStartingValues(CharacterClassDefinition)` | native | high | UNVERIFIED |
| `WillowPlayerController.ApplyCharacterClassStartingValues(CharacterClassDefinition)` | native, an override of the one above | high | UNVERIFIED |
| `WillowPlayerController.OnPlayerClassChange()` | script event, called by the override | high | UNVERIFIED |

The script native is declared on `Controller`. A `WillowPlayerController` behaves as in section 2, and every other
controller, AI minds included (NATIVE_COMBAT_GROUNDWORK.md section 1.11), as in section 1.

## 1. The base: Controller.ApplyCharacterClassStartingValues(Class)

In this order:
1. **Every pool of the controller's `ResourcePoolManager` is deleted**, with or without a class. The deletion itself
   (`DeleteAllPools`, NATIVE_COMBAT_GROUNDWORK.md section 1.6) acts only on a manager whose own `Role` is the
   authority; on any other manager it deletes nothing.
2. With a class:
   1. one pool for each entry of the class's `ResourcePools`, in array order;
   2. then a pool for `AccuracyPoolDefinition`, when set, stored as the controller's `AccuracyPool`;
   3. then a pool for `OffHandAccuracyPoolDefinition`, when set, stored as `OffHandAccuracyPool`.

   Each pool is made by the manager's pool creation, which takes the lowest free slot (NATIVE_COMBAT_GROUNDWORK.md
   sections 1.4 and 1.10).
3. Nothing else: no attribute, inventory, pawn or health change, and no check of the controller's role.

**Stored references that are not recreated.** Neither native clears a stored pool reference. A reference that is not
recreated keeps what it held before the call. Which ones are left, by the gate that applies:
- **No class argument:** `AccuracyPool` and `OffHandAccuracyPool` (section 1). The three player-only references are
  gated separately (section 2), not by the argument.
- **An unset definition:** only that definition's own reference (`AccuracyPoolDefinition` → `AccuracyPool`, and so
  on). The others are created as usual.
- **A controller that is not the authority, or one with no `PlayerClass`:** `ExpPool`, `SkillCooldownPool` and
  `MeleeSkillCooldownPool` (section 2). The two accuracy references follow the class argument as above.

Whether a left reference is stale depends on step 1, that is, on the **manager's** authority (not the controller's):
- **The manager is the authority:** step 1 emptied the reference's slot. By the validity rule
  (NATIVE_COMBAT_GROUNDWORK.md section 1.2, the `PoolGUID` check) the reference reads as not valid while the slot
  stays empty, or holds a new pool with another GUID. It reads as valid again only if a new pool in that slot carries
  the same GUID, which was not checked (UNVERIFIED; open question 2).
- **The manager is not the authority:** step 1 deleted nothing, so the left reference still names the pool it named
  before, which is still live. What the creation steps then do on such a manager was not read here (UNVERIFIED; open
  question 3).

## 2. The player-controller override

In this order:
1. **The base rule** (section 1) runs first, with the same class.
2. **Only on the authority** (the controller's `Role` is `ROLE_Authority`):
   1. **with a `PlayerClass`** (the controller's own field, not the argument): pools for the player class's
      `ExperiencePoolDefinition`, `SkillCooldownPoolDefinition` and `MeleeSkillCooldownPoolDefinition`, in that order,
      each only when set. They are stored as `ExpPool`, `SkillCooldownPool` and `MeleeSkillCooldownPool`;
   2. **then, with or without a `PlayerClass`,** the controller's script event **`OnPlayerClassChange`** runs, with no
      arguments.

   This corrects one detail of NATIVE_COMBAT_GROUNDWORK.md section 1.10 step 2, which put the call inside the
   `PlayerClass` case: it needs only the authority.
3. A controller that is not the authority gets only the base rule. The three references above are then left as they
were (section 1, "Stored references that are not recreated").

## 3. OnPlayerClassChange (script, orientation only)

Its effects: the skill tree is rebuilt (through `InitPlayerSkillTree`, native, already in NATIVE_INDEX.md). On the
authority, with a `WillowPlayerReplicationInfo`, the character name (from the class's `CharacterNameId`) and the
movement type are set. Then the input devices are set up. The port runs the installed script; a network client
reaches the same event through `ClientNotifyClassChanged`.

## 4. What the class definition supplies

- **The base:** `ResourcePools`, `AccuracyPoolDefinition`, `OffHandAccuracyPoolDefinition`. These are fields of
  `CharacterClassDefinition`.
- **The override:** `ExperiencePoolDefinition`, `SkillCooldownPoolDefinition`, `MeleeSkillCooldownPoolDefinition`.
  These are fields of `PlayerClassDefinition`, read from the controller's `PlayerClass`.
- Nothing else of the class is read by the natives. `OnPlayerClassChange` reads `CharacterNameId` and the default name.

For Maya this gives the slot layout NATIVE_COMBAT_GROUNDWORK.md section 1.10 lists: the seven ammo pools, then
accuracy, experience, action-skill cooldown and melee cooldown.

## 5. For an implementer, in the port's terms

- A `WillowPlayerController` behaves as in section 2, and every other controller as in section 1.
- The override always runs the base first. The authority check gates the three extra pools and the event, and the
  `PlayerClass` check gates only the pools.
- `OnPlayerClassChange` is installed script; the port runs it through the VM. Its one native, `InitPlayerSkillTree`,
  is already read.

## Open questions

1. Whether the class argument and the controller's `PlayerClass` can differ when the override runs (the base reads
   the argument, the override reads the field).
2. After a class application that does not recreate a stored pool reference, whether the left reference can ever read
   as valid again (a new pool in the same slot with the same `PoolGUID`). UNVERIFIED.
3. Whether the controller's manager can be other than the authority when the call runs (for example on a network
   client, through `ClientNotifyClassChanged`'s path), and what pool creation does on such a manager. UNVERIFIED.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- After a class is applied to Maya's controller, read `ResourcePoolManager.ResourcePools[i].Definition` for every slot,
  and `ExpPool`, `SkillCooldownPool` and `MeleeSkillCooldownPool`.
- Hook `OnPlayerClassChange` and confirm it runs once per class application on the host.
- Before and after a class application, read `AccuracyPool`, `OffHandAccuracyPool`, `ExpPool`, `SkillCooldownPool`
  and `MeleeSkillCooldownPool` (manager, slot, GUID) and `IsResourcePoolValid` on each, for a class that leaves one
  of the definitions unset (open question 2).
- On a network client, read the controller's `ResourcePoolManager.Role` and its pools before and after a class
  application (open question 3).
