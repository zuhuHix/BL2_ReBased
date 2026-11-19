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

Both natives share one script entry. Each controller class answers with its own rule: `WillowPlayerController` with
the override, and every other controller (an AI mind included, NATIVE_COMBAT_GROUNDWORK.md section 1.11) with the base.

## 1. The base: Controller.ApplyCharacterClassStartingValues(Class)

In this order:
1. **Every pool of the controller's `ResourcePoolManager` is deleted.** This happens always, with or without a class.
2. With a class:
   1. one pool for each entry of the class's `ResourcePools`, in array order;
   2. then a pool for `AccuracyPoolDefinition`, when set, stored as the controller's `AccuracyPool`;
   3. then a pool for `OffHandAccuracyPoolDefinition`, when set, stored as `OffHandAccuracyPool`.

   Each pool is made by the manager's pool creation, which takes the lowest free slot (NATIVE_COMBAT_GROUNDWORK.md
   sections 1.4 and 1.10).
3. Nothing else: no attribute, inventory, pawn or health change, and no role check.

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
3. A controller that is not the authority gets only the base rule.

## 3. OnPlayerClassChange (script, for reference)

The event the override fires, in order:
1. clears `InputContextSelector` and `PlayerSkillTree`, then rebuilds the skill tree (`InitPlayerSkillTree`, native,
   already in NATIVE_INDEX.md);
2. on the authority, with a `WillowPlayerReplicationInfo`:
   - sets the replication info's `CharacterNameIdDef` to the player class's `CharacterNameId`;
   - applies the controller's `PlayerMovementType` (`SetPlayerMovementType`);
   - sets `PlayerPreferredCharacterName` to the class's default name (`GetCharacterClassDefaultName`);
   - asks the replication info about the name (`IsInvalidName`);
3. sets up the input devices (`SetupInputDevices`).

Everything after the skill tree rebuild is script. A network client reaches the same event through
`ClientNotifyClassChanged` instead.

## 4. What the class definition supplies

- **The base:** `ResourcePools`, `AccuracyPoolDefinition`, `OffHandAccuracyPoolDefinition`. These are fields of
  `CharacterClassDefinition`.
- **The override:** `ExperiencePoolDefinition`, `SkillCooldownPoolDefinition`, `MeleeSkillCooldownPoolDefinition`.
  These are fields of `PlayerClassDefinition`, read from the controller's `PlayerClass`.
- Nothing else of the class is read by the natives. `OnPlayerClassChange` reads `CharacterNameId` and the default name.

For Maya this gives the slot layout NATIVE_COMBAT_GROUNDWORK.md section 1.10 lists: the seven ammo pools, then
accuracy, experience, action-skill cooldown and melee cooldown.

## 5. For an implementer, in the port's terms

- One rule per controller class: the base for every controller, and the override for `WillowPlayerController`.
- The override always runs the base first. The authority check gates the three extra pools and the event, and the
  `PlayerClass` check gates only the pools.
- `OnPlayerClassChange` is installed script; the port runs it through the VM. Its one native, `InitPlayerSkillTree`,
  is already read.

## Open questions

1. Whether the class argument and the controller's `PlayerClass` can differ when the override runs (the base reads
   the argument, the override reads the field).

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- After a class is applied to Maya's controller, read `ResourcePoolManager.ResourcePools[i].Definition` for every slot,
  and `ExpPool`, `SkillCooldownPool` and `MeleeSkillCooldownPool`.
- Hook `OnPlayerClassChange` and confirm it runs once per class application on the host.
