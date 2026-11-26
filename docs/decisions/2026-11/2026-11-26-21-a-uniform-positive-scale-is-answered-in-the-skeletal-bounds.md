## 2026-11-26: A uniform, positive scale is answered in the skeletal bounds

AI-assisted (Claude Code, Opus 5.5). Lane SCALED-BOUNDS-TRANSFORM, stacked on #761 (fix/rust-link67-pickup-gun-init @
4062b4ab), from main e947919. It implements the hub's ruling on NATIVE_LINK67_PICKUP_GUN_INIT.md's open question 3.
This lane did not read the executable. No session was run.

- **What was decided** (the hub's ruling):
  - `skeletal_bounds`' stored box (source f in NATIVE_COMPONENT_BOUNDS.md, step 8), and so `GetBoundingCylinder`,
    answers a scaled world transform when the combined scale is **uniform and positive**. The combined scale `s` is
    the component's `Scale` × `Scale3D` × the owner's `DrawScale` × `DrawScale3D`, as before.
  - "Uniform" is strict: the three axes are exactly equal, finite and above 0. It is not step 6's `UNIFORM` tolerance,
    so the factor is never a choice between axes.
  - The box is (`Location` + `s` × the mesh origin + `s` × `RootBoneTranslation`, the mesh extent × `s` ×
    `BoundsScale`). That is the standard UE3 bounds transform with no rotation: the root-adjusted local box, scaled,
    then moved to the owner. At `s` = 1 it is the earlier unit-scale rule, term for term.
  - A non-uniform, zero, negative or non-finite scale stays the bounding cylinder's `WORLD_TRANSFORM` `NotRead`. So
    does every other condition of the simplest transform (`Translation`, the Absolute flags, `Owner`, the rotations).
  - **UNVERIFIED:** the port's reading of the engine maths, not read from the native. The code and
    NATIVE_COMPONENT_BOUNDS.md (step 8) say so.
- **Reference:** unchanged. `GetBoundingCylinder` is bound in Off only (the C++ binds none), so the rule is Off-only.
- **The slice gate** (installed, Off, strict, release build, under the machine lock; #761 head 4062b4ab and this
  change): 6 ok / 8 not ok, 47.0% on both. Before: 8 `NotYet`. After: 6 `NotYet` and 2 errors.
  - Links 6 and 7 get past `GetBoundingCylinder`: the gun's `Scale` 0.92 box is answered. `TouchPickupTrace` and the
    pickup query (both bound) run. The links now stop in `touch` at **`WillowPickup.Pickupable_IsEnabled()`,
    unbound**, which the strict grading counts as an error, not a `NotYet`. So the gate exits 1, not 3.
  - That is the stop NATIVE_TOUCH_COMPONENT_TEMPLATE.md predicts (section 5 reads it, unimplemented): the next job.
  - Every other row is unchanged.
- **Tests** (`bounding_cylinder_tests.rs`, invented data):
  - `a_uniform_positive_scale_scales_the_stored_box`: a uniform 0.92 scales the extents, so the radius and the height
    shrink. The factor also comes through the owner's `DrawScale`. A non-uniform `Scale3D` or `DrawScale3D` is the
    `NotRead`, and so are a zero, a negative and a NaN `Scale`.
  - Three tests now pin the new rule instead of the unit-scale one:
    - the main source-f test: `Scale` 2 is answered (the box scaled about the owner), and a non-uniform `Scale3D` is
      the `NotRead`;
    - the parent-source test: a non-uniform `Scale3D` keeps the `NotRead`. Source b with a uniform non-unit scale
      is now the simplest transform, so it answers `PARENT_ROOT` there instead of `WORLD_TRANSFORM` (review of #764,
      item 3);
    - #761's pickup test: the gun's 0.92 is answered, scaled.
- **Verified:** the tests on invented data and the installed rows above. **Not verified:** everything in the running
  game, and the bounds maths against the native.
- **Sensitive files:** none.
