# AI-assisted (Claude Opus 5.5), 2026-11-11. Dot-sourced by the scripts that drive the editor for imports
# (tools/import_phaselock_target.ps1, tools/seed_slice_npc_assets.ps1). They take every path from their own location, so a script
# run through a relative path from another worktree works on that other worktree's data and writes its reports there (one
# 2026-11-10 report landed in the wrong worktree; the cause is a guess). This prints the root and refuses the mismatch.

function Assert-OwCheckout([string]$ScriptRoot) {
    # The checkout the script belongs to, printed, and returned for the caller's paths.
    $repo = [IO.Path]::GetFullPath((Split-Path -Parent $ScriptRoot)).TrimEnd('\')
    Write-Host "Repository root: $repo"
    # The checkout the current directory is in: the nearest folder upwards with a .git entry (a folder, or a worktree's file).
    $folder = (Get-Location).ProviderPath
    while ($folder -and !(Test-Path -LiteralPath (Join-Path $folder '.git'))) {
        $parent = Split-Path -Parent $folder
        if ($parent -eq $folder) { $parent = '' }
        $folder = $parent
    }
    if ($folder) {
        $here = [IO.Path]::GetFullPath($folder).TrimEnd('\')
        $isOurs = $here.Equals($repo, [StringComparison]::OrdinalIgnoreCase)
        if (!$isOurs -and (Test-Path -LiteralPath (Join-Path $here 'host\ue5\OpenWillow\OpenWillow.uproject'))) {
            throw ("The current directory is in another checkout of this repository ($here), but this script works on $repo. " +
                   "Run the script of the checkout you mean (cd there and use tools\...), or cd into $repo first.")
        }
    }
    return $repo
}
