# `PrimitiveComponent.SetScale`: the uniform scale and the deferred transform update

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-SETSCALE (decision 2026-11-19-256-primitive-setscale-in-off).

**Sources.**
- The native was read in `Borderlands2.exe` locally, through the project's Ghidra tooling (`tools/ghidra/run.ps1`,
  read-only queries). Its two siblings, `SetScale3D` and `SetTranslation`, were read alongside it.
- Field identities come from `tools/ghidra/class_layout.py`.
- The raw output stays under ignored `local/analysis/setscale/`. This note is in our own words.
- No bl-sdk session was granted, so **every rule here is UNVERIFIED in the running game.**

It is a stock UE3 engine native, not one of Gearbox's. The hub's brief let this lane both read it and implement it,
because it turned out plain (section 3).

**Why now.** #635's probe of link 8 passes both `SetSkeletalMesh` calls, with its stand-ins, and stops next at the
unbound `PrimitiveComponent.SetScale(float)`.

## 1. The rule

`SetScale(float NewScale)` has no script body; the exec function is the whole rule.
1. **The argument.** It reads the one float argument.
2. **No change.** When `NewScale` equals the component's current `Scale`, it does nothing: no write and no update.
3. **A change.**
   - It writes `Scale` := `NewScale`.
   - It then runs the component's **deferred transform update**. That is the same routine `SetScale3D`,
     `SetTranslation`, `SetRotation` and `SetAbsolute` call after their own field writes. Those four natives have
     the same shape: read, compare, write, update.
4. **The deferred transform update.**
   - It sets `bNeedsUpdateTransform` on the component.
   - **No `Owner`:** when the component is `bAttached`, it asks the component's own transform update, a virtual
     whose body was not read. Otherwise nothing more.
   - **With an `Owner`:** when the owner is `bStatic` or `bTickIsDisabled`, and not `bDeleteMe`, and two of the
     owner's object flags are clear, it asks the owner's component update, a virtual whose body was not read.
     Otherwise nothing more. The two object flags were not named.
   - This matches step 10 of NATIVE_SPAWN_PICKUP_PARTICLES.md ("the deferred transform update", by fit there). This
     read confirms that step's flag and its two branches. It also adds the owner's two object-flag tests, which make
     the update rarer, never more frequent.
5. **Effects otherwise:** none. Nothing is loaded and nothing is drawn.

**Field identities.**
- **By computed layout:** the owner's three flags are `bStatic`, `bDeleteMe` and `bTickIsDisabled`, and the
  component's word holds `bAttached` and `bNeedsUpdateTransform` beside `Owner`.
- **By fit:** the float the native compares and writes is `Scale`. The computed layout and the code disagree on where
  `Scale` sits. They disagree by the same amount for `Scale3D`, which `SetScale3D` writes right after it, so the pair
  moves together. That points to native-only fields the computed layout does not see.

## 2. Comparison

The comparison is an ordinary float inequality: a NaN on either side counts as a change. That was not checked in the
running game.

## 3. The Off answer (implemented: `crates/ow-world/src/world/primitive_scale.rs`)

`PrimitiveComponent.SetScale(float)` is bound in `ParityMode::Off`. Reference binds nothing, so the stub policy answers
there. The preflight is write-free, load-free and draw-free. Each refusal comes before any write, in this order:
1. **No receiver,** or an argument that is not a float: `VmError::Script`.
2. **An unloaded receiver:** `NotYet { NotRead }`.
3. **The declarations:**
   - the component must declare `Scale` (a float), `bNeedsUpdateTransform` and `bAttached` (bools) and `Owner` (an
     object reference);
   - with an owner, the owner must declare `bStatic`, `bTickIsDisabled` and `bDeleteMe` (bools).

   Anything else is `VmError::Script`.
4. **No change** (`NewScale` equal to `Scale`): done, nothing written.
5. **The deferred transform update's unread virtuals:**
   - no `Owner`, and `bAttached`: `NotYet { NotRead }`, "the deferred transform update: the component's own
     transform update";
   - an owner that is `bStatic` or `bTickIsDisabled`, and not `bDeleteMe`: `NotYet { NotRead }`, "the deferred
     transform update: the owner's component update". The owner's two object-flag tests are not modelled. When they
     would skip the update, the port refuses where the game does nothing. That is conservative, not a wrong answer.
   - an owner the store has not loaded: `NotYet { NotRead }`, since its flags cannot be read.
6. **Otherwise, the writes:** `Scale` := `NewScale`, then `bNeedsUpdateTransform` := true. Both slots are checked in
   the preflight, so neither write can fail after the other.

## 4. Open items and the live check

- **Open:** the two object flags in the owner test; the bodies of the two virtuals; the computed layout's shift
  before `Scale`.
- **Live check (bl-sdk, when a session is granted):** hook `PrimitiveComponent.SetScale` and log the component's
  `Scale` and `bNeedsUpdateTransform` before and after a call that changes the scale, then after one that passes the
  same scale. Expect `Scale` written and the flag set on a change, and nothing on the same value.
