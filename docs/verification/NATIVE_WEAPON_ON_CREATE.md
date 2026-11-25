# The weapon's creation event: WillowWeapon.OnCreate (2026-11-25)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINK8-NEXT (decision 2026-11-25-NNN), the analyst half of the clean
room: **another lane implements this note.** Raw output stays under ignored `local/analysis/link8next/`. This note is
in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here is
UNVERIFIED in the running game.**

**Why.** A look-ahead for link 8 (`equip_gun`, phase `init`), past `SetColorParameter`
([NATIVE_PSC_SET_COLOR_PARAMETER.md](NATIVE_PSC_SET_COLOR_PARAMETER.md), #719). Section 1 lists the path. Its first
native with no note is `WillowWeapon.OnCreate`, which `ClientInitializeFromWeaponTypeDefinition` calls just before it
returns.

## 1. Link 8's path past SetColorParameter (on paper, not a run)

Read with `research/script_disasm.py`. Link 8's data was read with `ow-package --def-dump` and `--behavior-dump` (input
`Startup.upk`) on the fixed weapon `EQUIP_WEAPON` (`ow-cli` `slice_gate.rs`). In order:
1. `InitEffects`: `SetColorParameter`, read in #719;
2. `InitVisibleAmmoBones` (script): the body part `AR_Body_Bandit` has no `VisibleAmmoBoneNames`, so
   `SkeletalMeshComponent.MatchRefBone` (no note) is not reached;
3. the `ExtraSlots` loop: `Default__WillowWeapon` has `ExtraSlots` empty, and only `InitExtraMeshes` fills it, from a
   pawn owner's body class. So nothing is reached;
4. `InitMeshes` returns. Then `InitAltMuzzleFlashes` (script):
   - `NumberOfMuzzleFlashes` := 1 + the barrel's `AltMuzzleFlashSockets` count. `AR_Barrel_Bandit` has none, so it is 1;
   - the alternate-flash arrays are empty, so neither `ClearAltMuzzleFlashes` nor `CreateAltMuzzleFlashes` (no notes)
     is reached;
5. `bUseOverheatBehavior` := the type's value, false for `WT_Bandit_AssaultRifle`. The overheat block is skipped;
6. `SetElementalFrame` (script). It calls `StaticGetWeaponDamageType`, `GetWillowGlobals` and
   `GetGlobalsDefinition`, all indexed, then writes `ElementalFrame` from the globals' `ElementalFrameNames`;
7. **`WillowWeapon.OnCreate`** (native, no index row): **this note**;
8. back in `InitializeInternal`: `GetFiringModeDefinition`. It is a native with a note
   (NATIVE_WEAPON_TRIGGER_AMMO.md, index row "WillowWeapon.GetFiringModeDefinition"). `ow-combat` has a Rust
   `firing_mode` helper, but no Off binding of the native was found. **Predicted next stop: that unbound native;**
9. the type's `DefaultFiringModeDefinition`, `Bullet_Assault_Default`, has `ExplosionOverrideDefinition` None. So the
   explosion block (`GetExplosion`, `StoreExplosionInstanceData`, no notes) is skipped, and `InitializeInternal`
   returns;
10. the driver's next step, `bases`: `CalculateWeaponBaseValues`, already run by the equip pack.

## 2. Evidence for OnCreate

- **bl-sdk live reads:** none. No session was granted.
- **The script:**
  - `WillowWeapon.OnCreate(optional array Providers)` and `WillowItem.OnCreate` are natives, each with a `Providers`
    array parameter;
  - the only script callers are `ClientInitializeFromWeaponTypeDefinition` and
    `WillowItem.ClientInitializeFromItemDefinition`, and both omit the argument;
  - the same function first registers the weapon as a behavior consumer (`ConsumerHandle` :=
    `RegisterBehaviorConsumer(self)`). It then initialises the providers of the weapon type definition and of its parts
    for that handle, and calls `BeginNondeterministicProviderRegistration`.
- **The native:** Ghidra 12.1.4 headless, read-only, under the machine lock:
  - the exec and the routine it calls;
  - the kernel routine that routine reaches, compared with the routines that
    `BehaviorKernel.BroadcastBehaviorEventFromScript` and `ActivateBehaviorEventFromScript` reach;
  - `WillowWeapon.GetBehaviorConsumerHandle`'s exec, which calls the same interface slot as `OnCreate` does.
- **Layouts:** the weapon's `IBehaviorConsumer` interface table sits where the routine reads it (the local copy of
  `tools/ghidra/class_layout.py`).

