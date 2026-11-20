## 2026-11-19: The level navigation join, in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-MAPLOAD-NAVLIST, the navigation lane of
docs/rust-port/SLICE_MAPLOAD.md. Coded from docs/verification/NATIVE_MAP_LOAD.md sections 5 and 6 alone (#536 at
d89963fe). Clean room: lane ANALYST-MAPLOAD-NAVLIST-PLAYACTOR read the executable; this lane did not open it or the
analysis output. Synthetic fixtures only. No live session was run.

- **What changes (`ow-world` `world::nav_join`, new; wired in `world.rs`).** A level's navigation, cover and pylon
  lists joined onto the world's `WorldInfo`, Off only (note 5.3):
  - `join_level_lists(vm, world_info, &LevelLists)`:
    - a write-free preflight of all three kinds runs before any splice. A null pair is a no-op for its kind. These are
      refused: a half-null pair; a walk that misses its end, links on past it, loops, or passes through an entry already
      in the world's list of the same kind; an entry not of the kind's class (`NavigationPoint`, `CoverLink`, `Pylon`);
      a world list that loops;
    - a level whose start is already in the world's list is a second join, a no-op for that kind (a port answer). A
      level some of whose set kinds are joined and others not is refused. **That refusal is a port addition:** the
      note's per-kind no-op (5.3, item 4) would join the rest;
    - then the splices run in the order navigation, cover, pylon: `end.<link> = head`, then `head = start`, on
      `nextNavigationPoint` / `NavigationPointList`, `NextCoverLink` / `CoverList`, and `NextPylon` / `PylonList`;
    - membership is per kind, so one actor may be in two kinds;
    - every refusal is `NotYet { NotRead }`, raised before any write, so none is marked. Malformed shapes are
      `VmError::Script`. Under Reference the join is `NotYet { NotPorted }` (the C++ reference has none).
  - `level_list_pairs(vm, level)` is always `NotYet { NotRead }`: "the level tail's navigation, cover and pylon list
    pairs (their position after the URL is not decoded)". No structural locator is known, and no offset is guessed
    (rule 4).
  - `join_level(vm, world_info, level)` is the pairs read, then the join. So today it stops at that refusal.
- **Two port choices against the note's game reading** (both UNVERIFIED):
  - **a half-null pair.** The note reads the game's join as skipping a kind unless both ends are set. The port
    refuses the pair instead, as the note's proposed rule and the hub's brief say. That rests on an assumption, that
    such a pair is damaged data. The cost: a real level with one would stop the bring-up where the game carries on;
  - **cover and pylon without navigation.** Note 5.2 reads the join as running only for a level whose navigation
    start and end are both set. The port joins a level's cover and pylon pairs even when its navigation pair is null,
    each kind on its own, as the note's proposed rule 5.3 has it.
- **Left out** (note 5.2; 5.3, items 6 and 7):
  - the per-point engine call the game's join makes first. It is not read, so a later stop that needs it raises it;
  - the streamed levels' joins and their order relative to the persistent level's (open question 1);
  - taking a level's lists out when it is removed.
- **Who calls it.** Nothing yet. This lane does not touch `mapload.rs` or `bringup.rs` (H6, lanes 2 and 3).
  - **Lane 2 (IMPL-MAPLOAD-GAMEINFO)** calls `ow_world::world::nav_join::join_level(vm, world_info, persistent_level)`
    in the bring-up's begin-play-once step, after the game info's spawn and before the actor initialisation pass
    (SLICE_MAPLOAD.md section 3, step 6; note 5.2, schedule item 1).
  - Until the level tail is decoded, that call is the typed `NotRead` stop of `level_list_pairs`. Lane 2 may hold it
    back until then, so that the bring-up does not stop earlier than today.
  - **Lane 4 (IMPL-SLICE-MAP-WORLD)** drives it through the bring-up, and calls no join of its own.
- **Tests** (`ow-world` `nav_join_tests.rs`, 7, invented classes):
  - a join in front of a non-empty world list, with one actor in two kinds, and a second level in front of the first;
  - null pairs are no-ops;
  - a second join is a no-op, and a partly joined level is refused;
  - every preflight refusal, with nothing spliced for any kind: half-null both ways, a missed end, past the end, a
    loop, the wrong class, a shared entry, a looping world list, and a wrong-class world entry;
  - the pairs read refused;
  - Reference `NotPorted`;
  - a receiver that is not a `WorldInfo`.
- **UNVERIFIED.** Nothing here was checked in the running game. The note's live check 1 (walking
  `NavigationPointList` right after Sanctuary loads) would confirm the join order.
- **Review r1 of #544:**
  - the preflight now also checks the two writes against their fields' declared classes: the level's start against
    `WorldInfo.<head>`, and the world's head (when set) against the end's `<link>`. Before, a narrower-declared field
    (a `CoverList` of a `CoverLink` subclass, say) failed the typed write after the navigation list was already
    spliced, leaving the world half joined with no mark. The refusal is `NotRead`, "a list entry that is not of the
    class its list field declares". The field ids are resolved in the preflight and carried to the splices;
  - `join_level` checks the mode first, so under Reference it is `NotPorted`, not the pairs read's `NotRead`;
  - the half-null refusal, the partly-joined refusal and the cover/pylon divergence are labelled as port choices above;
  - the source is #536 at d89963fe;
  - tests: 9 (added a narrower-declared `CoverList`, which leaves the navigation list unspliced, and pylon-kind
    refusals); `cargo test -p ow-world` 229 passed, 0 failed, 5 ignored; clippy shows nothing in these files.
