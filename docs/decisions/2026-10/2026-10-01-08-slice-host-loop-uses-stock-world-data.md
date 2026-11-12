## 2026-10-01: slice host loop uses stock world data

AI-assisted. Host and executor work; no package parsing change. Nothing here is compared against the original game.

- `-owquest` now reads world, NPC and audio data from ignored manifests (`-owslice`, `-ownpcs`, `-owaudio`).
  Marcus is a placed NPC whose walk is started by the installed Kismet; each move-node arrival re-enters the
  same sequence, so the door opens and closes through the installed links rather than host calls. The range
  objective uses the stock waypoint cylinder. The stock dummy spawns at its population point, the map's
  populated events attach it to the target carrier, its own behaviour events play and reverse the target
  Matinee, and the installed Destroy op removes it. Maya's health uses the recovered formula and respawn the
  decoded station selection. Dialog is looked up and logged, never played.
- Executor additions: `Kismet::eventsForOriginator`, `Mover::sequenceEvent` / `originatorEvent` / `output` /
  `advanceSequence` / `variables`, CLI `--kismet-run ... --originator <object-path>` (synthetic test case 13).
- Two host conventions, both UNVERIFIED: Matinee pose = Key(t) x Key(0)^-1 x placed pose (identical to the
  door's formula when the first rotation key is zero; chosen for the target on screenshot evidence only), and
  a door that receives the opposite request while moving turns around from where it is.
- Checks: quest suite 37/37 and resume 7/7, door suite 16/16, CTest 10/10, packages 9/9. Two earlier quest runs
  failed (35/37, 36/37) and are kept in the record. Not verified against the original game: the navmesh first
  leg of Marcus's walk (a straight line here), the dummy's spawn trigger and event order, touch semantics,
  station activation, mesh hit volumes, the dummy's health. Hand play is launched with `tools/run_quest.ps1`.
  Record: `docs/verification/SANCTUARY_RPG_MISSION.md`, "Host loop with stock world data".

