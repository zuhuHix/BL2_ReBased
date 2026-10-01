# Worktree-local Unreal assets

UE imports and runtime recipes are generated locally and ignored by Git. A new
worktree therefore needs a local seed before its level, Maya assets, inventory
recipes, and skill UI can load.

Run `tools/install_worktree_assets_hook.ps1` once from a populated worktree.
It keeps a seed under the main checkout's ignored `local/worktree-seed/` and
installs a shared Git `post-checkout` hook. The hook provisions these folders
into new worktrees before `git worktree add` returns:

- `host/ue5/OpenWillow/Content/`
- `local/items/`
- `local/character/`
- `local/ui/`

The UI seed includes the generated static weapon previews at
`local/ui/run/previews/`. Rebuild them from the provisioned UModel glTF files
with `python tools/render_weapon_previews.py` if they are missing.

Use `tools/run_ue_flash_hud.ps1` to launch Maya with the imported StatusMenu.
Inventory opens with **Tab** (or **I**) and Skills with **K**; the inventory movie is on by
default.

Provisioning copies missing files only, so it preserves worktree-specific
generated assets. To repair a worktree manually, run
`tools/provision_worktree_assets.ps1 -WorktreeRoot <worktree-path>`.

The hook and seed are local to this clone. The scripts are tracked, but no
game-derived assets are added to Git.

Files that cannot be regenerated (an executable analysis database, hand-made
patches) can be shared between machines outside the repository with
`tools/private_sync.ps1`; see "Working on two machines" in
[docs/NATIVE_ANALYSIS.md](../docs/NATIVE_ANALYSIS.md).
