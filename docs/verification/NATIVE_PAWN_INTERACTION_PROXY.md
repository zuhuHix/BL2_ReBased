# The player's trade interaction proxy: SetUpTradeInteraction, SetCollisionSize, SetHardAttach and SetBase (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-PAWN-INTERACTION-PROXY (queue 0aq, decision 2026-11-26-60), the
analyst half of the clean room: **another lane implements this note.** Raw output stays under ignored
`local/analysis/proxy/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. **Every rule here is UNVERIFIED in the running game.**

**Why.** Lane E's player-side input from #796 (the controller's `Pawn` set at the player's set-up) is deferred. With
the `Pawn` set, `WillowPlayerPawn.SetUpTradeInteraction` spawns a `PawnInteractionProxy`. Its `Initialize` calls
`SetCollisionSize`, `SetHardAttach` and `SetBase` on the proxy, and all three refuse today, so link 1 would regress.
This note reads them so the flag can turn on.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`):
  - `WillowPlayerPawn.SetUpTradeInteraction` and `DestroyTradeInteraction`;
  - `PawnInteractionProxy.Initialize` and `InteractionProxy.Initialize`;
  - the `Attach`, `BaseChange` and `CollisionChanged` events of every class on the path.
- **The natives:** Ghidra 12.1.4, headless and read-only, under the machine lock. Read:
  - the execs of `Actor.SetCollisionSize`, `SetHardAttach` and `SetBase`;
  - the `CylinderComponent` size setter;
  - the slots `SetHardAttach` and `SetBase` call, in the primary tables of `PawnInteractionProxy` and `Actor`, and in
    review r1 of #802 also of `InteractionProxy` and `Trigger` (the NATIVE_COMPONENT_SKIP_FLAG.md section 6 method).
    All four tables hold the same `SetHardAttach` body, `Actor`'s, and the same `SetBase` body, `Actor`'s.
- **Field places:** the local copy of `tools/ghidra/class_layout.py`. The actor fields land exactly. The cylinder's two
  floats sit past the tool's places by a constant shift, in the declared order; they are named by fit (review r1 of
  #802).
- **`ow-package --def-dump`:** `Default__PawnInteractionProxy`, Maya's `CharClass_Siren` and its trade definition.
- **A local probe** (temporary code, reverted, never committed): an installed Off `--combat-run` of `maya:8` on main
  03a8a09, under the machine lock, with a call trace.
  - It emulated lane E's flag: the controller's `Pawn` set before `ApplyCharacterClassDefaults`, and left set;
  - it stubbed each stop below in turn by the rule read here.

## 2. The script path

**`WillowPlayerPawn.SetUpTradeInteraction`:**
1. `DestroyTradeInteraction`: an existing `TradeInteractionProxy` is destroyed and the field cleared;
2. **the gates:** the pawn's `Controller` must be a `WillowPlayerController` with a `PlayerClass` whose
   `TradeInteractionDefinition` is set; otherwise it returns;
3. `TradeInteractionProxy` := `Spawn(PawnInteractionProxy)`, owned by the pawn, at the pawn's `Location` and
   `Rotation`;
4. `proxy.Initialize(TradeInteractionDefinition, the pawn)`;
5. the proxy's `bDisableDuringTrade` and `bDisableDuringVehicleUse` := true.

**`PawnInteractionProxy.Initialize(Definition, Instigator)`:**
1. `InteractionProxy.Initialize` with the same arguments:
   - with both arguments set: `Definition` := the definition as a `WillowPawnInteractionDefinition`;
   - with that set, and its `bSupportsTouchEvents` or `bUseInteractDistance` set:
     `SetCollisionSize(TouchRadius, TouchHeight)`;
   - when `IsInteractionDebugEnabled()` answers true (false in Off, NATIVE_GO_DOWN_PATH.md 4.2): `SetHidden(false)`;
2. then, with `Definition` set: `Instigator` := the instigator as a pawn, `SetHardAttach(true)`, and
   `SetBase(the instigator)`.

