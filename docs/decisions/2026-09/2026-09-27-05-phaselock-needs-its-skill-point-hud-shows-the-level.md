## 2026-09-27: Phaselock needs its skill point; HUD shows the level

Two host gaps from the Skills work:
- `AOpenWillowWalker::UsePhaselock` now returns unless the action skill has a
  grade. The trace shows the game selling Phaselock for one point in the
  Skills tab, and its tree skills stay locked until then. What the game does
  when the key is pressed before that is UNVERIFIED; the host does nothing.
  `-owcombattest` starts at level 5 or higher and spends that point, so the
  scripted combat run still casts.
- The HUD's XP-bar caption fields are `p1.levelClassMod.level` and `.comm`
  (next to the movie's localized "LV"), found by listing the clip in Ruffle.
  The game's writes to them were not recorded in the trace. `owHud` now takes
  `levelText` and `classModText`, and the host sends its level. A headless Edge
  capture shows "LV 45" beside a 42% bar, in the style of the public BL2
  screenshot "LV 17 Hoarding War Dog". No class mod exists in the host, so the
  title stays empty.

Checked: UE 5.8 module build; `OpenWillow.Skills` passes; headless `-nullrhi`
runs log "level 5, 0 skill points, action grade 1" plus a Phaselock cast for
`-owcombattest`, and "level 45, 41 skill points, action grade 0" for
`-owlevel=45`. A key press on F before spending was not tested (no input was
sent to the machine); the maintainer should confirm it by hand.

