# Maya's bounding cylinder, the hit-info trace, and the swing's tail (links 4, 13, 14) (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-GETBOUNDINGCYLINDER-AND-AFTER (decision 2026-11-26-56), the analyst
half of the clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/bcyl/`.
This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every
rule here is UNVERIFIED in the running game.**

**Why.** NATIVE_LINKS_13_14.md (#796) puts the next stop of links 4 and 13's swings at `Actor.GetBoundingCylinder`'s
"the component's world transform" `NotRead`, reached from `WillowPawn.PlayHit` through `Actor.CheckHitInfo`.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`): `Actor.CheckHitInfo`, `WillowPawn.PlayHit`,
  `WillowPlayerController.ClientDamageShake`.
- **The natives:** Ghidra 12.1.4, headless and read-only, under the machine lock. Read: the execs named below and their
  class-table slots (the NATIVE_COMPONENT_SKIP_FLAG.md section 6 method), in the primary tables of:
  - `WillowPawn`, `WillowAIPawn`, `WillowPlayerPawn`;
  - `PlayerController`, `WillowPlayerController`;
  - the pawns' damageable-interface table.
- **Field places:** the local copy of `tools/ghidra/class_layout.py`. Every field lands exactly where the routine reads
  it, except one `WillowAIPawn` field, which is one field after the tool's place. That is the layout gap
  NATIVE_MIND_ROUTE.md records; it is marked "by fit".
- **`ow-package --def-dump`:** class defaults.
- **A local probe** (temporary code, reverted, never committed): an installed Off `--combat-run` on main 54c9cf05,
  under the machine lock, with a call trace and read-only prints. It ran the gate's steps for links 1, 3, 4, 13 and
  14, with every earlier stop of #788, #793 and #796 stubbed.
  - **The class-modifier stand-ins:** the resistance read was stubbed at 1.0 in place of #796's ruled inputs. For Maya
    that is the same value: her class's twelve modifiers are 1 (NATIVE_LEVEL_UP_ATTRIBUTES.md).
  - Each stop below was then stubbed in turn, by the rule read here.

## 2. `GetBoundingCylinder` on Maya: the skeletal mesh component's world transform

Maya's pawn is live, so the pawn rule falls through to the actor default: the union of her counted components'
`Bounds` boxes (NATIVE_GET_BOUNDING_CYLINDER.md section 2). The stop is not the cylinder: her `CylinderComponent` (R 42,
H 80, `Translation` zero) passes `entry_box`. **It is her `SkeletalMeshComponent`.** Its stored-bounds path
(`skeletal_bounds.rs`, `simple_transform`) refuses with the bounding cylinder's `WORLD_TRANSFORM` text.

**What the probe read at the call:**

| Field | Value |
|---|---|
| the component's `Translation` | (0, 0, -86) |
| the three Absolute flags | clear |
| the component's `Owner` | the pawn |
| the component's and the pawn's `Rotation` | zero |
| the pawn's `PrePivot` | zero |
| the combined scale | exactly 1 on every axis |

So the only failing condition is the non-zero `Translation`.

**The reading** (NATIVE_COMPONENT_BOUNDS.md section 3, the transform update): `LocalToWorld` is built from the
component's `Translation`, `Rotation` and scale, then the owner's matrix. With both rotations zero and every scale
exactly 1, each matrix is a pure translation. The component's world position is then the owner's `Location` plus the
component's `Translation`, **whatever order the two matrices are composed in**.

**Proposed Off answer** (an extension of the simplest world transform, for `stored_box` and the cylinder rule alike):
- a non-zero `Translation` is accepted **only when all of these hold** (review r1 of #799):
  - the inherited conditions: none of the three Absolute flags set, and the component's `Owner` is the receiver;
  - both rotations (the component's and the owner's) zero;
  - the owner's `PrePivot` zero (UE3 also moves an actor's matrix by its pre-pivot; the probe read it as 0);
  - the combined scale exactly 1 on every axis (a stricter case than the uniform-scale rule);
- then the skeletal mesh's origin is `Location` + `Translation` + the mesh origin + `RootBoneTranslation`, and the
  cylinder's origin is `Location` + `Translation`; the extents are unchanged;
