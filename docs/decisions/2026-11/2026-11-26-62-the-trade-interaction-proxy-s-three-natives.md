## 2026-11-26: The trade interaction proxy's three natives: SetCollisionSize, SetHardAttach and SetBase

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-PAWN-INTERACTION-PROXY, on main 50114dc. Clean room: implemented
from docs/verification/NATIVE_PAWN_INTERACTION_PROXY.md (#802 as landed, r1) alone; this lane did not read the
executable. No session was run. Every rule is UNVERIFIED in the running game.

- **What was decided** (Off only; Reference binds neither new key), in `ow-world`:
  - **`Actor.SetCollisionSize`** (`world::proxy_natives`, replacing `go_down`'s `NotRead`, "the component's world
    update"): the preflight is write-free and load-free. It checks `CollisionComponent` (an object), on a
    `CylinderComponent` its two sizes and the deferred reattach's declarations and refusals (`component_setters`),
    `bNetDirty`, and `CollisionChanged` resolved as Off event dispatch resolves it (code not loaded is `NotRead`). Then
    comes the touch update: a world with a collision service is `NotRead` (its overlap rule is not read), and one with
    none is `LayerAbsent`, unless the **hub's ruling on review of #802** applies. Under the Off-only harness setting
    `NothingTouched` (a registration error outside Off or twice; logged at set-up, turned on in the slice's combat
    world), the touch update finds nothing, with one harness-input line per call. The stand-in covers only the
    world with no collision service; a world with one keeps its `NotRead`. The commit runs in the note's order:
    `CollisionHeight`, `CollisionRadius` (by fit), `bNeedsReattach`, the event, then `bNetDirty`. A refusal after the
    first write is prefixed;
  - **`Actor.SetHardAttach`** (`world::proxy_natives`, leaving `go_down`'s unresolved list): it is bound for a first
    native class of `Actor`, `Engine.Trigger`, `WillowGame.InteractionProxy` or `WillowGame.PawnInteractionProxy`.
    Any other class keeps the unresolved `NotRead`, decided first. The same value does nothing; a change with no
    `Base` writes the flag. A change with a `Base` takes its refusals from the first `SetBase(None)`'s preflight and
    the declarations (no prefix), then runs `SetBase(None)` and writes the flag. A receiver whose `bDeleteMe` is set
    then gets `NotRead` with the prefix ("destroyed by its BaseChange"); otherwise the second `SetBase(old base, ,
    kept component, kept bone)` runs with its full preflight, a refusal prefixed. Both inner calls are `set_base`'s
    own plan and commit, which take the rule only for a class on its list;
  - **`Actor.SetBase`** (`world::set_base`): `READ_CLASSES` gains `WillowGame.PawnInteractionProxy` (eight). The
    identity case goes ahead of the hard-attach `NotRead` in `check_pose`. A hard-attached receiver that is not a
    simulated proxy, with `Location` and `Rotation` equal to the base's, on a base with a zero `Rotation`,
    `DrawScale` 1, `DrawScale3D` (1, 1, 1) and `PrePivot` zero, takes the zero relative pose; any other
    hard-attached case keeps the `NotRead`. The base's three scale fields are read, and must be declared, only on
    the hard-attach path.
- **A gap in the note's test 2:** it expects a hard attach on a toy `Trigger` with a `Base` to keep the first
  `SetBase`'s writes and refuse the second with the prefix. By the rule, the first `SetBase(None)` already refuses on
  a `Trigger`, which is not on SetBase's list, in its preflight, so nothing is written and there is no prefix. The
  test asserts that, and covers the prefixed second refusal with a proxy one unit off its base.
- **Tests** (invented packages, `proxy_natives_tests.rs`): the note's tests 1 to 5. SetCollisionSize: the order of
  the writes under the stand-in, the `LayerAbsent` without it, the `NotRead` with a collision service, a
  non-cylinder component, the owner rule's refusal and a mistyped argument writing nothing, and the setting's
  Off-only registration. SetHardAttach: the same value, no `Base`, `Trigger` and `InteractionProxy` bound, a change
  with a `Base` rebased with the event order, a class outside the four, a refused first `SetBase`, the prefixed
  second refusal, and the destroyed proxy's prefixed `NotRead` with no second `SetBase`. SetBase on a proxy: the
  identity case as authority and as autonomous proxy, the simulated proxy with no pose, and four non-identity cases
  refused with nothing written. `BaseChange` after `SetBase(None)` marks the proxy deleted. Reference binds neither
  key. The world fixture's `Actor` gains `DrawScale`, `DrawScale3D` and `PrePivot`. Review of #805: the identity case also requires each `Location` part to be finite; added are a turned receiver on an unturned base, a base with `DrawScale3D` (1, 1, 2) and equal infinite `Location`s (each the hard-attach `NotRead`, nothing written), and a stale relative pose zeroed for the authority and the autonomous proxy and kept for the simulated proxy; link 6's two older hard-attach tests put the receiver off its base, so they no longer pass only by the fixture's zero `DrawScale` (removing the rotation, `DrawScale3D`, finiteness or pose-role checks each fails a test).
- **Measured:** see the GATES row of this lane, including the local probe merged with #803 and the deferred player
  `Pawn` forced on.
- **Verified:** the tests on invented data. **Not verified:** everything in the running game.
- **Sensitive files:** none.
