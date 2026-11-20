## 2026-11-19: Actor.GetBoundingCylinder in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-GET-BOUNDING-CYLINDER, slice links 6 and 7. The decision number is
pending (the hub numbers decisions at landing); NNN is a placeholder. Coded from NATIVE_GET_BOUNDING_CYLINDER.md alone,
the note as accepted at 6800f603 (PR #590; clean room: lane ANALYST-GET-BOUNDING-CYLINDER wrote it). The executable and
the analysis output were not opened. Synthetic fixtures only.

- **What changes:** `ow-world` `world::bounding_cylinder` binds `Actor.GetBoundingCylinder(float,float)` and its `Pawn`
  and `NavigationPoint` keys in Off. Reference keeps the stub policy (R3: the C++ binds none). `placement::extent` is
  not changed. The native shares only `box_cylinder`, `PAWN_RULE_CLASSES`, the `SKIP_FLAG` text and refusal C's text
  and record with it, as the note's section 2a and its link 9 paragraph say.
- **The answer, in the note's section 4 order.** Both values are computed first, and the two out arguments are written
  last, so a refusal or a data error writes neither:
  1. **The body, by exact class:**
     - `PAWN_RULE_CLASSES` take the pawn rule;
     - `Engine.NavigationPoint` takes the navigation-point rule;
     - `Engine.Actor` and `WillowGame.WillowPickup` take the actor default;
     - any other class is refusal **C**, main's "the bounding-cylinder rule of <class>".
  2. **Template or live**, from the heap `Origin`, asked only where it is used. `Default` is a template, `Runtime` is
     live, and `Export` or `Resource` is refusal **T**.
  3. **The pawn rule:** a `CylinderComponent` that differs from the `CollisionComponent`, on a template, and set,
     answers its radius and height. Otherwise the actor default.
  4. **The navigation-point rule:** a set cylinder answers. Otherwise the actor default.
  5. **The actor default:** walk the `Components` entries in order.
     - None and non-primitives are skipped, the latter decided from the class the handle carries.
     - A lazy handle is refusal **N**, never a load.
     - `bAttached` is read on a template, and an entry with it clear is skipped. A live receiver refuses **L**; an
       export or resource receiver refuses **T**.
     - An entry with `CollideActors` clear is skipped.
     - The skip flag is refusal **S**, main's `SKIP_FLAG` text.
     - With no entry left the answer is 0 and 0.
- **Nothing is loaded.**
  - Classes are looked up only through the class table's `by_path`.
  - A stand-in with no slots counts as a lazy handle (N).
  - A receiver that is itself a lazy handle is N as well. This is the port's reading of the note's load-free
    contract, which names only components.
- **`WillowPickup` differs from main's extent on purpose (review r1 of #595).** Main's `placement::rule` still
  refuses C (`NotRead`) for a `WillowPickup` template's spawn extent, while this native answers the actor default for
  `WillowGame.WillowPickup`. Both are right on their own notes: NATIVE_SPAWN_EXTENT.md read five classes only, and
  the native's note (section 4, step 1) adds `WillowPickup` on its own reading of that class's vtable. Bringing the
  extent's class list into line with it is a known follow-up for a `placement` lane, outside this native's scope (the
  note keeps the two codes apart).
- **The out arguments** are checked before either is written: both must exist and be declared `out` (review r1 of
  #595), so a malformed declaration cannot leave the radius written and the height not.
- **Malformed data** is a `VmError::Script` naming the field, through the world's field reads.
- **Verified:** host unit tests on invented packages, covering the note's tests list. Every refusal leaves every heap
  object and the store's load state unchanged.
  - Two cases on the list cannot be built on these classes: a `Components` that is not an array, and a
    `CollideActors` that is not a bool. The heap refuses those writes.
  - Instead, the tests cover a pawn template's mesh used as its `CylinderComponent`, a script error naming
    `CollisionRadius`.
- **Not verified:** nothing is checked against the running game, so every rule is **UNVERIFIED** in game.
