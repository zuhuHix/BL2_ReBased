## 2026-11-25: The skeletal mesh data layer reads the reference skeleton from the mesh's package export

AI-assisted (Claude Code, Opus 5.5). Lane SKELMESH-REFSKELETON, stacked on #673 (fix/rust-skelmesh-lod-count-2,
fbfcd3d3, decision 2026-11-25-09).

- **What was decided:** `ow-assets` `skeletal_lods::read_lod_models` now also returns the reference skeleton, as
  `RefBone`s: name, parent, rotation, position, stored child count, colour. It is given only by a read of the whole
  export, so every check of decision 2026-11-25-09 has held. The skeletal mesh data layer gets a defaulted
  `SkeletonSource::skeleton_in`, asked by the mesh's export origin. `reference_skeleton` asks the name-based source
  first (a UModel `.psk` export, behind the flat-name guard). Only if that has no answer does it ask the package.
  `ow-cli`'s source implements `skeleton_in` from the package. The `.psk` path is unchanged and still wins when an
  export exists.
- **The bone layout, fitted and UNVERIFIED** (AGENTS.md rule 4). Each bone is 52 bytes, recovered in the earlier
  lane: a name-table entry, a flags word, a rotation quaternion `(x, y, z, w)`, a position, the child count, the
  parent index and a colour word. New checks, enforced in code:
  - the flags word is zero (every census bone; its bits are not decoded, so another value is refused);
  - the rotation is a unit quaternion (length within a thousandth of 1, compared as squares; the census' largest
    departure is under 2e-7).

  The checks the prefix already made still hold: valid names, finite numbers, the root its own parent, parents
  before children, and the stored depth equal to the parent chain's. Together with the LOD models, the bone-name map
  that follows must name every bone by its own name.
- **What is not checked:** the stored child count. It equals the bone's actual children in 104 of 106 census meshes;
  the other two (one composite gestalt mesh, one prop) each hold one bone whose stored count differs, so its meaning
  there is not settled. The colour word is read
  as a fixed-width value.
- **The convention, by comparison** (an ignored installed test in `ow-cli`, `installed_refskeleton_census`, counts
  only; UModel exports under ignored `local/analysis/refskel/umodel`). UModel is used only as an external binary,
  as `THIRD_PARTY.md` lists it. Twelve packages were compared: Startup, the six class streaming packages, Sanctuary,
  Southern Shelf, Hyperion City, Frost and Ice.
  - **106 meshes (1,398 bones) read, 0 refused, 106 of 106 equal the `.psk` reader's skeleton bone for bone:** names,
    parents, rotations (to 1e-4, a quaternion and its negation counted as one rotation) and positions (to 1e-3).
  - So the stored rotation and position are taken as the bone's bind pose with no conversion, in the `.psk` reader's
    convention (`p_parent = q p q* + t`, game frame). That they are the engine's reference pose is UNVERIFIED.
- **Installed, without any `.psk`** (the existing ignored `installed_mesh_probe`, its export root pointed at a missing
  folder): Maya's `Char_Siren.Mesh.Skel_SirenBody` gets 27 bones from the package through the layer, and all 13 of
  its sockets name a bone of that skeleton.
- **The slice gate** (installed, Off, strict, no `OPENWILLOW_PSK_EXPORTS`, no default export root present): 4 ok / 9
  NotYet / 1 error, the same row for row as the stack's base (fbfcd3d3, run the same way). On that base, link 1 no
  longer reaches the skeleton read. It stops earlier in `maya:8`, at `AnimNodeSpecialMoveBlend`'s interface
  `GearboxFramework.SpecialMoveInterface`, which has no code key (NATIVE_SPECIALMOVE_OWNER.md). Links 3, 4, 5, 13
  and 14 stop at the same place. Link 9's error, the unbound `SpecialMoveComponent.Queue`, is the base's too. So
  this change moves no row today; it removes the UModel dependency for the skeleton once link 1 gets past that node.
- **Sensitive files:** none. `crates/ow-package` and `Cargo.toml` are unchanged (`ow_package::tables::name_text` is
  only called).
- **Verified:** nothing in the running game.
