## 2026-11-13: Range bullymongs leap and throw rocks

AI-assisted (Claude Code). Lane N of the hub run of 2026-11-13, the implementer half of a clean-room pair: built from the own-words
note `docs/verification/NATIVE_AI_LEAP_THROW.md` and the records it cites, with no access to the analyst's material or the executable.
Until now the range's enemies only swung (`COMBAT_RANGE.md`, acceptance row 9, "not modelled"); the den reads saw most first blows
come from leaps and rocks.

- **What the host does now.** Each bullymong kind has a row (`OpenWillowLeapThrow.h`): its close band, the random node's weights, its
  leap and its throw, with the note's bands, speeds, angle fractions, lead caps, resources and clip times. Inside the close band the plain
  swing runs; outside it, one weighted draw per finished attack (adult 1.0 : 0.25 : 0.25, regular 1.0 : 0.5), a drawn attack that
  cannot run (band, line of sight, Maya in front for the throw, a free resource) dropping out. A leap winds up for its launch clip,
  flies the note's arc (fixed speed, launch angle from the angle fraction, capped lead, gravity solved to land on the aim point) and
  lands with the radius-leap volume (320 around a point 200 ahead); a throw releases a rock at the clip's release moment on the same
  solver (speed 2000, fraction 0.3), and the rock damages Maya on a touch. Both deliver the plain melee's numbers (COMBAT_MELEE: the
  leap's delivery equals the swing's) and push Maya: 100,000 for the leap, 50,000 for the rock.
- **The hub's seven decisions, all kept.** (1) The smaller chooser above, a host stand-in for the stock tree; the draw's stream is
  seeded per enemy (`-owrangeaiseed=N`, default 20261113, enemy i draws from N + i). (2) Bands are horizontal enemy-to-Maya distances
  with line of sight, no target-size allowance; the throws' move limit is not modelled. (3) One leaper and three throwers at a time in
  the range's one den. (4) The adult's roar is skipped and logged (`roar=skipped_stand_in`). (5) One solver, `OpenWillowSolveArc`,
  for leaps and rocks; `OpenWillow.LeapThrow` reproduces the note's worked table. (6) The leap's landing volume and the rock's touch
  hit with the note's damage rule and knock-back. The momentum becomes an added horizontal velocity of momentum / 100 through the
  character's launch (a mass of 100 is a stand-in); that adds 1,000 uu/s for a leap and 500 for a rock, and Maya's own braking then
  stops her within about 60 and 30 uu (worked out from her ground braking, not measured), well short of the den-down's "about 500 uu" (one run). Not added to the plain swing. (7) The
  clip times of note 3.1 and 4.1; the host's Attack moment is the start of the launch clip, so its Attack-to-landed is the launch clip
  plus the flight (1.34 to 2.10 s), inside L-4's 1.08 to 3.48 s.
- **Other stand-ins (UNVERIFIED):** a leaper that comes down over another enemy is set on top of that enemy's hit capsule (the landing's
  floor trace is a Visibility trace that ignores only the leaper; the note says nothing about it; not fixed); the injured gate is not modelled: range enemies have no injured state (the note's injured adult
  neither roars nor leaps); the back-up throws never run (the host's melee child can always walk in); the slinger's high-ground
  throw is not modelled (no raised spots); the baby leaps whenever it is outside its grab band and inside its leap band; the slinger and
  the baby walk in for the swing when they can run nothing else; a rock leads (the row's cap) or not on an even draw per throw (what picks
  one is not decoded); the landing's vertical test reads DamageHeight 192 as either way of the leaper's centre; a leaper that comes down
  inside Maya steps back to body contact (the host's enemies do not collide with her); the rock is an engine sphere of touch radius 20
  that ignores other enemies; with no leap or throw clips in the import, the body holds its idle; the rock leaves `R_Hand_Rear` when the
  mesh has it, else a point in front of the chest; a leap or throw is atomic once started (a hold, death, AI off or the target lost
  before take-off or a release ends it).
- **Tests.** `kind_melee` measures the plain swing, so it now turns its enemy's leaps and throws off (a test fixture): at 600 uu Maya
  lies inside the adult's throw band. After review r1 the shared reset of the other AI steps does the same for enemy 0, because
  `approach` (Maya 2,000 uu ahead, inside the regular's leap band) had graded a leaper in the air as a chase; COMBAT_RANGE.md lists
  why each other step is not changed by it. Two new range steps, `leap` and `throw`, ask each kind's first enemy for its attack with Maya
  inside the band and ahead, and `tools/check_range.py` grades the band, the launch clip and the release moment (up to one frame late),
  the arc against its own copy of the solver, the landing on the aim point, the hit, the damage and the knock-back, one result per
  spawned kind, and checks each enemy's logged row against the note's. A new automation test brings `ue_automation` to 14 tests; the
  suite's pass line in `tools/suites.txt` moves with the launcher's count, which `tests/automation_paths_test.py` requires.
- **Verified:** the host's arithmetic against the note's own table and the checker's synthetic cases; in the host (zuhu desktop,
  2026-11-13), the range suite 39 / 0 with the leap and throw rows inside the rule, `ue_automation` 14 / 14, the slice gate plain,
  `-Shield` and `-DownAfterFight` 14 / 0 / 0, the kill row PASS. `-Unforced` failed `bleed_out_respawn` once on the run's respawn count, with every down in
  that run following a plain swing (`COMBAT_RANGE.md`, "First runs"). Nothing here was compared with the running game; every rule above
  stays UNVERIFIED in game.