## 3. What OnCreate does

1. **The argument.** With `Providers` omitted (as both script callers do), the routine gets no array. A supplied
   array is passed through only when the engine gives its variable's address. That is the normal case for a variable,
   but it was not read for other arguments.
2. **The handle.** The weapon's consumer handle, through the weapon's `IBehaviorConsumer` interface. That is the same
   slot `WillowWeapon.GetBehaviorConsumerHandle`'s exec reads. By fit, it is the weapon's own `ConsumerHandle`, as
   NATIVE_BEHAVIOR_CONTEXT.md reads it for the pawn; the slot's body was not read.
3. **The event.** The name `OnCreate`, with the output-link filter -1 (every output link) and no parameters, goes to
   the kernel. The kernel step is **the one `BroadcastBehaviorEventFromScript` uses, without parameters:**
   - no kernel, or a handle the kernel does not hold as a live consumer: nothing;
   - no array: the event is fired on every provider of the consumer. Each provider's enabled event entries named
     `OnCreate` run, as the broadcast rule already ported says (COMBAT_DEATH.md: every provider, in registration
     order);
   - an array: once per listed provider, in array order;
   - then the kernel's usual step after firing.

   `ActivateBehaviorEventFromScript`'s step differs only in that it needs a provider: a None provider fires nothing
   there.
4. **Nothing else.** The native writes no field itself. What runs is the providers' own behaviors.

**On link 8.** The weapon is registered, and its providers are those of `WT_Bandit_AssaultRifle` and its parts. Every
one of the nine providers on the fixed weapon (the type, body, barrel, grip, sight, stock, material, prefix and title)
has **zero behavior sequences** (`--behavior-dump`, `values_decoded` true). So `OnCreate` fires nothing on link 8.

## 4. The Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** Bind `WillowWeapon.OnCreate` in Off, in the behavior pack next to
`BroadcastBehaviorEventFromScript`. Reference keeps the stub policy.
1. **The argument:** omitted gives every provider. A supplied `Providers` array is `NotYet { NotRead }`,
   "WillowWeapon.OnCreate with a Providers argument (how a non-variable argument is passed is not read)". No slice
   caller passes one.
2. **The handle:** the weapon's `ConsumerHandle`, a plain read as `WillowPawn.GetBehaviorConsumerHandle`'s Off rule
   reads the pawn's. Missing or of the wrong kind: `VmError::Script`.
3. **The event:** the kernel's broadcast, as `BroadcastBehaviorEventFromScript` is bound, with:
   - event name `OnCreate`;
   - filter -1;
   - no payload;
   - every provider of the consumer.

   An unknown or dead handle, or no kernel, is nothing. The behaviors it fires answer for themselves; their refusals
   keep their own kinds.
4. **No other write.**

`WillowItem.OnCreate` has the same script shape. Its native was not read; leave it unbound.

**What link 8 would meet** (a prediction, not a run): `OnCreate` fires nothing. Then the unbound
`WillowWeapon.GetFiringModeDefinition` (item 8 of section 1).

## 5. Tests for the implementer

Synthetic, with `package()` / `tag()`. Each refusal leaves #518's three counters and the name table unchanged:
1. a toy weapon registered as a consumer with one provider whose sequence has an `OnCreate` event entry: `OnCreate()`
   runs that sequence's behaviors once, with every output link;
2. two providers, each with an `OnCreate` entry: both run, in registration order;
3. a provider whose only event is another name: nothing runs;
4. a weapon whose `ConsumerHandle` the kernel does not hold (never registered, or unregistered): nothing, and no
   error;
5. providers with no sequences (link 8's case): nothing;
6. a supplied `Providers` array: the `NotRead`, before anything is fired;
7. `ConsumerHandle` missing or of the wrong kind: `VmError::Script`;
8. an `OnCreate` entry whose enabled bit is clear: not run (the kernel's per-entry test, shared with the broadcast);
9. Reference: unchanged.

## Open questions

1. **UNVERIFIED:** every rule above. The kernel step is shared with the broadcast, by the routines read.
2. **Not read:** the consumer interface slot's body (the handle, by fit `ConsumerHandle`); how a supplied
   non-variable `Providers` argument is passed; `WillowItem.OnCreate`'s native.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Hook `WillowWeapon.OnCreate` while a Bandit assault rifle is spawned. Expect one call, with no argument. Expect no
   behavior to run from its providers.
2. On a weapon whose part has an `OnCreate` behavior event (not found on the slice), expect that event to fire once
   per spawn.