**On the slice:**
- Maya's class `GD_Siren.Character.CharClass_Siren` names `GD_PlayerShared.Trade.PlayerTradeInteractionDefinition`, a
  `WillowPawnInteractionDefinition` with `bSupportsTouchEvents` true, `TouchRadius` 80 and `TouchHeight` 78;
- so the proxy gets `SetCollisionSize(80, 78)`, `SetHardAttach(true)` and `SetBase(Maya)`;
- **the proxy's class defaults:** its `CollisionComponent` is its `CylinderComponent` (`CollisionCylinder`),
  `bCollideActors` true, `bBlockActors` false, `bHardAttach` true, `bHidden` true, `Role` authority;
- the probe's spawn passed.

## 3. `Actor.SetCollisionSize(Radius, Height)`

Not a class virtual: one body for every class. In order:
1. **with a `CollisionComponent` that is a `CylinderComponent`:** the cylinder's size setter. It writes
   `CollisionHeight` := Height, then `CollisionRadius` := Radius (by fit), then asks for the deferred reattach
   (NATIVE_COMPONENT_BOUNDS.md section 3, item 2). With an owner, that sets `bNeedsReattach` and leaves the reattach to
   the owner's component update;
2. **the touch update** (by fit): it takes the actor's collision bounds, walks the actors near them and tests each for
   overlap. By fit it begins and ends touches. It is a collision-world query, and was outlined, not read to the end;
3. **a script event** with no argument, by fit `CollisionChanged`: `Actor`'s body is empty and no class on the path
   overrides it;
4. **`bNetDirty`** := true.

This replaces NATIVE_GO_DOWN_PATH.md 4.1's outline, whose two unread steps are 2 and 3.

