## 2026-11-09: The magazine fixture makes its own reload after the dummy link (slice audio lane)

AI-assisted (Claude Haiku 5.5). The Z42 run showed the magazine fixture stalls the gate when the magazine holds less than one shot's cost; the fix sets it to at least that cost. The older Z41 run (ShootDummy) fired one shot, and that shot completed the objective, so no later press would reload the magazine it emptied. With the switch on, once the mission_objective link has passed the fixture asks for one reload through the existing key path, holds the ShootDummy step until the reload finishes or 10 s pass (fixture notes only; never a link failure), then moves on. Without the switch the step is unchanged: the link passes once and the step moves on as before.

- **Verified (automated):** the listen-report suite (20 tests), the prepare suite (34), ctest (34), and the UE module build through the slot.
- **Not verified:** in engine. The post-link reload, its start and finish lines, and the gate verdict are unseen.
- **Open (not in this PR):** `ShootAt` stalls when the magazine is above zero but below one shot's cost: it neither fires nor reloads. The no-switch gate never hit it on the runs so far. Needs its own review.

