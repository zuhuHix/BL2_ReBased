# The weapon's firing mode: WillowWeapon.GetFiringModeDefinition (2026-11-25)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-GETFIRINGMODEDEFINITION (decision 2026-11-25-66), the analyst half
of the clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/firingmode/`.
This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every
rule here is UNVERIFIED in the running game**, except where a line says it was observed.

**Why.** Link 8 (`equip_gun`, phase `init`) reaches `WillowWeapon.GetFiringModeDefinition()` once `OnCreate` is bound
(NATIVE_WEAPON_ON_CREATE.md, #726, section 1 item 8). The native has had an index row since lane G13, pointing at a
one-sentence rule in NATIVE_WEAPON_TRIGGER_AMMO.md section 3. Two records disagreed on its second step, and nothing in
Off binds it.

## 1. Evidence

- **The running game (lane L7, PROJECTILE_FIRING_PATTERNS_LIVE.md section 3, OBSERVED):** over about 26 gun builds,
  the native returned the barrel part's `CustomFiringModeDefinition` when the barrel had one, and the weapon type's
  `DefaultFiringModeDefinition` otherwise. No installed elemental part carries a mode, so the elemental step was never
  exercised live. No new session was run for this note.
- **The executable** (Ghidra 12.1.4 headless, read-only, under the machine lock):
  - the exec wrapper takes no parameters and returns a virtual of the weapon object;
  - that virtual is the same function in the class tables of `WillowWeapon` and of its three native subclasses
    (`WillowBuzzaxeWeapon`, `WillowTurretWeapon`, `WillowVehicleWeapon`, the only script subclasses in `WillowGame`).
    So no subclass overrides it. The class table was found from each class's registration and constructor, as
    NATIVE_ANALYSIS.md's "When `virtual:` refuses a class" describes, with a throwaway helper kept under
    `local/analysis/firingmode/scripts/` (not tracked);
  - the field names come from `tools/ghidra/class_layout.py` on the installed packages. Every read lands on the named
    field of the expected type, and the class size the layout tool computes for `WillowWeapon` equals the one its
    registration passes. So the names are a strong fit.
- **The script** (`ow-package --exports` on `WillowGame.upk`, and `research/script_disasm.py`):
  `GetFiringModeDefinition` is declared on `WillowWeapon` only, as a native function whose one child is its
  `ReturnValue`, an object property: no parameters. `InitializeInternal` is link 8's caller (section 4).
- **The slice's data** (`ow-package --def-dump`, input the installed `CookedPCConsole`), on link 8's fixed weapon
  `EQUIP_WEAPON` (`ow-cli` `slice_gate.rs`):
  - `GD_Weap_AssaultRifle.Barrel.AR_Barrel_Bandit`: `CustomFiringModeDefinition` None;
  - the weapon has no elemental part;
  - `GD_Weap_AssaultRifle.A_Weapons.WT_Bandit_AssaultRifle`: `DefaultFiringModeDefinition`
    `GD_Weap_AssaultRifle.FiringModes.Bullet_Assault_Default`;
  - that mode: `FireType` 0 (Bullet), `ExplosionOverrideDefinition` None.

## 2. The rule

`GetFiringModeDefinition()` reads the weapon's own `DefinitionData` and nothing else of the weapon. In order:
1. the barrel (`DefinitionData.BarrelPartDefinition`) set, with its `CustomFiringModeDefinition` set: that mode;
2. otherwise the elemental part (`DefinitionData.ElementalPartDefinition`) set, with its `CustomFiringModeDefinition`
   set: that mode;
3. otherwise the weapon type (`DefinitionData.WeaponTypeDefinition`) set: its `DefaultFiringModeDefinition`, None
   included;
4. otherwise None.

A part that is None is skipped. A part's mode is taken as it stands: the native does not read the mode it returns.
**It writes nothing**, to the weapon or anywhere else, and keeps no cache: every call reads the three fields again.

**The second step settles a disagreement.** NATIVE_WEAPON_TRIGGER_AMMO.md section 3 had the elemental part second.
NATIVE_WEAPON_FIRING.md section 1.1 had only "the barrel's or the type's". The executable has the elemental step, so
TRIGGER_AMMO was right. The live record never saw that step, because no installed elemental part has a mode.
`ow-combat`'s `hit::shot::firing_mode` refuses exactly that case today; this rule answers it.

**For link 8's Bandit AR:** the barrel has no mode and there is no elemental part, so the answer is the type's
`Bullet_Assault_Default` (a Bullet mode).

## 3. Proposed Off answer (for the implementer)

Bound in `ParityMode::Off` only; Reference keeps the stub policy. Read only, and **load-free**: no store load and no
class-code load. The checks run in the rule's order and stop where the rule stops:
1. **The weapon:** its class declares `DefinitionData` as the weapon-definition struct, and the struct holds
   `BarrelPartDefinition`, `ElementalPartDefinition` and `WeaponTypeDefinition` as objects. Otherwise `VmError::Script`
   naming the field.
2. **Each object the rule reads, when it reads it** (the barrel, then the elemental part, then the type):
   - an unloaded or failed store handle: `NotYet { NotRead }`, "WillowWeapon.GetFiringModeDefinition: the
     <barrel part | elemental part | weapon type> <path>, which the store has not loaded (a load is a write)", with
     nothing loaded;
   - its class must declare `CustomFiringModeDefinition` (a part) or `DefaultFiringModeDefinition` (the type) as an
     object. Otherwise `VmError::Script` naming it.

   An object the rule does not reach is not checked. For example, an unloaded elemental part on a weapon whose barrel
   has a mode is no refusal.
3. **The result:** the mode object as the field holds it. A store handle is returned as it is, not loaded; a script
   that reads it loads it the way script reads do.

No partial-writes prefix can arise: the native writes nothing.

The existing `hit::shot::firing_mode` helper loads the objects it reads, and refuses the elemental case. The native
needs neither behaviour. Whether the shot path keeps its own helper or calls the native's rule is the implementer's
call; the shot path's own rules (NATIVE_WEAPON_TRIGGER_AMMO.md) are not changed by this note.

## 4. Link 8's call shape, and what init calls after it

`InitializeInternal` stores the result in a local. When it is not None, it reads the mode's `ExplosionOverrideDefinition`;
only when that is set does it go on to the explosion block (`StoreExplosionInstanceData`, with the mode's
`ExplosionOverideInstanceDataName`). Then it returns.

What link 8 then meets (a prediction from the data and the script, not a run):
1. the script reads `Bullet_Assault_Default.ExplosionOverrideDefinition`. If the mode is still a store handle, the
   VM's script read loads it on demand (a script-side load, after the phase's earlier writes). It is None, so
   `StoreExplosionInstanceData` (no note, no index row) is not reached;
2. `InitializeInternal` returns, and the `init` phase ends;
3. `bases`: `WillowWeapon.CalculateWeaponBaseValues` (script; NATIVE_WEAPON_ON_CREATE.md item 10 notes the equip
   pack already ran it);
4. `equip`: `WillowWeapon.ApplyAllExternalAttributeEffects` (script);
5. `unequip`: `WillowWeapon.RemoveAllExternalAttributeEffects` (script).

The natives these three scripts reach were not traced for this note.

## 5. Tests for the implementer

Synthetic, with `package()` / `tag()`. Each refusal leaves #518's three counters and the name table unchanged, and
causes no class-code load:
1. a barrel with a mode: the barrel's, whatever the elemental part and the type hold;
2. a barrel without one, an elemental part with one: the elemental part's;
3. neither has one: the type's `DefaultFiringModeDefinition`; with that None, None;
4. no barrel and no elemental part: the type's;
5. no type either: None;
6. an unloaded barrel, elemental part or type **on the rule's path**: the `NotRead` naming it, nothing loaded. The
   same handle off the path (an unloaded elemental part behind a barrel with a mode, an unloaded type behind any part
   with a mode): no refusal;
7. a mode that is an unloaded store handle: returned as the same handle, still unloaded;
8. `DefinitionData`, a part's `CustomFiringModeDefinition` or the type's `DefaultFiringModeDefinition` declared as
   something else: `VmError::Script` naming it;
9. writes nothing: the counters are unchanged after a successful call too;
10. a toy subclass of `WillowWeapon`: the same answer (no native subclass overrides it);
11. **link 8's call shape:** `InitializeInternal`'s tail on a weapon whose type default is a mode with
    `ExplosionOverrideDefinition` None: the local takes the mode, the explosion block is skipped and the function
    returns. With the override set, the call reaches `StoreExplosionInstanceData` (the next stop then);
12. Reference: unchanged.

## Open questions

1. **UNVERIFIED:** every rule above. The barrel and type steps are OBSERVED (lane L7); the elemental step and the
   None cases are read only.
2. **By fit:** the field names, through `class_layout.py` (section 1).
3. **Not traced:** the natives that `CalculateWeaponBaseValues`, `ApplyAllExternalAttributeEffects` and
   `RemoveAllExternalAttributeEffects` reach on link 8.

## Live check that would confirm the rules (bl-sdk, the RTSE way; not run, no session granted)

On a weapon built with an elemental part that carries a `CustomFiringModeDefinition` (none does in the installed data,
so the test would set one in memory) and a barrel without one: hook `GetFiringModeDefinition` and log its return.
