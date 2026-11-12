## 2026-11-03: Regeneration gaps: one schema owner, the whole mission dialog group in the audio manifest, font library placed by the converter

AI-assisted (Claude Opus 5.5). Found by a from-scratch data regeneration on a second PC (main 683a083). Tool-side only; no
host code and no reader parsing changed.

- **Duplicate array-schema key.** `tools/weapon_recipe.py` gained `ValueResolverChain=ObjectProperty` in e767921;
  `tools/slice_values.py` and `tools/loot_pools.py` already declared it and append weapon_recipe's list, so ow-package
  rejected their schema ("duplicate array schema key") and `tools/prepare_slice_world.py` stopped. weapon_recipe owns the
  line (its own evaluator reads the chain); the two copies are removed, and `slice_values.SCHEMA_LINES` now starts from
  `weapon_recipe.SCHEMA_LINES` like the other tools. New CTest `tool-schema-lines` (`tests/schema_lines_test.py`) applies the
  reader's rule to every tool's schema, including the one `slice_values.build` writes at run time; it fails on the old files.
  `prepare_slice_world.py` without the regeneration agent's shim: 21/21 oracles.
- **Audio manifest covers the mission's dialog group, not one run.** The quest suite's `dialog_hook_finds_every_line_and_plays_nothing`
  failed (110/111) because `tools/audio_slice_chain.py` listed only the lines one fixed `--mission-run` reaches (01, 02, 03b
  at 95e48fb), while the host reaches 03a, 04 and 05 on some runs. The check is right (a line the host plays must resolve);
  the tool was too narrow. It now resolves every `DialogEvents` entry of the mission's `MissionDialogGroup` plus any group a
  run names: 35 events, all with an AkEvent, 47 manifest entries, each dialog entry marked `reached_in_scripted_run`. The
  group is shared by the Rock, Paper, Genocide series, so lines of the sibling missions are included (a superset by design).
  The null-`OutputAction` pairing rule stays UNVERIFIED as the engine's rule; the host's independently chosen AkEvent agreed
  with the manifest on every line it reached (`dialog_lines_name_the_manifest_ak_events`). New CTest
  `audio-slice-chain-synthetic`. The tool also makes the reader path absolute (a relative forward-slash path is not found by
  Windows `CreateProcess`, the first failure in the regeneration log).
- **Font library next to each movie.** `gfx_to_swf.py --inline-font-imports` resolves plain `gfxfontlib.swf` imports next to
  `--output`; UI_HUD and UI_StatusMenu use that form as well as `../gfxfontlib.swf`, so without a copy in the movie folder those
  imports were kept and their text fell back to other fonts. The converter now copies the library from the run root when it
  is missing beside the movie (never overwriting) and lists it in the report. Checked on the installed HUD movie: 5 of 5 font
  imports inlined, 2 of them only with the copy. `prepare_inventory_movie.py` stays a separate, documented step (it is a Ruffle
  workaround for one library, not part of conversion).
- **Review round 1 (GPT-6-Luna).** The library copy now happens only for an import named `gfxfontlib.swf` and only to a
  destination inside the movie's output folder; a `..` URL that leaves it is skipped with a warning (listed under
  `skipped_font_library_copies`). Manifest keys stay `dialog:<event tag>` (the host lookup is unchanged), so the tool now
  refuses to write a manifest when one tag occurs in two included groups, and `reached_in_scripted_run` is matched per
  (group, tag). The installed data has no shared tag (35 tags, one group). Both have synthetic tests.
- **Docs.** `docs/TOOLING.md` "Regenerating local data on a fresh machine" (order, UE-step ordering, the `MSYS2_ARG_CONV_EXCL=/Game`
  note), the UI conversion steps, and `tools/worktree-assets.md` (what the seed leaves out and how to fill it).
- **Checks.** CTest 21/21, `verify_packages` 9/9, UE module build; `tools/test_quest.ps1` run twice: first run 111/111 and
  resume 11/11 both times, 9 dialog lookups and 0 misses each. Both runs reached the same six mission lines (01, 02, 03b, 03a,
  04, 05), so the run-to-run variation reported by the regeneration was not reproduced here; the manifest no longer depends on it.
  Host behaviour only; nothing here was compared with the running game.

