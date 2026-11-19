# The spawn's extent: the bounding-cylinder rule for a template without a cylinder

AI-assisted (Claude Code, Opus 5.5). Lane LINK-9-NEXT, decision 2026-11-19-101 (file 2026-11-19-91). Analyst note only (clean room): written
in own words from local, read-only analysis of `Borderlands2.exe` (`tools/ghidra/run.ps1` queries and the local
table reader, one headless run at a time under the machine lock), with field names from `tools/ghidra/class_layout.py`.
Raw output stays under ignored `local/analysis/link9/` and `local/analysis/spawnspot/`. There is no bl-sdk session
and no recorded live evidence, so **every rule here is UNVERIFIED** in the running game. This lane does not implement
it; another lane will.

It answers open question 3 of NATIVE_SPAWN_PLACEMENT.md, "the bounding-cylinder rule for a template without a
cylinder collision component". Section 2 of that note gives the extent when the template's `CollisionComponent` is a
cylinder.

**Why it matters.** Slice link 9 (`phaselock_enemy`) stops at its `spawn` step with "the actor's bounding-cylinder
rule". The cast spawns `WillowPlayerPawn` from its class default, and that template's `CollisionComponent` is None (a
local run showed it). So the spot search needs the other branch.

## 1. Where the rule sits

The spawn's extent is computed from the template (the archetype, or the class default object):
1. **The template's `CollisionComponent` is a cylinder component:** (`CollisionRadius`, `CollisionRadius`,
   `CollisionHeight`), as NATIVE_SPAWN_PLACEMENT.md section 2 says.
2. **Otherwise:** the template's class's bounding-cylinder rule, an engine method each class may replace. It gives a
   radius and a half height, used as (radius, radius, half height).

The rule's body was read for `Actor`, `Pawn`, `WillowPawn`, `WillowAIPawn` and `WillowPlayerPawn`. The four pawn
classes share one body, `Pawn`'s. Other classes were not read; they may keep `Actor`'s body or have their own.

## 2. `Actor`'s rule: the components' bounding box

1. **The box.** The union of the bounds boxes of the template's components that qualify. A box is its bounds' origin
   plus and minus its box extent. A component qualifies when, in order:
   - it is an entry of the actor's `Components` array that is not None;
   - it is a primitive component;
   - it is attached (`bAttached`);
   - it collides (`CollideActors`; the caller asks for colliding components only);
   - its skip flag is clear. That is the same collision-flag bit the trace filters skip on (NATIVE_SPAWN_PLACEMENT.md
     section 7); its name is still unresolved. The layout tool calls that bit `BulletListener`, which does not fit its
     use, so the name is not used here.
2. **No qualifying component:** the box stays all zeros.
3. **The cylinder:**
   - the half height is half the box's height;
   - the radius is the length of the horizontal half-diagonal, √((width / 2)² + (depth / 2)²), where the width and
     depth are the box's X and Y sizes.

So a template with no attached colliding primitive has radius 0 and half height 0.

## 3. The pawns' rule (`Pawn`, `WillowPawn`, `WillowAIPawn`, `WillowPlayerPawn`)

1. When the pawn's `CylinderComponent` is set, is not the same object as its `CollisionComponent`, and is a cylinder
   component: its `CollisionRadius` and `CollisionHeight`.
2. Otherwise: `Actor`'s rule (section 2).

## 4. What link 9 meets

The cast's template is `WillowPlayerPawn`'s class default:
- In the installed packages, neither `WillowPlayerPawn`'s, `WillowPawn`'s nor `Pawn`'s class default has a cylinder
  component subobject. Only other classes' defaults have one, a matinee pawn and a pawn-only trigger among them.
- The template's `CollisionComponent` and `CylinderComponent` are both None. So section 3 falls to `Actor`'s rule.
- Its `Components` hold six entries. A class default is never attached to a world, so `bAttached` is clear on each
  (read: it is a runtime field that starts clear; UNVERIFIED for these templates). No component qualifies, and the
  extent is (0, 0, 0).
- A zero extent makes the spot search's first test a zero-extent box test (NATIVE_SPAWN_PLACEMENT.md section 3,
  steps 1 and 2, and section 7's `BlockZeroExtent` rule).

## 5. Off answers, step by step (for an implementer)

The rule reads only; it writes nothing, so nothing is preflighted for it and nothing is rolled back. Its answers come
before the spawn's first write, so a refusal here is clean (no prefix).
1. **A cylinder `CollisionComponent`:** as today.
2. **The template's class is exactly one of the four pawn classes:** section 3.
3. **The template's class is exactly `Actor`:** section 2.
4. **Any other class:** `NotYet { NotRead }`, "the bounding-cylinder rule of <class>". The body was read for the five
   classes of section 1 only, and an implementer may not assume that another class keeps its parent's body.
5. **Section 2's inputs**, read from the template's fields:
   - `Components`;
   - each entry's class (a primitive component) and its `bAttached` and `CollideActors`;
   - the skip bit (the trace filters' unresolved bit);
   - `Bounds` (origin and box extent).

   A field the classes do not declare is bad data, a typed error.
6. **The result:** (radius, radius, half height). A zero extent is a valid extent.

**The next stop it would give link 9.** With the extent answered (0, 0, 0), the spot search runs its first test. The
cast's world has no collision service, so the search answers "the spawn placement spot search" `NotYet { LayerAbsent }`
(NATIVE_SPAWN_PLACEMENT.md section 6, step 3), as main does today, unless the cast's world registers one. Main's order
checks the service before the extent; #477's checks the extent first. Either way link 9 then stops at the collision
service.

## Open questions

1. Which other classes replace the bounding-cylinder rule (vehicles, projectiles and the game's own actors were not
   read).
2. The skip bit's name (shared with NATIVE_SPAWN_PLACEMENT.md section 7).
3. Whether any class default has a component with `bAttached` set, which would give a non-zero box.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

1. **A pawn class default.** Read `WillowPlayerPawn`'s class default: its `CollisionComponent`, its `CylinderComponent`
   and, for each entry of `Components`, its class, `bAttached`, `CollideActors` and `Bounds`. Expect both collision
   fields None and every `bAttached` clear.
2. **A spawn at a blocked point.** Spawn a `WillowPlayerPawn` from its class default with `bNoCollisionFail` false,
   inside world geometry, then at a free point. Expect the outcome of a zero-extent (point) search: a point inside solid
   fails, a free point keeps its location.
3. **A pawn with its own cylinder.** Spawn a pawn from an archetype whose `CylinderComponent` is set and is not its
   `CollisionComponent`. Expect the moved location to fit that cylinder's radius and height.
