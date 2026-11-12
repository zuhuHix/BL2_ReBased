## 2026-11-07: The use target follows the view on machines, lootables, stations and doors; UNVERIFIED as the game's rule (controls)

AI-assisted (Claude Opus 5.5). Maintainer's decision, through the hub. Record: `docs/verification/CONTROLS.md`, "Use prompt".

- **Change:** a vending machine, a lootable, a Quick Change station or a door is a use target only while a ray from the camera along the view, 350 uu long, enters its box with nothing blocking first (`OpenWillowUseView`). Being in reach behind Maya no longer offers or opens it. The prompt follows, since it asks the same code as the key.
- **Evidence:** the native reading (`NATIVE_USE_INTERACTION.md`, itself unconfirmed), and a real-game lead: standing 120 uu from a usable thing and turning 15 to 30 degrees, the usable went to none on 8 of 11 targets; 3 stayed, and why was not checked. That is a lead, not proof, so the rule stays **UNVERIFIED** as the game's. A door and a strongbox were never read.
- **Found by the first run:** the station's own imported machine blocked the ray, since in the host it is another actor from the station. The rule would have refused the station from every spot in front of it. It now takes the first hit of the whole ray and requires it to be one of that machine's sections, as the native note describes; machines, lootables and doors keep the box. The test had stood her inside the machine's collision, which also let `use_view_blocker` pass its blocked line without the sheet stopping the ray: it now grades what stopped it.
- **The host's own, not the game's:** the boxes stand in for usable collision; the old reaches stay and the press still goes nearest first among what she looks at, where the game takes the first hit along the ray; a station's open prompt also ends on looking away.
- **Left alone:** pickups (the native note gives them their own path, and they win the key) and Marcus (his own ray). A pickup against an interactive in reach was not read.
- **Checked:** the graders' synthetic tests only. The host code was **not compiled or run** when this was written: both machines were held by a batch. The new checks (`use_view_rule`, `quick_change` with `OWQCTEST looked away selected=0 target=none`, and `use_view_blocker`, the same spot with something solid on the ray and without it) have never run.


