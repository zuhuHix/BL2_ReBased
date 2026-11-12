## 2026-11-07: ADS on iron sights: a height fit on pistols and rifles, refuted on a shotgun (gunlook)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/WEAPON_VISUALS.md` section 23.

- **Change:** on a pistol or assault rifle with no sight part, the ADS fit adds the height from its eye socket down to its barrel's front sight, in the tool and in `OpenWillowViewTypes::ComposeEntry`. It replaces the slice pistol's own fit, whose height was stale. Fits now go by weapon class: a side term for pistols only. On a machine that regenerates view data, ADS frames move.
- **Stale data is refused:** view entries now carry the tool's schema (2). The host drops a `weapon_view.json` line from another schema, with a warning, and refuses an old table, so every machine and worktree must re-run `weapon_view_model.py`. A refused line composes from a valid table; with both old, its guns lose ADS, recoil and reload data.
- **Why not a fit per weapon type:** the gap changes with the body and barrel, even within one type.
- **Why only those classes:** a real capture of the same parts shows it about 110 px wrong on the slice shotgun, where the eye socket alone was about 12 px off, and right on a second pistol. It is a fit to frames, not the game's rule; the mechanism is unknown. Shotguns and SMGs stay on the eye socket.
- **Checked:** two real pistol frames, one real rifle frame, one real shotgun frame by eye. The class limit: tool against tool and synthetic tests; its frames are still to come. UNVERIFIED on any other gun.
