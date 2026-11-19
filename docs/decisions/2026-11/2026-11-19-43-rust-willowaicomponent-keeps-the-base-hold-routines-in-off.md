## 2026-11-19: Rust WillowAIComponent keeps the base hold routines in Off; its own base kind is NotYet

AI-assisted (Claude Code). Lane FIX-AI-HOLDS-SUBCLASS, the implementer of the second reading of
`docs/verification/NATIVE_AI_HOLDS.md` (lane LINK-1-NEXT, decision 2026-11-19-31; read at c78265f2 on
`research/rust-link1-next`, which had not landed when this lane started). It was coded from the note alone. There was no
real-game session, so every rule is **UNVERIFIED** in the running game. No dependency was added and no bounds check was
loosened. The Pawn IAIInterface refusal, `GetAIComponent`'s refusal and the release-order refusal are untouched.

- **The note's finding.** `WillowAIComponent` keeps `AIComponent`'s routines for the target, movement, demigod, god and
  dialog kinds. It replaces only the base kind (`Hold`, `Release`, `OnHold`, which it declares itself), and the
  replacement was not read.
- **`ow-combat` (`holds.rs`, Off).** The movement, demigod, god and dialog natives now accept an exact
  `WillowAIComponent` as well as an exact `AIComponent`. Before, Off refused every class but the exact `AIComponent`
  ("AI hold virtual overrides ... unread"). Any other subclass is still refused: its table was not read.
- **`ow-ai` (`awareness/actions.rs`).** `WillowAIComponent`'s own `Hold`, `Release` and `OnHold` are now
  `NotYet { NotRead }` in Off, and nothing changes when they are refused.
  - Before, they were bound to the base kind's rule in both modes (decision 2026-11-16-66), which the note now shows
    the class replaces with an unread routine. This adds a refusal in Off where there was an answer; it moves no
    existing guard the other way.
  - Reference keeps the base rule, as before.
  - The target kind (`HoldTarget` and the rest) already reached `AIComponent`'s natives on a `WillowAIComponent`, which
    the note confirms.
- **Tests.**
  - `ow-combat` `holds::tests`:
    - `willow_components_keep_the_four_kinds_routines`: an inherited kind passes on a `WillowAIComponent`;
    - `subclass_virtuals_are_refused_for_all_kinds_before_mutation` now uses another invented subclass, still refused.
  - `ow-ai`: `willow_base_hold_kind_is_not_yet_in_off` (the base kind refused in Off with nothing changed; the base rule
    under Reference).
  - **Seven tests repinned.** They pinned Off's old stop text on the shared combat fixture: five in `ow-combat`
    (`hit::tests::off_reads_the_willow_components_demigod_list`, two `damage` tests, two `loot` tests) and two in
    `ow-cli`'s `cmd/combat/tests.rs` (`off_mode_stops_before_undeclared_attributes_at_the_willow_hold`, and
    `off_mode_reads_the_willow_demigod_list`, renamed from `off_mode_refuses_unread_willow_hold_virtuals`). Off now reads
    the Willow component's `DemigodHolds`, and the fixture's toy `AIComponent` declares no such list, so they stop at
    the typed error "AIComponent has no DemigodHolds array". The fixture was not changed, because `ow-cli` pins it
    against the C++ digests. The rule itself is covered by `holds::tests`.
- **Installed, before and after** (pc-hummer, Off, both on main 2981b801; outputs under ignored
  `local/p2/fix-ai-holds-subclass/`). All rows are identical before and after.
  - `--combat-run`, the kill steps of COMBAT_DEATH.md, S-f, and a dummy hit (`spawn:dummy:8 player:8
    hit:0:10::Bullet`): every Off spawn now stops earlier, at `WillowPawn.SetDefaultPhysics` past its gates
    (FIX-MIND-ROUTE, decision 2026-11-19-32), or at the Targetable resolver. So no hit reaches the hold on main, and the
    hit cannot be shown reaching damage here.
  - `--slice-gate`: strict 0/11/3, past-errors 0/14/0. Links 3 and 13 stop where link 1 does, at `maya:8` (the
    controller's missing `PlayerClass`, NATIVE_PAWN_DATA.md), blocked behind links 1 and 4.
  - What this lane removes is the hold refusal that the dummy's and the bullymongs' hits would meet once their spawns
    pass. Their `DemigodOnHold` is then the base answer (any entry in `DemigodHolds`).
- **Open:**
  - `WillowAIComponent`'s base-kind replacement (Off `NotYet`);
  - the readers of `DemigodHolds` and `GodHolds` on the damage path (NATIVE_AI_HOLDS.md, "Not read");
  - the spawn's `SetDefaultPhysics` stop, which stands in front of every installed hit in Off.
