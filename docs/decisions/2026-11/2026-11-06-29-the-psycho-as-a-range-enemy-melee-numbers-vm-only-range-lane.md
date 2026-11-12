## 2026-11-06: the psycho as a range enemy, melee numbers VM only (range lane)

AI-assisted (Claude). Record: docs/verification/COMBAT_RANGE.md "The psycho". `tools/prepare_range.py` adds the psycho kind (health, melee and loot chances per level from the VM's
spawn of the stock pawn) and `tools/prepare_phaselock_target.py` an import target for its mesh and clips. It joins the range's spawns only when its own mesh is imported, last in the list.
Its melee numbers have no live row and are labelled VM only, UNVERIFIED in the manifest; its collision cylinder and scale are the archetype's (no live read); its death clip is a choice
among what the set holds. It uses the range's brain, a host stand-in for its stock AI tree. No marauder or nomad: they shoot, and the brain only melees (a separate lane). New test step
`psycho_melee`, which may SKIP without the imported mesh. No parsing change.