- any condition that fails is the `WORLD_TRANSFORM` `NotRead`;
- any other non-zero `Translation` stays the `WORLD_TRANSFORM` `NotRead`. A scaled or rotated case needs the
  composition order, which is not settled here;
- write-free, load-free, as now. UNVERIFIED in game.

**On the slice:** Maya's two counted boxes come out as:
- the cylinder: centre (0, 0, 0), extent (42, 42, 80);
- the mesh: centre about (-3.0, 0.8, 20.9), extent about (29.0, 107.8, 115.1).

Their union has half extents of about (42, 107.8, 115.1). So the answer is radius about 115.7 and height about 115.1.

## 3. `Actor.CheckHitInfo` and `Actor.TraceComponent`

**The script, `Actor.CheckHitInfo(HitInfo, FallBackComponent, Dir, out HitLocation)`:**
- a hit whose component is a `SkeletalMeshComponent` with a bone name already set: nothing;
- a hit with no component, or (in a condition read only in part) one that is not that mesh: the
  `FallBackComponent` is used;
- when the component is a skeletal mesh with no bone name (the melee swing's case), the hit is repaired by tracing that
  one component:
  - the direction is `Dir`, or the actor's facing when `Dir` is zero;
  - the hit location is the given one, or the actor's `Location` when it is zero;
  - the trace runs along the direction through the hit location, from one side to the other, with a half length of
    max(128, half the sum of the bounding cylinder's height and radius);
  - on a hit, the bone name, physical material and location are taken from it.

**`Actor.TraceComponent(out HitLocation, out HitNormal, Component, TraceEnd, TraceStart, Extent, out HitInfo, bComplexCollision)`:**
- **no component, or one whose `bAttached` is clear:** false, with the outs untouched;
- **otherwise:** the component's own line check (a primitive class virtual) with the trace flags 0x20df, or 0x220df
  when `bComplexCollision` is set (8415, or 139487; their meaning by fit, not decoded). On a hit it answers true and fills:
  - the hit location and normal;
  - `HitInfo`: the material, the physical material (from the hit's physical material, else its material's), the item,
    the level index, the bone name and the hit component.

**Proposed Off answer:**
- the attachment through #605's attach record. A component the record does not know is the bounding cylinder's
  refusal L;
- a component that is not attached: false, nothing written;
- an attached one: `LayerAbsent`, "Actor.TraceComponent: the component's line check", layer "collision (a primitive
  component's own geometry)". The port holds no per-component collision geometry, and for a skeletal mesh the line
  check needs its physics bodies posed by the bones;
- every field must be declared with its kind, else `VmError::Script`;
- write-free until the outs, which are written only on a hit.

**On the slice:** Maya's mesh component is attached by the record. So this is the `LayerAbsent`, and it is links 4 and
13's next stop.

**Hub ruling (review of #799): a miss stand-in, behind a setting.** Under an Off-only slice harness setting (the same
pattern as the base-search stand-in, the hub's ruling on review of #798), `TraceComponent` on an attached component
answers a miss: false, with no out written, so `CheckHitInfo` keeps the hit as it was. Each use is logged as a harness
input, and the answer is UNVERIFIED. Without the setting it stays the `LayerAbsent` above. Reference is unchanged. The
probe used the miss.

## 4. `WillowPawn.TryFullBodyGib(Damage, Controller, DamageType, ..., ..., ...)`

`WillowPawn.PlayHit` passes the pawn's `RecentDamage.HealthDamage`, the instigator's `Controller` and the damage type.
- **already gibbed:** with `bGibbed` set, or no `MyDeathDef`, the answer is `bGibbed`, with nothing done;
- **the conditions,** unless `bDidServerGibPawn` is set:
  - the pawn's health must be 0 or below. A live pawn answers `bGibbed` at once;
  - the death definition's gib test must pass. It takes the damage over the pawn's maximum health (1 when the maximum is
    not above 0) and the damage arguments;
  - `DeathTime` must equal `WorldInfo.TimeSeconds` within 0.0001, that is, the pawn died this frame;
- **then the gib:** a pawn virtual with the controller, and a script event. The answer is `bGibbed`.

**Proposed Off answer:**
- the early answers as read;
- a dead pawn that reaches the gib test: `NotRead`, "TryFullBodyGib: the death definition's gib test";
- with `bDidServerGibPawn` set, the gib itself: `NotRead`;
- every field must be declared with its kind, else `VmError::Script`; write-free up to the gib.

**On the slice:** Maya is alive, not gibbed, and has a `MyDeathDef`, so the answer is false.

## 5. `WillowPawn.GetParent()` and `WillowAIPawn.GetParent()` (resolved)

NATIVE_GO_DOWN_PATH.md 3.2 left these as "the class's own implementation is not resolved". With the class-table method
they are now read:
- **`WillowPawn` and `WillowPlayerPawn`** (one body):
  - the `DrivenVehicle`, when it is set and passes two queries of its own (not named);
  - else the `Owner` when it is a `WillowPawn`;
  - else None;
- **`WillowAIPawn`:**
  - its `SpawnParent` (by fit, across the layout gap), when set;
  - else its `Base`, when its `BodyClass` has `bOnlyDisplayParentInfoWhileAttached` set and the `Base` is a
    `WillowPawn`;
  - else the `WillowPawn` rule.

**Proposed Off answer:**
- the two keys leave `go_down.rs`'s unresolved list and take this rule;
- a set `DrivenVehicle` is `NotRead`, "GetParent: a driven vehicle's two queries";
- declared, else `VmError::Script` (review r1 of #799): `DrivenVehicle`, `Owner`, `Base` (objects), `SpawnParent` (an
  object on `WillowAIPawn`), `BodyClass` (an object) and its `bOnlyDisplayParentInfoWhileAttached` (a bool);
- every object read must be resident, else `NotRead`; write-free.

**On the slice:** the adult, the baby and Maya have no vehicle, no owner, no spawn parent and no base, so the answer is
None. This also clears link 3's kill stop (#796 section 7).

## 6. `WillowPawn.GetMaxShieldStrength()` and `GetShieldStrength()`

Both go through the pawn's damageable interface. One body serves `WillowAIPawn` and `WillowPlayerPawn`:
- **the maximum:** when `ShieldArmor` resolves to a valid pool, that pool's maximum value; else `ShieldMaxVar` (an
  int) as a float;
- **the current:** when it resolves, the pool's current value; else `ShieldVar` as a float.

These are the shield reads NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md section 2 already uses.

**Proposed Off answer:** that rule, through the port's pool-handle rule (NATIVE_COMBAT_GROUNDWORK.md) with the effective
maximum. Write-free. Declared, else `VmError::Script` (review r1 of #799): `ShieldArmor` (a `ResourcePoolReference`),
`ShieldMaxVar` and `ShieldVar` (ints). **On the slice:** Maya's `ShieldArmor` names a pool.

## 7. `WillowPlayerController.ShakeView(Info)` and the rest of the swing's tail

- **`ShakeView`** (`ClientDamageShake`'s camera shake; one exec shared with `GearboxPlayerController`, which calls the
  controller's virtual). `WillowPlayerController`'s body:
  - `CurrentViewShake` := Info;
  - then `ShakeOffsetLength`, `ShakeRotLength` and `ShakeFOVLength` := Info's `OffsetTime`, `RotTime` and `FOVTime`.

  **Off:** those writes. `Info` is a `GearboxViewShakeInfo`, and `CurrentViewShake` and the three lengths must be
  declared, else `VmError::Script`, checked before the first write (review r1 of #799);
- **`Controller.InGodMode()`:** #793 section 5's rule, the controller key (not yet bound in the port);
- **`PlayerController.DoForceFeedbackWaveform(Waveform, ..., Scale)`** (`Scale` is 1 when omitted): one body for
  `PlayerController` and `WillowPlayerController`:
  - nothing when the controller's `PlayerInput` is set and its `bUsingGamepad` is clear;
  - nothing when `ForceFeedbackManager` is None, or `PlayerReplicationInfo` is None;
  - otherwise the waveform's `Scale` := `Scale`, and the manager's play virtual runs (not read).

  **Off:** the gates as read. With a manager, the `NotRead` "DoForceFeedbackWaveform: the manager's play", decided
  **before** the waveform's `Scale` write, so nothing is written. Declared, else `VmError::Script` (review r1 of #799):
  `PlayerInput`, its `bUsingGamepad`, `ForceFeedbackManager`, `PlayerReplicationInfo`, and the waveform's `Scale`.
  **On the slice:** `ForceFeedbackManager` is None in the port (also its class default), so nothing happens;
- **`WillowGameInfo.GetCombatMusicManager()`:** it answers `GlobalCombatMusicManager`. When that is None, it first makes
  a new manager on the game info and initialises it, a lazy creation that was not read further. **Off:** the field when
  set; when None, `NotRead`, "the combat music manager's lazy creation", as `GetVehicleLifetimeManager`'s answer in
  NATIVE_GO_DOWN_PATH.md 4.2. `GlobalCombatMusicManager` must be declared as an object, else `VmError::Script` (review
  r1 of #799). **On the slice:** it is None, so this is the stop.

## 8. Predicted stops

**Links 4 and 13**, past #796's stops, on each swing:
1. `GetBoundingCylinder`'s world transform (section 2), until the extension is in;
2. `TraceComponent`'s `LayerAbsent` (section 3); with the hub's harness setting on, the logged miss instead;
3. past sections 4 to 7: **`WillowGameInfo.GetCombatMusicManager`'s lazy-creation `NotRead`.** The next read.

**Link 14:** it needs a down. With the stubs, the swings still took about 13.9 health each (Maya at 156.9 after four),
so there is no down and `bleed:50` does nothing (#796 section 7, the hub's no-cap ruling).

**Link 3** (by the way): past `GetParent`, the kill stops at an unbound
`WillowPlayerController.NotifyKilledEnemy(string)` (`WillowPawn.TookDamageFromEnemy`).

## 9. Tests (synthetic fixtures)

1. **Section 2:**
   - a toy pawn with a skeletal component, `Translation` (0, 0, -86), zero rotations, scale 1: the box moved by the
     translation;
   - the same with a scale of 2 or a rotation: the `WORLD_TRANSFORM` `NotRead`;
   - a cylinder component with a translation under the same conditions.
2. **`TraceComponent`:** no component: false; `bAttached` clear: false and outs unchanged; attached: the `LayerAbsent`;
   unknown to the record: refusal L. With the hub's harness setting on (hub ruling): an attached component gives
   false, the outs unchanged and one harness-input log line; the unattached and unknown cases answer as without it;
   Reference with the setting on is unchanged.
3. **`TryFullBodyGib`:**
   - gibbed: true;
   - no `MyDeathDef`: false;
   - alive: false;
   - dead: the `NotRead`;
   - `bDidServerGibPawn`: the `NotRead`.
4. **`GetParent`:**
   - `WillowPawn` with a `WillowPawn` owner: the owner;
   - a non-pawn owner: None;
   - a `WillowAIPawn` with a `SpawnParent`: it;
   - the body flag with a pawn `Base`: the base;
   - a driven vehicle: the `NotRead`.
5. **The shield getters:** a valid pool: its maximum and current; an invalid reference: the two ints as floats.
6. **`ShakeView`:** the struct copied and the three lengths set.
7. **`DoForceFeedbackWaveform`:** each gate gives nothing; with a manager: the `NotRead`, nothing written.
8. **`GetCombatMusicManager`:** set: the field; None: the `NotRead`, nothing written.
9. Write-free where stated; Reference: unchanged.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:**
   - the component line check;
   - the death definition's gib test and the gib;
   - the vehicle's two queries in `GetParent`;
   - the force-feedback manager's play;
   - the combat music manager's creation;
   - `NotifyKilledEnemy`.
3. **By fit:** `SpawnParent`'s place; the trace flags' meaning.
4. **Ruled by the hub on #799:** `TraceComponent`'s miss stand-in, behind an Off-only harness setting (section 3).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Read Maya's `GetBoundingCylinder` answer and her mesh component's `Bounds` right after spawn.
2. After an enemy melee hit, read the hit info's bone name and the player controller's `CurrentViewShake` and
   `ShakeOffsetLength`.
3. Read `GlobalCombatMusicManager` before and after the first hit of a fight.
