## 2026-11-19: The spawn attaches a new actor's components, which sets their `Owner`

AI-assisted (Claude Code). Analyst lane ANALYST-SPAWN-COMPONENT-OWNER (link 9). Documentation only: no code changed,
and the lane implements none of it (clean room). **Every rule is UNVERIFIED in the running game.**

- **The note:** `docs/verification/NATIVE_SPAWN_COMPONENT_ATTACH.md`, with a `NATIVE_INDEX.md` row.
  NATIVE_ENGINE_CORE.md's spawn routine gets a pointer for the step it did not list. The note builds on
  NATIVE_ATTACH_MESH_TO_PICKUP.md section 3.4 and its empty-scene ruling without changing them.
- **What was read** (local Ghidra, read-only, under the machine lock; raw output under ignored `local/analysis/`):
  - right after a new actor's `Location` and `Rotation`, and before `PhysicsVolume`, its owner and `Instigator`, the
    spawn calls a full component update. It runs on every spawn, begun play or not;
  - the update walks the actor's `Components` and runs each entry's conditional attach. An unattached component gets
    `bNeedsReattach` and `bNeedsUpdateTransform` cleared, `Scene` and `Owner` set, its transform, and its own attach
    entry;
  - for `AIComponent` and `WillowAIComponent` that entry sets `bAttached` and appends the component to its owner's
    `AllComponents`. Nothing is loaded or drawn;
  - one body serves `Actor`, `Controller`, `GearboxMind` and `WillowMind`.
- **Proposed Off answer for a separate implementer:** run the step in `ow-world`'s spawn after `Location` and
  `Rotation`, with a write-free and load-free preflight and the empty scene layer's answers. The layer moves to a
  crate the spawn can see. For the hub: a world without the layer refuses with LayerAbsent (recommended), or the step
  is skipped there. Reference is unchanged. The tests are in the note's section 4.
- **Local probe (never pushed):** the link 9 stack with #524 and a hand-merged #523, plus probe-only stubs of #589,
  #594, #600 and this step. Installed Off strict `--slice-gate`: 3 ok / 8 NotYet / 3 errors. The mind's
  `WillowAIComponent` got the mind as `Owner`. Link 9 ran both steps and is an error at the unbound
  `AttributeDefinitionBase.ResolveContext(object)`, the next stop.
- **Live check (bl-sdk; not run, no session granted):** the note's last section.
