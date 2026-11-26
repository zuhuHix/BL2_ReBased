## 2026-11-26: The swing's tail past GetBoundingCylinder in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-BOUNDING-CYLINDER-AND-AFTER, from NATIVE_BOUNDING_CYLINDER_AND_AFTER.md
as of #799's r1 fix fc8d2c42, which carries the hub's #799 review changes. The lane implemented from that note alone and
did not read the executable. The branch is stacked on fix/rust-links-13-14 @ e00e3de. **Every rule is UNVERIFIED in the
running game.** Reference is unchanged.

- **What was decided (Off only):**
  - **the translated bounds** (`ow-world` `skeletal_bounds`, `bounding_cylinder`):
    - a non-zero `Translation` is accepted only when all of these hold: no Absolute flag set, the component's `Owner`
      is the receiver, both rotations zero, the owner's `PrePivot` zero, and the combined scale exactly 1;
    - then the skeletal origin is `Location` + `Translation` + the mesh origin + `RootBoneTranslation`, and the
      cylinder's is `Location` + `Translation`;
    - every other case keeps the `WORLD_TRANSFORM` `NotRead`;
  - **`Actor.TraceComponent`** (`ow-combat` `bcyl_tail`):
    - no component, or `bAttached` clear: false;
    - a component the attach record does not know: refusal L;
    - one not attached to the receiver: false;
    - an attached one: the line check's `LayerAbsent`;
    - **the hub's ruling on #799:** under the Off-only `TraceComponentMiss` setting, an attached component gets a
      logged miss instead (false, no out written). The slice's combat world turns the setting on and logs a set-up
      line;
  - **`TryFullBodyGib`:** the early answers; a dead pawn is the gib test's `NotRead`, and with `bDidServerGibPawn`
    the gib's `NotRead`;
  - **`WillowPawn.GetParent` and `WillowAIPawn.GetParent`:** they leave `go_down`'s unresolved list. The order is
    `SpawnParent`, then the body flag's pawn `Base`, then the pawn rule: a set `DrivenVehicle` is `NotRead`, else a
    `WillowPawn` `Owner`;
  - **the shield getters:** through `ShieldArmor`'s valid pool, else the two ints as floats;
  - **`ShakeView`:** on a `WillowPlayerController`, the struct and three lengths are written, with the declarations
    checked before the first write; any other receiver is `NotRead`;
  - **`DoForceFeedbackWaveform`:** the gates as read. With a manager it is `NotRead`, decided before the `Scale`
    write;
  - **`GetCombatMusicManager`:** the field. A None field is the lazy creation's `NotRead`; the creation itself is
    left to #801;
  - **declarations:** every field the note lists must be declared with its kind, else `VmError::Script`.
- **The slice gate** (installed, Off, strict, release build, under the machine lock): 7 ok / 14, X 58.3%, every row as
  on the base. Links 3, 4, 5, 13 and 14 stop at LINK34's base-search floor trace, before the swing reaches this code.
- **Tests:**
  - `bcyl_tail` (8): `TraceComponent` with and without the setting, `TryFullBodyGib`, `GetParent`, the shields,
    `ShakeView`, the force-feedback gates, the music manager, and the declarations;
  - `ow-world` (2): the skeletal translated case with unit scale, scale 2 and a rotation; the cylinder with a
    translation, plus `PrePivot`, an Absolute flag and another `Owner`.
- **Review of #804 (fix round):**
  - **the hub's ruling:** `TraceComponent` on a component the record holds under another owner, or a stale one, is
    refusal L, as `GetBoundingCylinder` answers it, instead of false. This replaces the note's "known but not attached
    to the receiver: false" for the port. It is also refusal L with the miss setting on, and nothing is logged;
  - the shield getters check that `ShieldArmor`'s manager is resident before the resolve reads it;
  - the slice gate's trace-miss set-up line sits above the combat group's doc comment again, single-spaced;
  - the tests cover both new refusals and the unloaded manager;
  - #799's index rows were filled during the rebase onto main.
- **Sensitive files:** none.
