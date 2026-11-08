[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('Push', 'Pull')][string]$Direction,
    # A folder outside this repository (a private repository clone, a synced folder, an external drive).
    [string]$Store = $env:OPENWILLOW_PRIVATE,
    # Repository-relative folders to mirror.
    [string[]]$Folders = @('local/analysis', 'local/notes', 'local/ui/colours'),
    [switch]$WhatIf
)
# Copies game-derived working files (analysis databases, hand patches, notes) between this worktree and a private
# store that is NOT part of this repository, so two machines can share them without committing them (docs/NATIVE_ANALYSIS.md,
# "Working on two machines"). Additive only: robocopy /E /XO never deletes and never replaces a newer file with an older one.
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if (!$Store) { throw 'Pass -Store <folder> or set OPENWILLOW_PRIVATE (a folder outside this repository)' }
$store = [IO.Path]::GetFullPath($Store)
# Identity check, not a path-string one: a uniquely named marker file is put in the repository root, and the store
# and every one of its ancestors is asked whether it holds that marker. Any spelling that reaches the repository
# (a junction or symbolic link chain, a UNC share of the checkout, a different case) sees the marker and is refused.
# The marker is always removed; if it cannot be written the script stops (fail closed).
$marker = ".ow_private_sync_marker_$([guid]::NewGuid().ToString('N'))"
$markerPath = Join-Path $repo $marker
try {
    New-Item -ItemType File -Path $markerPath -ErrorAction Stop | Out-Null
    $probe = $store  # full path; GetDirectoryName walks up to the drive or share root, then gives null
    while ($probe) {
        if (Test-Path -LiteralPath (Join-Path $probe $marker)) {
            throw "The store $store is inside the repository $repo (its folder $probe is the repository root); game-derived files must stay out of it"
        }
        $parent = [IO.Path]::GetDirectoryName($probe)
        if (!$parent -or $parent -eq $probe) { break }
        $probe = $parent
    }
} finally {
    Remove-Item -LiteralPath $markerPath -Force -ErrorAction SilentlyContinue
}
New-Item -ItemType Directory -Force -Path $store | Out-Null
$failed = $false
foreach ($folder in $Folders) {
    $relative = $folder -replace '/', '\'
    if ([IO.Path]::IsPathRooted($relative) -or $relative -match '(^|\\)\.\.(\\|$)') { throw "Folder must be repository-relative: $folder" }
    $local = Join-Path $repo $relative
    $remote = Join-Path $store $relative
    $from, $to = if ($Direction -eq 'Push') { $local, $remote } else { $remote, $local }
    if (!(Test-Path -LiteralPath $from)) { Write-Output "skip (missing): $from"; continue }
    New-Item -ItemType Directory -Force -Path $to | Out-Null
    $arguments = @($from, $to, '/E', '/XO', '/R:1', '/W:1', '/NP', '/NFL', '/NDL')
    if ($WhatIf) { $arguments += '/L' }
    robocopy @arguments | Select-Object -Last 8
    # 0-7 are success codes (files copied, extras, mismatches); 8 and above are failures.
    if ($LASTEXITCODE -ge 8) { Write-Error "robocopy failed ($LASTEXITCODE) for $folder"; $failed = $true }
}
exit ($(if ($failed) { 1 } else { 0 }))
