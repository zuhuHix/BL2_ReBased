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
a 404 like a missing file. The check follows the same rule. Use real copies inside the folder. The provisioner refuses a junction or symlink on a destination or source path, so a junction at
`local/ui` is filled by hand. A junctioned `local/` as a whole is a shared data root: the provisioner skips its local folders and
still provisions Content (see [docs/TOOLING.md](../docs/TOOLING.md) "What a new worktree gets").

The UI seed includes the generated static weapon previews at
`local/ui/run/previews/`. Rebuild them from the provisioned UModel glTF files
with `python tools/render_weapon_previews.py` if they are missing.

Use `tools/run_ue_flash_hud.ps1` to launch Maya with the imported StatusMenu.
Inventory opens with **Tab** (or **I**) and Skills with **K**; the inventory movie is on by
default.

Provisioning copies missing files only, so it preserves worktree-specific
generated assets. To repair a worktree manually, run
`tools/provision_worktree_assets.ps1 -WorktreeRoot <worktree-path>`.

A file that already exists is never replaced by default, so a stale seeded copy survives (on 2026-11-10 a worktree kept an
`M_OW_NPC_Zone` older than its newer scalar parameters, and an in-engine run was invalid). Add `-RefreshStale` to replace an
existing file when the source's copy (the seed, `-SourceWorktree` or, with `-FillSeedFrom`, the checkout) is newer and differs.
"Newer" is the file time only, so the old file is not deleted: it is first moved to
`local/worktree-refresh-backup/<UTC time>/<its path in the worktree>` (in seed mode, `<seed>/worktree-refresh-backup/...`), and
the move and the replacement are each printed as they happen, so a failed replacement (a file the editor holds open) still shows
what was done. Every folder line gains a count. A worktree file newer than the source is kept, as is an older file with identical
contents. A target that is itself a symlink or junction is refused before it is read. The backup folder is never provisioned
anywhere; delete it by hand once the refreshed worktree is checked. With a junctioned `local/` (a shared data root) there is no
local backup place, so a refresh that would replace a file is refused. Refresh the seed first
(`tools/install_worktree_assets_hook.ps1`) if the seed is the stale one.

The slice-npc, ambient-npc and Phaselock-target editor jobs (`local/slice/editor_job.json`, `local/slice/ambient/editor_job.json`
and `attach_job.json`, `local/phaselock/target/editor_job.json`) store paths relative to the repository root, so a copied job
works in the worktree it is copied into. `tools/seed_slice_npc_assets.ps1`, `tools/seed_ambient_npc_assets.ps1` and
`tools/import_phaselock_target.ps1` write `<job>.resolved.json` for their own checkout before each editor run
(`Resolve-OwEditorJob` in `tools/checkout_guard.ps1`, which runs `python tools/repo_paths.py resolve-job`). That file holds absolute
paths and is never provisioned. The launcher stops, naming the job, the path and the command that rewrites it
(`python tools/slice_npc_assets.py editor-job`, `python tools/ambient_npc_assets.py editor-job` or `attachments`,
`python tools/prepare_phaselock_target.py editor-job`), for any path, input or output, that:

- climbs out of the repository root (`../other/local`), or is rooted or drive-relative (`\local\x`, `C:foo`);
- really lies in another checkout of this repository, followed through junctions, even when that file still exists;
- is an output outside this checkout, or an absolute input outside every checkout that does not exist.

An absolute input outside every checkout (the game install, an export folder) is used when it exists. A `local/` junctioned to a
plain data folder (a runner's shared data root) is not another checkout and is allowed. The same three scripts print
`Repository root: ...` and refuse to run when the current directory is inside a different checkout of this repository. The
checkouts are compared by physical path, so a current directory that reaches this checkout through a junction is allowed
(`tools/checkout_guard.ps1`). Run the copy of the script that belongs to the worktree you mean.

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
