## 2026-11-25: The game info's refusal after a class code load is marked

AI-assisted (Claude Code, Opus 5.5). Lane GAMEINFO-COLLIDER-CODE-LOAD, stacked on #685 (decision 2026-11-25-20).
A marking fix, not a rule change: the rule is
docs/verification/NATIVE_MAP_LOAD.md section 3's, as `ow-world` `mapload::game_info` codes it, unchanged. Synthetic
fixtures only.

- **The finding (#685):** `spawn_game_info`, with a configured class whose code is cold, loads that class's code in
  its class lookup and then, in the spawn, refuses for collision (a placement search with no collision service). The
  refusal was reported unmarked, because the routine's marking read only #518's three counters, which a code load
  does not move.
- **Why the load stays:** the note has the routine load and cache the configured class before spawning it ("loaded
  once and cached"). The port's class lookup, which stands for that step, also loads the class's code into the VM.
  That code load is a side effect of the host's lookup, not something the note says the game does. The module's
  step-3 rule counts it anyway: from step 3 on, a lazy load counts as a write and marks a later refusal. So the load
  belongs to the lookup the note describes, and the collision check belongs to the spawn, after the lookup. The
  native's behaviour is **UNVERIFIED** in the running game.
  - **Not taken, a resident-only lookup:** the hub suggested this at first. It would refuse the lookup the note has
    the routine make.
  - **Not taken, checking collision first:** this would reorder the note's steps.

  The real defect was the missing mark (the hub agreed).
- **What changes:** this routine's marking, for `NotYet` errors and for the not-found stop, now also counts the VM's
  code loads (`WriteMark::still`). The other `WriteMark` users (the player routine, the bring-up, the mission bridge)
  are unchanged.
- **Tests:**
  - **#685's pinned test** is rewritten as `a_refused_spawn_after_a_class_code_load_is_marked`, un-ignored. With the
    class's code cold, the lookup loads it while #518's three counters stay put, and the collision refusal comes back
    marked "stopped after partial writes" with its `LayerAbsent` kind kept.
  - **The warm case,** `a_refused_spawn_with_the_class_code_loaded_is_unmarked`: with the code loaded first, nothing
    is written or loaded and the same refusal is unmarked, as before.
  - **The existing missing / unfit / refused test's** collision step now expects the mark.
  - **Mutation check:** marking on #518's three only fails both of those tests.
- **Not verified:** nothing in the running game (**UNVERIFIED**). Whether the slice's game class has its code loaded
  when the map load runs is not measured.
