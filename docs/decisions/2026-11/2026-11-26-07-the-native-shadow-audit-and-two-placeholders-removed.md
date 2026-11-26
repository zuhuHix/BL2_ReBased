## 2026-11-26: The native shadow audit and two placeholders removed

AI-assisted (Claude Code, Opus 5.5). Lane PLACEHOLDER-SHADOWS, on main 64f070fa. No session was run. Nothing here
changes a rule of the game. Each removed placeholder was a `NotRead` for a native whose rule another pack already binds.

- **The problem:**
  - many packs bind a native only when no binding exists yet (keep-existing: a `has_native` or `is_bound` check, or a
    refused registration that the caller swallows);
  - so when two packs bind one key, the pack registered first wins with no error;
  - a placeholder registered first then keeps a real rule out. #744's landing check met the hard-duplicate form of
    this; the keep-existing form gives no error at all.
- **What was decided:**
  - **ow-vm keeps a record of who registered each key, and an opt-in audit.** Each native key keeps the source file
    of the call that first registered it (`#[track_caller]` on the three registration methods).
    - Under the audit, a `has_native` or `is_bound` check that finds a key bound by another file is recorded as a
      shadow, and so is a refused registration. `Vm::audit_native_shadows` turns it on for one VM,
      `audit_new_vms` for a thread's new VMs, and `Vm::native_shadows` lists what it met.
    - Off by default; nothing else changes. The test is "another file", which had one known blind spot:
      `ow_combat::hit::bind` is a helper that ow-combat and ow-skills both bind through. A shadow between two users of
      that helper was checked and registered from the same file (`hit.rs`), so the audit could not see it. Every other
      keep-existing guard calls the VM from its own module. *Closed by decision 2026-11-26-NNN: the helper is
      `#[track_caller]`, so the audit records its callers' files.*
  - **The sweep** is an ignored installed census (`ow-cli` `slice_gate_shadow_tests`). It covers the five worlds the
    slice gate builds: combat-run, equip, pickup, phaselock and mission. It found four keys:

    | Key | Bound by | Given up in | Action |
    |---|---|---|---|
    | `MeshComponent.GetInstancedMaterialInstanceConstant(int)` | ow-world `player_pawn_tail` (a `NotRead` placeholder) | ow-inventory `material_instances` (#744's rule) | placeholder removed; combat-run now registers `material_instances` in Off |
    | `WillowPlayerController.UpdateInputContext()` | ow-world `after_login` (the native's own rule) | ow-combat `go_down` (a `NotRead` entry of `UNRESOLVED`) | entry removed |
    | `AttributeDefinitionBase.ResolveContext(object)` | ow-inventory `equip::natives` | ow-combat `hit::bind_fn` (the action skill pack) | kept: one wrapper bound by both, by design |
    | `AttributeDefinitionBase.AddAttributeModifier(object,object,bool)` | the same | the same | kept, the same |

    After the change, the census finds no shadow outside those two allowed keys.

    *Added by decision 2026-11-26-NNN:* the phaselock and mission worlds register no inventory pack, so they leave
    `GetInstancedMaterialInstanceConstant` unbound. There it is an unbound-native refusal, not #744's rule.
  - **The test** (#744's all-packs test, `ow-cli` `combat/pack_wiring_tests.rs`):
    - it now also registers the inventory pack: its invented packages declare the two script functions that pack
      overrides, and the natives that the packs bind by declaration;
    - it runs under the audit and fails on any shadow except the two allowed ones, which `shared_wrapper_shadow`
      names by exact key and the two exact files;
    - before the removals it failed with exactly the census's two placeholders.
  - **Queue 0r** (review of #744): `GetInstancedMaterialInstanceConstant`'s preflight requires the instance class's
    first native class to be `Engine.MaterialInstanceConstant`, else `SetParent`'s receiver refusal. So that refusal
    comes before the instance is made, not after.
- **Verified:** the census and the slice gate on the installed packages (no row moved), and the tests on invented
  data.
- **Not verified:** nothing about the game is claimed. The two remaining shadows are judged harmless because both sides
  name one function.
- **Sensitive files:** none.
