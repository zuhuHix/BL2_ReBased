# Worktree-local Unreal assets

UE imports and runtime recipes are generated locally and ignored by Git. A new
worktree therefore needs a local seed before its level, Maya assets, inventory
recipes, and skill UI can load.

Run `tools/install_worktree_assets_hook.ps1` once from a populated worktree, and again after a change to the provisioner or its folder
list (it refreshes the seed's copies and refills the seed; see [docs/TOOLING.md](../docs/TOOLING.md) "What a new worktree gets").
It keeps a seed under the main checkout's ignored `local/worktree-seed/` and
installs a shared Git `post-checkout` hook. The hook provisions these folders
into new worktrees before `git worktree add` returns: `host/ue5/OpenWillow/Content/` and the `local/` folders the suites read
(the default list, with what reads each, is in [docs/TOOLING.md](../docs/TOOLING.md) "What a new worktree gets").

The UI seed must hold every converted movie the overlay loads, including `local/ui/run/UI_Inworld/` (the world pickup
card); `python tools/check_overlay_data.py` lists what a worktree lacks. Links: `serve.py` resolves both its movies folder and
each file, and answers 404 for a file that resolves outside the folder. So a junction at `local/`, `local/ui` or `local/ui/run` is fine
(the folder resolves too and everything stays inside it), and a junction inside `local/ui/run`, such as `UI_Inworld` pointing elsewhere, is
a 404 like a missing file. The check follows the same rule. Use real copies inside the folder. The provisioner itself refuses a junction or symlink on its destination or source path, so
a junction at `local/` or `local/ui` is filled by hand, not by the provisioner.

The UI seed includes the generated static weapon previews at
`local/ui/run/previews/`. Rebuild them from the provisioned UModel glTF files
with `python tools/render_weapon_previews.py` if they are missing.

Use `tools/run_ue_flash_hud.ps1` to launch Maya with the imported StatusMenu.
Inventory opens with **Tab** (or **I**) and Skills with **K**; the inventory movie is on by
default.

Provisioning copies missing files only, so it preserves worktree-specific
generated assets. To repair a worktree manually, run
`tools/provision_worktree_assets.ps1 -WorktreeRoot <worktree-path>`.

The shared seed is only as fresh as the last time it was refreshed. For a worktree that continues earlier work, copy everything from the most
recent populated worktree instead (Content plus every `local/` folder: slice data, weapon tables, real-game
captures, Phaselock data, analysis notes). Existing files are kept:

```powershell
tools/provision_worktree_assets.ps1 -SourceWorktree <previous-worktree-path>
```

What the default seed leaves out is listed in [docs/TOOLING.md](../docs/TOOLING.md) "What a new worktree gets": `sanctuary`
(add it with `-ExtraSeedFolders sanctuary`), `local/external/` (the UModel exports; the full export is over 10 GB, so copy only
the subfolders a task needs), and the per-run output folders. The suites name the manifest they miss. To fill a worktree from the main checkout without
overwriting anything, either use `-SourceWorktree <main checkout>` above or copy single folders with
`robocopy <main>\local\slice <worktree>\local\slice /E /XO`; otherwise regenerate them in the order given in
[docs/TOOLING.md](../docs/TOOLING.md) "Regenerating local data on a fresh machine".

`build/` is not copied; configure and build it in the new worktree (`cmake -S . -B build -T version=14.50`
on a machine whose UE needs that toolset). Then run the checks in AGENTS.md before regenerating anything:
regenerate only what is missing or older than the code that produces it.

The hook and seed are local to this clone. The scripts are tracked, but no
game-derived assets are added to Git.

Files that cannot be regenerated (an executable analysis database, hand-made
patches) can be shared between machines outside the repository with
`tools/private_sync.ps1`; see "Working on two machines" in
[docs/NATIVE_ANALYSIS.md](../docs/NATIVE_ANALYSIS.md).
