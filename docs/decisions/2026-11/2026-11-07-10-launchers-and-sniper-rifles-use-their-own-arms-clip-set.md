## 2026-11-07: launchers and sniper rifles use their own arms clip set (gunlook)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/WEAPON_VISUALS.md` section 22.

- **Behaviour change:** the Walker takes the arms' clip set from the view data's `weapon_anim_set`; a set not imported still falls back to the pistol's.
- **Checked:** two rolled launchers no longer fill the view once the set is imported; placement close to real frames of those types, by eye, on other
  rolls. Local data step per machine. Reload clips and the other arm sets stay open.