**Proposed Off answer** (replacing `go_down.rs`' `NotRead`, "the component's world update"):
- **preflight, write-free and load-free:**
  - declared, else `VmError::Script`: `CollisionComponent` (an object); on a cylinder, its `CollisionHeight`,
    `CollisionRadius` and `bNeedsReattach`; `bNetDirty`;
  - the deferred reattach's own preflight, through the existing planner (`component_setters`);
  - the event resolved on the receiver's class (state-aware, load-free, as `SetPhysics`' events are). A name not in
    the name table means no event; nothing is interned (review r1 of #802);
  - **the touch update:** with no collision service in the world, `LayerAbsent`, "Actor.SetCollisionSize: the touch
    update", layer "collision (the world has no collision service)"; with one, `NotRead`, "Actor.SetCollisionSize: the
    touch update (its overlap rule is not read)". Both are decided here, before any write, so neither carries the
    prefix;
- **commit:** the two cylinder writes, the reattach, the event, then `bNetDirty`. A refusal after the first write
  carries the prefix;
- **hub ruling (review of #802): a "nothing touched" stand-in, behind a setting.** Under an Off-only slice harness
  setting, the touch update finds nothing: no touch begins or ends, and one harness-input log line is written. It is
  UNVERIFIED, as the base search's stand-in is (ruling on review of #798). Without the setting it stays the
  `LayerAbsent` above. Reference is unchanged. For the proxy just spawned on Maya in the combat world, nothing else
  collides near her at `maya:8`.

## 4. `Actor.SetHardAttach(optional bNewHardAttach)`

The exec takes the current `bHardAttach` for an omitted argument and calls the class's implementation. The
`PawnInteractionProxy`, `InteractionProxy`, `Trigger` and `Actor` tables all hold the same function, `Actor`'s own
body (section 1). That resolves NATIVE_GO_DOWN_PATH.md 3.2's "not resolved" for those four classes only:
- **the same value as now:** nothing;
- **a change with no `Base`:** `bHardAttach` := the new value;
- **a change with a `Base`:**
  - the current `BaseSkelComponent` and `BaseBoneName` are kept;
  - `SetBase(None)`;
  - then `bHardAttach` := the new value;
  - then `SetBase(the old base, …, the kept component and bone)`. Both `SetBase` calls go through the receiver's
    `SetBase` key, so they take NATIVE_ACTOR_SETBASE.md's rule only for a class on its list. Of the four,
    `PawnInteractionProxy` (section 5) and `Actor` are on it. `InteractionProxy` and `Trigger` hold the same `SetBase`
    body, but are not proposed for the list here; for them the inner `SetBase` keeps its `NotRead`.

**Proposed Off answer:**
- bind the key only for a receiver whose first native class is `Actor`, `Trigger`, `InteractionProxy` or
  `PawnInteractionProxy` (review r1 of #802). Every other class, NATIVE_ACTOR_SETBASE.md's other six tables included,
  keeps the unresolved `NotRead`, decided first;
- `bHardAttach` must be declared as a bool, else `VmError::Script`;
- **the change with a `Base`** (hub wording, review r1 of #802):
  - the preflight covers only the first `SetBase(None)` and the flag write. A refusal there stands with no prefix;
  - the first `SetBase(None)` raises `Detach` and `BaseChange`, and on a proxy with `Base` None, `InteractionProxy`'s
    `BaseChange` destroys it. So no preflight can predict what the second `SetBase` meets;
  - after those writes, the second `SetBase` runs its full preflight. A refusal there carries the partial-writes
    prefix;
  - a receiver destroyed in between (`bDeleteMe` set) gets `NotRead`, "Actor.SetHardAttach: the receiver was
    destroyed by its BaseChange", with the prefix.

**On the slice:** the proxy's `bHardAttach` is already true, so this does nothing.

## 5. `Actor.SetBase` on a `PawnInteractionProxy`

The `PawnInteractionProxy` table holds the same `SetBase` body as `Actor`'s. **So NATIVE_ACTOR_SETBASE.md section 3's
rule applies, and its class list gains `WillowGame.PawnInteractionProxy` as an eighth table** (its first native class).

**The proxy's call** (probe):
- the receiver: the proxy, `Location` (0, 0, 0), `Rotation` zero, `Role` authority, `bHardAttach` true, `bStatic`
  false, `bMovable` true, `Base` None;
- the new base: Maya, `Location` (0, 0, 0), `Rotation` zero, `DrawScale` 1, `DrawScale3D` (1, 1, 1), `PrePivot` zero,
  `bWorldGeometry` false;
- steps 1 to 5 pass (no collection actor, no loop, a movable receiver, no component, a change);
- step 6: no old base;
- step 7: `Base` := Maya;
- **step 8:** a hard-attached authority gets both computations. NATIVE_ACTOR_SETBASE.md 3.8 answers the second, the
  full inverse of the base's transform, `NotRead` ("SetBase: the hard-attach relative transform"). It does so for any
  hard-attached receiver that is not a simulated proxy.

**Proposed Off answer, the identity case of step 8** (an extension of NATIVE_ACTOR_SETBASE.md 3.8). It applies to
any hard-attached receiver that is not a simulated proxy: an authority or an autonomous proxy. Either one gets the
same computations, so the same zero pose (review r1 of #802). A simulated proxy gets neither, as 3.8 says.
- **when all of these hold:**
  - the receiver's `Location` equals the base's exactly, and its `Rotation` equals the base's;
  - the base's `Rotation` is zero;
  - the base's `DrawScale` is 1 and `DrawScale3D` (1, 1, 1);
  - the base's `PrePivot` is zero;
- **then** both computations give `RelativeLocation` (0, 0, 0) and `RelativeRotation` zero. Each matrix is the identity
  or a pure translation, so the result does not depend on the order or the rounding;
- **any other hard-attached case** stays the `NotRead`;
- every field must be declared with its kind, else `VmError::Script`, as 3.8 already lists, plus `DrawScale`,
  `DrawScale3D` and `PrePivot` on the base;
- UNVERIFIED in game.

**Where it lands** (review r1 of #802): in `crates/ow-world/src/world/set_base.rs`, `READ_CLASSES` grows from seven
entries to eight (`PawnInteractionProxy`), and the identity case goes into `check_pose`, ahead of the hard-attach
`NotRead`.

**Then steps 9 and 10:**
- Maya's `Attached` gains the proxy, and she gets `Attach(proxy)`: `Actor`'s empty body;
- the proxy gets `BaseChange()`. `InteractionProxy`'s body destroys the proxy when its `Base` is None, and otherwise
  does nothing. Here the `Base` is Maya, so nothing.

## 6. Predicted stop

With lane E's flag on and sections 3 to 5 answered (the probe):
- `SetUpTradeInteraction` completes;
- `maya:8` then stops at **`refuse_class_parts`' class-level `NotRead`** ("the pawn properties they land in are not
  named"). With the controller's `Pawn` set, the class-level part of `RecalculateAttributeInitializedState` now applies
  to Maya. **That is #796 section 3's rule**, which lane E is implementing. Once it is in, this stop clears.

Without the touch-update stand-in, the first stop is section 3's `LayerAbsent`.

**So to turn the flag on without link 1 regressing, an implementer needs:**
- section 3's rule, with the hub's touch-update stand-in;
- section 4's rule;
- section 5's class-list extension and identity case;
- #796 section 3's class-level part.

## 7. Tests (synthetic fixtures)

1. **`SetCollisionSize`:**
   - a toy actor with a `CylinderComponent` as its `CollisionComponent`:
     - the two sizes, `bNeedsReattach`, the event and `bNetDirty`, in order;
     - with no collision service: the `LayerAbsent`, nothing written;
     - (hub ruling) with the stand-in setting on: the writes as above, no touch begun or ended, and one
       harness-input log line; with it off, the `LayerAbsent`; Reference with the setting on: unchanged;
   - a non-cylinder `CollisionComponent`: no size write, then the rest;
   - a mistyped field: `VmError::Script`.
2. **`SetHardAttach`:**
   - the same value: nothing;
   - a change with no `Base`: the flag;
   - a change with a `Base`: `SetBase(None)`, the flag, then `SetBase(old)` with the kept component and bone;
   - a refusal in the first `SetBase`'s preflight: nothing written, no prefix;
   - the second `SetBase` refused (its receiver class not on SetBase's list, a toy `Trigger`): the first `SetBase`'s
     writes and the flag kept, the refusal with the prefix;
   - a toy `InteractionProxy` going from hard-attached to not: the first `SetBase(None)`'s `BaseChange` destroys it,
     then the `NotRead` with the prefix, and no second `SetBase`;
   - a toy `Trigger` and a toy `InteractionProxy`, each with no `Base`: the key is bound, the flag set;
   - a receiver class outside the four (a toy `WillowWeapon`, which is on SetBase's list): the unresolved `NotRead`,
     nothing written.
3. **`SetBase` on a toy `PawnInteractionProxy`:**
   - the identity case: `Base`, zero relative pose, the base's `Attached`, `Attach` then `BaseChange`; the same with
     the receiver an autonomous proxy; a simulated proxy: no pose computation;
   - a receiver one unit away, a base with a non-zero `Rotation`, a base with `DrawScale` 2 or a non-zero `PrePivot`:
     the hard-attach `NotRead`, nothing written.
4. **`InteractionProxy.BaseChange` after `SetBase(None)`:** the proxy is destroyed.
5. Reference: unchanged.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:** the touch update's overlap rule.
3. **By fit:**
   - the two cylinder fields' places;
   - the event's name;
   - the touch update's identity.
4. **Ruled by the hub on #802:** the touch update's "nothing touched" stand-in, behind an Off-only harness setting
   (section 3).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After the player spawns, read `TradeInteractionProxy`: its `Base`, `RelativeLocation`, `RelativeRotation`,
   `bHardAttach`, and its cylinder's `CollisionRadius` and `CollisionHeight`.
2. Read the proxy's `Touching` with another player standing within 80 units.
