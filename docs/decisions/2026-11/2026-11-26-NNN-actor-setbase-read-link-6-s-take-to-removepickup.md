## 2026-11-26: Actor.SetBase is read, and link 6's take to RemovePickup

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-ACTOR-SETBASE, docs only, rebased onto main 969d7a3 (from d3be3e5; #789, #790 and #791 landed in between). The analyst half of the
clean room: another lane implements NATIVE_ACTOR_SETBASE.md. No session was run. Raw output stays under ignored
`local/analysis/setbase/`. UNVERIFIED in the running game.

- **What was read.** `Actor.SetBase`'s exec hands to the class's own implementation. NATIVE_GO_DOWN_PATH.md 3.2 had
  left that unresolved. It was found in the class tables of `Actor`, `Inventory`, `Weapon`, `WillowWeapon`,
  `WillowInventory`, `DroppedPickup` and `WillowPickup`, by the registration-and-constructor method of
  `docs/NATIVE_ANALYSIS.md`, and it is the same function in all seven. Its rule is section 3 of the note:
  - four cases that do nothing (a base loop, a fixed receiver onto a moving or deleted base, a component of another
    actor, and no change);
  - else the old base is left (`Attached` removal, `Detach`), `Base` is set and the bone fields cleared, the relative
    pose is computed, the new base is joined (`Attached` append, `Attach`), and `BaseChange` is raised.

  Which event goes where is by fit.
- **A method note.** The registration routine refers to the class's prefixed C++ name, two bytes in. A search for the
  bare script name therefore finds nothing. Recorded in `docs/NATIVE_ANALYSIS.md`.
- **The slice.** The weapon's `Base`, `BaseSkelComponent` and `BaseBoneName` are None, and the call is `SetBase(None)`,
  so the rule does nothing.
- **Link 6 after it**, by a local probe (on #789's head a9f7187a; temporary stand-ins by the rules read, reverted):
  1. `WillowWeapon.GetAmmoCount()`, unbound. Its rule is NATIVE_WEAPON_TRIGGER_AMMO.md section 2, re-read here; on the
     slice it answers `StoredAmmo`, 30;
  2. `AttributeExpression.SelectAkEvent`, unbound. It is read: the first entry whose non-empty expressions hold, else
     the last entry with none. The slice's list has one empty entry;
  3. `Actor.PlayAkEvent`, unbound. The note's default Off answer is the audio `LayerAbsent`;
  4. the pickup's `Destroy()` while it is in its state `Pickup`, the port's W5-STATES `LayerAbsent`. That state's
     `EndState` is a bare return in the script;
  5. `WillowGlobals.RemovePickup`'s `NotRead`, in `WillowPickup.Destroyed`.
- **The hub's rulings (2026-11-26), recorded in the note, UNVERIFIED:**
  - `PlayAkEvent`: a presentation stand-in behind an Off-only harness setting that the slice turns on, like
    `PhysXLevel` and `AtRestBodyPose`. With it on, a set event is a logged no-op returning the default handle ("audio
    is not modelled: the event is not played (hub ruling)"). Without it, the audio `LayerAbsent` stands;
  - the stateful destroy: lifted only when the `EndState` that the method lookup resolves for the actor's current
    state is a script function whose body is a bare return, read with no load. Every other case keeps the W5-STATES
    `LayerAbsent`. The rule checks the body, not a state name.
- **Paperwork:**
  - `NATIVE_INDEX.md`:
    - new rows for `SetBase`, and for `SelectAkEvent` with `PlayAkEvent`;
    - `SetBase` taken out of the shared go-down row;
    - the `GetAmmoCount`, `RemovePickup` and `Destroy` rows given link 6's use;
  - a pointer in NATIVE_GO_DOWN_PATH.md 3.2;
  - a GATES row.
- **Verified:** the reads and the probe's rows. **Not verified:** everything in the running game.
- **Sensitive files:** none.
