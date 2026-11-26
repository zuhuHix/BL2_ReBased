## 2026-11-26: InitializeBehaviorProviders and the material natives in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-INITIALIZE-BEHAVIOR-PROVIDERS, on main 4c01c9a1. Clean room: implemented
from docs/verification/NATIVE_INITIALIZE_BEHAVIOR_PROVIDERS.md alone (#740 with its review r1, decision 2026-11-25-75);
this lane did not read the executable. No session was run. Every rule is UNVERIFIED in the running game.

- **What was decided:** `WillowGame.WillowPawn.InitializeBehaviorProviders` is bound by the kernel pack in Off
  (`ow_behavior::kernel::natives::pawn_providers`). Reference keeps the stub policy.
  - **Only while `ConsumerHandle.PID` is exactly -1.** Any other value: nothing, and no write.
  - **The preflight** reads only: no write, no store load, no class-code load, nothing interned. Its order:
    1. a `WillowAIPawn` receiver is a typed `NotRead` (the AI body);
    2. the four declarations, else `VmError::Script`;
    3. the `PID` gate;
    4. no kernel service is the pack's `LayerAbsent`;
    5. a pawn the kernel already holds is `RegisterBehaviorConsumer`'s `NotRead`;
    6. the list: (a) the default hit region, only when `HitRegionList` holds it, (b) the hit regions and their
       override death definitions, (c) the stances, (d) the instance data's `BodyHitRegionDefinition` hit regions,
       by name, (e) `DeathDef`, (f) `PlayerEventDef`. An object not resident is a typed `NotRead`. The kernel's
       gate then drops None and empty definitions;
    7. per definition in list order, the repeat (by stock path, as the kernel rule compares) and then the condition
       (a plain read of `CustomEnableCondition`), with the kernel rule's own texts and kinds;
    8. `bNeedToApplyThisState` is a typed `NotRead`.
  - **The commit:** register by `RegisterBehaviorConsumer`'s allocator (owner `Kernel`), write `ConsumerHandle`, then
    the kernel rule per definition. The decode's loads come after the registration write. A refusal after the
    registration carries the partial-writes prefix by #518's counters; none before it does.
  - **The script-facing `IntializeBehaviorProviderForConsumer` is unchanged.** Its body moved into a shared helper
    with the same steps, and its refusal texts are now shared functions. Note section 5's two differences (a None
    provider, an empty definition) stay as they were.
- **The material natives** are bound in Off, keep-existing (`ow_inventory::equip::material_instances`); Reference binds
  none. "First native class" is `material_natives::first_native`, and the class tests are by name, so none loads class
  code.
  - `MeshComponent.GetMaterial` on `SkeletalMeshComponent` and `GearboxSkeletalMeshComponent`: the override entry,
    else the mesh's entry, else None. Any other class is a typed `NotRead`, and so is an unloaded mesh. A negative
    index is None after the declarations.
  - `MaterialInstance.SetParent` on a `MaterialInstanceConstant` writes only a different parent.
  - `MaterialInstance.SetVectorParameterValue` on a `MaterialInstanceConstant`: the first entry of that name gets the
    four floats when any differs (an exact compare, so a NaN always writes); with no such entry, one is appended with
    a zero `ExpressionGUID`.
  - `MeshComponent.GetInstancedMaterialInstanceConstant`: an instance the component owns is the answer, with no write.
    Otherwise the preflight checks the class, then `SetParent`'s declarations, then `SetMaterial`'s preflight
    (`REATTACH_OWNER`, `REATTACH_SCENE`). Only then is the instance made as a script `new` makes it, with the
    component as `Outer`; then `SetParent` and `SetMaterial`. A refusal after it is made carries the prefix.
  - `SetMaterial`'s plan and commit (#705) are shared with it, and its behaviour is unchanged.
- **Fitted, not read:** the stance's provider definition (its `BehaviorProviderDefinition` field) and the hit-region
  membership test (medium) are by fit, as the note says. The made instance's flags were not decoded. The repeat is
  compared by stock path, as the kernel rule does.
- **Evidence:** the note's tests on invented data: 19 for the pawn native (the note's 16, plus preflight 5, a missing
  declaration and Reference) and 13 for the material natives (3.1-3.5, the arms' two instances, the keys).
  `cargo test --workspace -j 8`, `crate_edges_test.py` and the installed slice gate are in GATES.md.
- **Sensitive files:** none.
- **Verified:** nothing in the running game.
