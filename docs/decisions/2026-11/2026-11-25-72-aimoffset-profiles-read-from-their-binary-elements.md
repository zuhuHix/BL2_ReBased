## 2026-11-25: AimOffset profiles read from their binary elements, and the store marks a skipped property

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-AIMOFFSET-PROFILES (queue 0o), stacked on lane D's
fix/rust-aimoffset-set-profile. The design was ruled by the hub before coding. No session was run. The format is
**fitted, UNVERIFIED** in the running game.

- **What was decided, in three commits:**
  1. **The marker.** `ow_object::decode::DecodeState::skipped` records `(object, property)` only where
     `ValueCx::apply_tags` logs "defaults: skipped" for a declared top-level property. The log line, the slot (the
     class default) and the return value are unchanged. `ow_defs::DefStore` records each load's pairs by the handle the
     load moved into (live objects only, pruned with the rest), and gains the read-only `DefStore::is_skipped(obj,
     prop)`. Undeclared tags and fields skipped inside a struct are not tracked. No existing caller changes: the
     `decode.rs` diff is 9 lines.
  2. **The reader.** `ow_assets::aim_profiles::read` reads `AnimNodeAimOffset.Profiles`' value. `read_export` finds
     the tag in an export's tag stream.
  3. **The switch.** In `SetActiveProfileByName` / `ByIndex` (step 3 of NATIVE_AIMOFFSET_SET_PROFILE.md, #730):
     - a decoded non-empty list is the slot's values;
     - a list the store marked skipped is the profiles layer's names (`ow_world::world::aim_profiles`, cached by the
       node, its package and its export index);
     - an empty list is "no match" only when the owner's own export tags `Profiles` (review of #737); an untagged or
       inherited empty list, or one over a skipped class default, is the reader's "no Profiles tag" `LayerAbsent`;
     - any other empty list keeps the `LayerAbsent`.
- **The layer's shape.** `ow-world` may not depend on `ow-assets` (the crate edges), so the layer takes the reader as
  an installed `ProfileSource`, as the skeletal mesh layer takes its skeleton source. `ow-cli` installs
  `ow_assets::aim_profiles` at the combat run's set-up, logged as a harness input. Every refusal is
  `NotYet { LayerAbsent }` on "a reader for AnimNodeAimOffset.Profiles":
  - no source is installed;
  - the node is not a loaded export, or its export has no `Profiles` tag;
  - the source's tag walk, from the ordinary prefix 4, does not end where the store's own decode ended (so a prefix
    other than the store's is refused, not misread);
  - the reader's oracle refuses.
- **The format (fitted).** A 4-byte count, then each `AimOffsetProfile` untagged, its fields in declaration order with
  the transient ones left out:
  - left out: `AimTransformsCache` and `bDirtyAimTransformsCache`, exactly the struct's two fields with the transient
    flag;
  - a profile: `ProfileName`, the two 2-float ranges, the `AimComponents` count, the components, then nine
    `AnimName_*` names. That is 100 bytes plus the components;
  - a component: `BoneName` and nine `AimTransform`s, each a quaternion and a translation: 260 bytes.

  The serializer was not read in the executable.
- **The structural oracle, enforced:** every count fits the bytes left; every name index is in the package's name
  table and its number is not negative; every float is finite; every quaternion is unit length (squared length within
  1e-4 of 1); the read ends exactly at the value's end. Nothing is clamped.
- **Evidence (counts only):** the ignored installed census `installed_aim_profiles_census` reads all 5 aim-offset nodes
  in the 4 installed class packages, none refused:
  - Siren, Assassin and Soldier: 7 profiles each (components 27, 27, 26, 11, 26, 26, 26);
  - Mercenary: 3 (27, 26, 17);
  - a second Soldier node: 1 (2).

  Maya's node is 4 + 44,640 = 44,644 bytes, its tag's size. Local raw output stays under ignored
  `local/analysis/profiles/`.
- **Tests:**
  - the marker in `ow-defs`, in both modes: skipped, decoded-empty, untagged, through a handle's load;
  - the reader on invented bytes: the values, and each oracle miss refused;
  - the switch in `ow-world`: the reader's names on the node and through a run-time copy, a reader refusal, a walk
    mismatch, decoded-empty giving "no match", and untagged, inherited or
    over a skipped class default giving `LayerAbsent`;
  - the harness log line in `ow-cli`.
- **The slice gate** (installed Off strict, under the machine lock, base d906ee9f and this head 762bc93a, run the same
  way): 5 ok / 8 NotYet / 1 error on both.
  - Links 1, 3, 4, 5, 13 and 14 move in `maya:8` from "AnimNodeAimOffset: the profiles (a binary-serialized struct
    array)" to "AnimNodeAimOffset: the required-bones rebuild's sort and parent pass" (`NotRead`). Maya's `Unarmed`
    profile is found at index 3, the index changes, and her node's component has a mesh, so the rebuild refuses
    (NATIVE_AIMOFFSET_SET_PROFILE.md, step 5).
  - Link 8's error (the unbound `WillowWeapon.GetFiringModeDefinition()`) is the base's own. Every other row is
    unchanged.
- **Sensitive files:** `crates/ow-object/src/decode.rs` sits close to the sensitive reader code. Its change is the
  marker only, and the decoding is unchanged. `crates/ow-package`, `Cargo.toml` and the crate edges are unchanged.
- **Verified:** nothing in the running game.
