# `WillowPlayerController.UnlockPlayerClassAchievementIfConditionsMet` on link 9 (draft, 2026-11-20)

AI-assisted (Claude Code, Claude Opus 5.5), analyst lane ANALYST-UNLOCKPLAYERCLASSACHIEVEMENT (decision
`docs/decisions/2026-11/2026-11-19-NNN-unlock-class-achievement.md`, number pending). Analyst only: a different lane
implements it (clean room).

**Sources:**
- **bl-sdk:** no live reads are recorded for this native, and no session was granted. Nothing here is observed.
- **The script** (`research/script_disasm.py`; the local script search over the nine code packages): its three
  callers, and the caller on link 9's path.
- **The native:** `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the
  machine lock): the exec step, its body and the two routines the body calls.
- **Field and parameter names:** `tools/ghidra/class_layout.py`, on the native's parameters, `PlayerClassDefinition` and
  its `PlayerClassAchievementUnlockData`.
- **The installed data:** Maya's class definition through `ow-package --def-dump` (counts and values only are
  recorded here).
- **The sibling native:** NATIVE_UNLOCK_ACHIEVEMENT_IF_CONDITIONS_MET.md (`UnlockAchievementIfConditionsMet`), whose
  script event this one shares.

Raw output stays under ignored `local/analysis/upca/`. This note is in our own words: no decompiler output,
pseudo-code, addresses, absolute offsets or listing structure. **Every rule here is UNVERIFIED** in the running game.

## 1. What it is, and who calls it

`native final function bool UnlockPlayerClassAchievementIfConditionsMet(byte AchievementUnlockType, int IntData, bool
bCanBeGreater, bool bCanBeLess)` on `WillowPlayerController`. It is **native**, with no script body. Its three script
callers on the controller each pass a constant type, a count, `bCanBeGreater` true and `bCanBeLess` omitted (false):

| Caller | Type | Count |
|---|---|---|
| `NotifyPetKilledEnemy` | 0 | the controller's `PlayerStats` pet-kill stat, after its increment |
| `NotifyActionSkillRunTime` | 1 | its own run-time argument |
| `NotifyTargetedActionSkillImpact` | 2 | the `PlayerStats` targeted-action-skill-impact stat, after its increment |

**Link 9's path:** `LiftActionSkill.BeginLifting` calls `NotifyTargetedActionSkillImpact`, which increments the stat
and then calls this native with type 2. The return value is not used by any of the three callers.

## 2. What it does

1. **The class definition.** It takes the controller's player-class field and casts it to `PlayerClassDefinition`.
   The field is by meaning `PlayerClass`, the one the port's harness reads; the layout tool cannot place
   `WillowPlayerController`'s fields, so the name is a fit (UNVERIFIED). The array the next step reads lands exactly on
   the layout tool's `PlayerClassAchievementData`. A field that is None, or holds an object that is not a
   `PlayerClassDefinition`, gives **false**, with nothing else read.
2. **The selection.** It walks the definition's `PlayerClassAchievementData` in order. Each entry is an
   `UnlockType` (byte), an `Achievement` (byte) and an `IntData` (int). An entry is selected when **all** of these hold:
   - its `Achievement` is not 0;
   - its `UnlockType` equals the argument `AchievementUnlockType`;
   - its `IntData` is at least 1;
   - and one of:
     - its `IntData` equals the argument `IntData`;
     - `bCanBeGreater` is set and the argument is at least the entry's `IntData`;
     - `bCanBeLess` is set and the argument is at most the entry's `IntData`.

   Every selected entry's `Achievement` is kept, in the array's order, duplicates included.
3. **The unlock.** For each kept achievement, in order, it sends the controller a script event by name with that
   byte. It is the same event name the sibling native sends: `ClientUnlockAchievement(AchievementId)`
   (NATIVE_UNLOCK_ACHIEVEMENT_IF_CONDITIONS_MET.md). The event's script is the controller's own and may do what it
   does (a client call).
4. **The answer:** **true** when at least one entry was selected, otherwise **false**.

**What it writes itself:** nothing but its own temporary list. Every other effect is the script event's.

## 3. Link 9's call, with the installed data

Maya's class definition (`GD_Siren.Character.CharClass_Siren`) holds **one** entry: type 2, a non-zero achievement,
threshold 100.

Link 9's call passes type 2, the impact count after its increment, and `bCanBeGreater` set. The entry is selected
only once the count reaches 100. A first lift (a count of 1 on a fresh stat) selects nothing, so the answer is
**false**, no event is sent, and nothing is written. The port's count is whatever its `PlayerStats` holds after the
script's increment; that value was not observed.

## 4. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.**
- **Bind in Off** the key `WillowPlayerController.UnlockPlayerClassAchievementIfConditionsMet(byte,int,bool,bool)`,
  keep-existing. Bind it where the sibling native is bound (`ow-inventory` `globals/achievement.rs`). Then bind it in
  the action-skill pack too, since link 9's world does not register the inventory pack (the same reuse as #623's
  `AttachComponent`), with one shared function.
- **Reference** keeps the stub policy and is unchanged.

**Preflight** (write-free, load-free, RNG-free; the first failure is the answer):
1. **The receiver** is resident. An unloaded handle is `NotRead`, "UnlockPlayerClassAchievementIfConditionsMet: the
   controller <path> is not loaded (a load is a write)".
2. **The arguments:** a byte, an int and two bools. Another shape is a `VmError::Script`.
3. **The class field:**
   - the receiver's class must declare `PlayerClass` as an object field (`VmError::Script` otherwise);
   - None, or an object whose class is not `WillowGame.PlayerClassDefinition` or a child of it: answer **false**, no
     write;
   - an unloaded handle: `NotRead`, "UnlockPlayerClassAchievementIfConditionsMet: the player class <path> is not loaded
     (a load is a write)".
4. **The array:** `PlayerClassAchievementData` must be an array of a struct with the three fields of section 2
   (`VmError::Script` otherwise).
5. **The selection** of section 2, read-only.
6. **Nothing selected:** answer **false**, nothing written.
7. **Something selected:** before the first event, the receiver's class must resolve `ClientUnlockAchievement` by name
   without a load. Reuse the sibling port's lookup and its refusals (`globals/achievement.rs`). Then call it once per
   kept achievement, in order, and answer **true**. A refusal from inside the event, after an earlier event has run,
   is marked "stopped after partial writes" through #518's counters, as the sibling port does.

**Tests** (synthetic classes built with `package()` / `tag()`, every value invented; each refusal with #518's three
counters and the name table's length unchanged):
1. link 9's shape: one entry of type 2 with threshold 100; the call (2, 1, true, false) answers false, no event, no
   write;
2. the same entry with a count of 100, and of 150: true, with one event carrying the entry's achievement;
3. `bCanBeLess` set with a count below the threshold: true. Neither flag set and a count different from the threshold:
   false;
4. entries passed over: an `Achievement` of 0, another type, a threshold of 0 or below;
5. two selected entries: two events in the array's order, and the answer true;
6. the class field None, and an object of another class: false, nothing written;
7. an unloaded receiver, and an unloaded player class: their refusals, write-free;
8. a malformed array or argument: `VmError::Script`, write-free;
9. registration: keep-existing in both packs, either order, one function; under `ParityMode::Reference` nothing new is
   bound.

## Open questions

1. **UNVERIFIED:**
   - the controller field's name (`PlayerClass`, by fit);
   - the class the cast tests (`PlayerClassDefinition`, by the array landing on its layout);
   - the event's name (shared with the sibling native, as recorded there).
2. **Not observed:** the port's impact count at link 9's call.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Hook this native during a Phaselock lift on a fresh character. Expect (2, 1, true, false) and a false answer.
2. On a character whose impact stat is at least 100, expect true and one `ClientUnlockAchievement` call.
