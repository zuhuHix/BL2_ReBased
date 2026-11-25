## 2026-11-25: Cold class loads off the write-free paths of ow-inventory and ow-world

AI-assisted (Claude Code, Claude Opus 5.5). Lane COLD-LOAD-FIXES, on `fix/rust-cold-class-audit-world-inv` (#701,
d253ffe2). It fixes the five findings #701 pinned with ignored tests, plus the case it listed as not reached. No
game data and no native reading; synthetic fixtures only. No rule of a native changes where its classes are warm.
**Nothing here is verified in the running game.** Reference is unchanged.

**The pattern** (as #686 and #691): a write-free path asks only for what is resident. That means:
- the class table by path, where a class's identity or chain is all the answer needs;
- `Vm::find_loaded_method`, where a method must be resolved;
- the code index by key, where `implements` is asked.

A cold class is a named `NotYet { NotRead }` ("the class <path>, not loaded (a class load is a write)") only where the
answer needs its code.

**The fixes, one commit each:**
1. **`WillowPlayerController.GetInventoryPawn`:** the pawn's class is compared with `WillowPlayerPawn` through
   `by_path`, not `find_class`. A table without that path answers the existing refusal, no longer `find_class`'s error.
2. **`DroppedPickup.GetPickupableInventoryDefinition`:** `find_loaded_method` replaces `find_method`. A class of the
   inventory's chain with its code not loaded is the named `NotRead` (record NATIVE_INVENTORY_EQUIP.md), before the
   dispatch check. **The pinned test's expected answer changes:** #701 expected the warm sibling's `UnboundNative`, but
   that answer needs the cold class's code, so the test now expects the refusal.
3. **`UnlockAchievementIfConditionsMet`:** step 1 uses a new `globals::globals_definition_resident`, which takes
   `GearboxGlobalsDefinition` and `GlobalsDefinition` by path. Only a declared type and `is_child_of` use them, which
   are layout questions. A class the table does not hold cannot match, so the refusals are the same. One text differs:
   a missing `GearboxGlobalsDefinition` was `find_class`'s error and is now the declaration data error.
4. **`DataStoreClient` getter (`GetDataStoreClient`):** before it takes its classes with `find_class`, a read-only
   check asks the code index for each one by path: the global stores and the client, then the settings provider class
   where a store derives from the settings store. A missing one is `find_class`'s own error for that path, which loads
   nothing. **Only this part is fixed.** The plan's other checks need the classes' layouts, and the getter makes
   objects on success, so they still follow `find_class`'s code load. Making the whole plan load-free would need
   layout-only resolution (`define_layout`, which interns names) or a refusal on cold store classes. Several existing
   tests build the client on a cold world, so that is a ruling for the hub or the maintainer.
5. **`LoadPlayerPawnDataAsync`:** the `IStreamingDataEvent` check takes the interface's code-index key
   (`CodeIndex::find_class`) and asks `CodeIndex::implements` by key, the comparison `Vm::implements` makes.
6. **pickup targeting's `suspended`:** `WillowPlayerPawn` is compared by path, and the owner's state name is compared
   by its text (without case) instead of interning "Dead" on each pass, the audit's side finding. A second fixture
   leaves `WillowPlayerPawn` cold (`fx_with(false)`), the case #701 listed as not reached.

**Tests.** #701's five pinned tests are un-ignored and pass (finding 2 with the changed answer above). The warm-pawn
targeting test now also compares the name table, and one new test covers the cold `WillowPlayerPawn`.

**Open:**
- the data-store plan's load-before-check (fix 4);
- #701's "not tested" sites;
- GetParticleEffect's ignored finding, which is another lane's.
