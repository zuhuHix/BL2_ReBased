## 2026-11-19: Read the Willow globals on the pickup path as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-WILLOW-GLOBALS, analyst only (clean room). New record:
`docs/verification/NATIVE_WILLOW_GLOBALS.md`, with six `NATIVE_INDEX.md` rows.

- **Sources, in order:**
  - the records (NATIVE_CONTROLLER_HELPERS.md, NATIVE_PAWN_DATA.md, NATIVE_PICKUP.md);
  - the script (`research/script_disasm.py`, every `WillowPickup` function and the controller's pickup functions);
  - the installed packages (`tools/ghidra/class_layout.py`);
  - `Borderlands2.exe`, through two `tools/ghidra/run.ps1` read-only queries under the machine lock.
- Raw output stays under ignored `local/analysis/globals/`. No bl-sdk session was granted and no live evidence is
  recorded, so every rule is **UNVERIFIED**.
- **No code changes.** A fresh implementer codes it from the note alone.

**What the note settles.**

- Slice links 6 and 7 reach the globals first in the spawned pickup's `PostBeginPlay`, which calls
  `GetWillowGlobals().AddPickup(Self)`. That is inside the spawn, after the run's first write.
- **The two getters** read one engine-held pointer: the one globals object, or None with no engine object.
- **`AddPickup`** appends to `PickupList` with no duplicate check and no event.
- **`GetGlobalsDefinition`** answers `TheGlobalsDefinition`, loading the configured definition once when it is unset.
  That first load also numbers the challenge list, which is not read further.
- **`GetDropLifeSpanType`** answers the first rarity band holding the level, else 3.
- **`GetPopulationMaster`** is a field read. **`RemovePickup`** removes every entry of the pickup, keeping order.
- **Off answers:**
  - the world holds one globals object, checked in the drivers' preflight;
  - its absence is a `NotYet(LayerAbsent)`, never None, since None would skip `AddPickup` silently;
  - `TheGlobalsDefinition` is filled at world set-up, and the unread first load is a `NotYet(NotRead)`;
  - `RemovePickup`'s event dispatch stays a `NotYet(NotRead)` raised before its removal.
  - Every `NotYet` the run reaches here comes after the first write, so it is terminal under the hub's rule.

**Open (in the note).** Where the globals instance is created; what the definition's first load writes into the
challenge list; whether the headless world needs a population master; who listens to the two pickup events.

**Review r1 of #469 (five findings, fixed in the note).**
1. The preflight covers everything the route needs that can be known before the first write: the globals instance
   and its class, a `TheGlobalsDefinition` that world set-up wrote (the mission bridge's cached loader alone leaves
   the field unset), and the declared schemas of `PickupList`, `RarityLevelColors` and `ThePopulationMaster`.
   Preloading serves the pickup reads only; the challenge numbering stays unimplemented.
2. Typed outcomes: a missing layer and an unread rule are `VmError::NotYet` (kinds `LayerAbsent` and `NotRead`), a
   malformed declaration is `VmError::Script`. After the first write a `NotYet` keeps its variant, kind and record,
   gains the prefix and leaves the writes in place. Both getters answer `LayerAbsent` with no world globals object.
3. The destroy path: the headless fixture's population master is None, a non-None master reaches an unread native
   (`NotRead`), `RemovePickup` refuses before any dispatch or list change, and `OnAllPickupsDetached` fires only for
   the last pickup on its `Base` (NATIVE_PICKUP.md now points there).
4. The live checks tell the rules apart: a cold, before, after and repeated capture of the definition's load; event
   hooks that sample the list in both same-holder cases; `AddPickup(None)`; band boundaries and overlaps; the
   population master's field and result identity, None and non-None. What cannot be exercised live stays open.
5. The `make` sequence carries the timer's condition (a span above 0) and timing (`PickupShrinkDuration` before the
   end).
6. Where `TheGlobalsDefinition` lives (asked by the -94 implementer): an ordinary script field, an object reference
   typed `GearboxGlobalsDefinition`, declared on `GearboxFramework.GearboxGlobals` and inherited by `WillowGlobals`,
   read from the installed export table. `DefaultGlobalsDefinitionName` is only the first load's configured name. Off
   writes the definition into the inherited field at world set-up, and the preflight checks its schema (note 6.0).
7. Review r2 of #469: the preflight requires the declared reference targets (`PickupList` of `WillowPickup`,
   `TheGlobalsDefinition` of `GearboxGlobalsDefinition`) before any value is read; the cold-load check has a pass
   condition (empty, then stored and returned as one object, then the challenge writes once named, else UNVERIFIED);
   the timer delay is `LifeSpan - PickupShrinkDuration`, a delay of 0 or below UNVERIFIED; a mission item takes kind 3
   without the getters.
