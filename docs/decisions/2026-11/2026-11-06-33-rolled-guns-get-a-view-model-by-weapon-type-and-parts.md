## 2026-11-06: rolled guns get a view model by weapon type and parts (gunlook)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/WEAPON_VISUALS.md` section 22.

- **Behaviour change:** a gun without a `weapon_view.json` line gets its view model composed from a table per weapon type and part (new
  `local/weapon_library/view_types.json`, `weapon_view_model.py --library`). Before, it had none: no arms offset, zoom, reload clip or flash layers.
- **Checked:** the table reproduces the generated entries, Python tool against tool on cooked data; the host's composition has synthetic tests. UNVERIFIED in game. Base-game types only.
- **Open:** launcher and sniper arm sets, reload clips by type, and zoom by parts.

