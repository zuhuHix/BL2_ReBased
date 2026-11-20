## 2026-11-19: WillowWeapon.StaticGetWeaponDamageType in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-STATIC-GET-WEAPON-DAMAGE-TYPE, implementer. The decision number is
pending from the hub (three digits); NNN is a placeholder.

- **Source:** `docs/verification/NATIVE_STATIC_GET_WEAPON_DAMAGE_TYPE.md` (#576, accepted at 3225f887, analyst lane
  ANALYST-STATIC-GET-WEAPON-DAMAGE-TYPE, not this thread), section 4, with the hub's three points on #576 r1. No native
  code or analysis output was opened.
- **Base:** origin/main at d52bcc79.
- **No real-game session:** every rule is **UNVERIFIED** in the running game.
- **Unchanged:** no dependency was added and no bounds check was loosened. Reference binds nothing (the stub policy).

**What changed:** `crates/ow-inventory/src/equip/damage_type.rs` (new), with one module line in `equip.rs` and one
registration line in `manager.rs`'s pack.
- **The binding:** `WillowWeapon.StaticGetWeaponDamageType(weapondefinitiondata)` in Off. A key bound before keeps
  its binding.
- **The preflight** follows the note's checks 1 to 7 in the native's read order, and the first failing check answers:
  1. the type field must be present and an object field (`VmError::Script` otherwise);
  2. a None type answers None, and the elemental field is not looked at;
  3. an unloaded type is `NotYet { NotRead }`;
  4. the type's `DefaultDamageTypeDefinition` must be an object, checked before anything of the elemental part;
  5. the elemental field must be present and an object field (`VmError::Script` otherwise);
  6. a set, unloaded part is `NotYet { NotRead }`;
  7. a set part's `CustomDamageTypeDefinition` must be an object.

  It writes nothing, loads nothing and draws nothing. The damage-type handle is returned as stored, never loaded.
- **The shared core** is `choose`: it picks the part's custom type, else the type's default, from references already
  read.
  - `ow-combat`'s `hit::shot::weapon_damage_type` is not reused: it loads, and it answers the part's custom type over
    an unloaded weapon type.
  - The shot pipeline is unchanged.
- **The counters:** main has no #518 counters yet. The native neither writes nor loads, so none of its refusals takes
  the "stopped after partial writes" mark (the note's section 4).

**Tests** (an invented `WillowGame.upk` in a synthetic store, plus hand-built argument structs; ow-inventory 187 / 0):
- a None type answering None whatever the elemental field is, the part not loaded;
- the rule's three answers, the damage type never loaded;
- an unloaded type refused over a loaded part that has its own type;
- the type's field checked before the part (a missing or int field, with an unloaded part, a missing elemental field
  or a malformed part);
- checks 5 to 7;
- Off binding the key and Reference not;
- the core's choice.

The workspace (`-j 2`) passes 2,451 / 0 / 46 ignored.

**The installed Off `--slice-gate`** (strict, under the machine lock, pc-hummer), on a throwaway local merge with #572
at 3d948c43, deleted after:
- ok 1, NotYet 4, error 9 of 14;
- link 8 passes 1 of 5 driver steps (`make`) and stops at `init` on the unbound `WillowWeapon.CacheWeaponCard()`,
  before this native;
- #573 (0b9dbc0b), #534 (8fc0420b) and #552 (0438f6b3) were left out because each conflicts with #572 (`equip_run.rs`,
  `registry.rs`, `manager.rs`), and the brief included them only if they merged cleanly. So link 8 does not reach
  `StaticGetWeaponDamageType` on this probe, and #576's expected next stop (`SetSkeletalMesh`) is not confirmed here.

**Review r1 of #582:**
1. **Check 1's errors** name the field when the argument is not a struct, or when its struct has no registered
   layout.
2. **The tests go through the VM:**
   - the synthetic package declares the static native, and its VM-generated key is asserted to be the binding's key;
   - the pack's key-bound equip natives (the slots, the internal slots and this native) now register through one
     helper, `manager::register_equip_keys`. The pack calls it, and so do the tests;
   - the tests call the native through the VM in Off: None, the elemental answer, the two refusals as literal texts,
     and both malformed arguments;
   - in Reference, the binding is absent.

   ow-inventory now passes 190 / 0.

**Review r2 of #582:** in Reference, both unloaded-input cases are called through the VM, and each comes back as the
VM's `VmError::UnboundNative` naming the literal key, under the fixture's default stub policy.
