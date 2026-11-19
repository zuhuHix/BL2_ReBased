## 2026-11-19: The level navigation join, in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-MAPLOAD-NAVLIST, the navigation lane of
docs/rust-port/SLICE_MAPLOAD.md. Coded from docs/verification/NATIVE_MAP_LOAD.md sections 5 and 6 alone (#536 at
43a75d9c). Clean room: lane ANALYST-MAPLOAD-NAVLIST-PLAYACTOR read the executable; this lane did not open it or the
analysis output. Synthetic fixtures only. No live session was run.

- **What changes (`ow-world` `world::nav_join`, new; wired in `world.rs`).** A level's navigation, cover and pylon
  lists joined onto the world's `WorldInfo`, Off only (note 5.3):
  - `join_level_lists(vm, world_info, &LevelLists)`:
    - a write-free preflight of all three kinds runs before any splice. A null pair is a no-op for its kind. These are
      refused: a half-null pair; a walk that misses its end, links on past it, loops, or passes through an entry already
      in the world's list of the same kind; an entry not of the kind's class (`NavigationPoint`, `CoverLink`, `Pylon`);
      a world list that loops;
    - a level whose start is already in the world's list is a second join, a no-op for that kind (a port answer). A
      level some of whose set kinds are joined and others not is refused;
    - then the splices run in the order navigation, cover, pylon: `end.<link> = head`, then `head = start`, on
      `nextNavigationPoint` / `NavigationPointList`, `NextCoverLink` / `CoverList`, and `NextPylon` / `PylonList`;
    - membership is per kind, so one actor may be in two kinds;
    - every refusal is `NotYet { NotRead }`, raised before any write, so none is marked. Malformed shapes are
      `VmError::Script`. Under Reference the join is `NotYet { NotPorted }` (the C++ reference has none).
  - `level_list_pairs(vm, level)` is always `NotYet { NotRead }`: "the level tail's navigation, cover and pylon list
    pairs (their position after the URL is not decoded)". No structural locator is known, and no offset is guessed
    (rule 4).
  - `join_level(vm, world_info, level)` is the pairs read, then the join. So today it stops at that refusal.
- **A port choice against the note's game reading.** The note reads the game's join as skipping a kind unless both
  ends are set. The port refuses a half-null pair instead, as the note's proposed rule and the hub's brief say,
  because it is damaged data.
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
