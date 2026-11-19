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
