[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('Push', 'Pull')][string]$Direction,
    # A folder outside this repository (a private repository clone, a synced folder, an external drive).
    [string]$Store = $env:OPENWILLOW_PRIVATE,
    # Repository-relative folders to mirror.
    [string[]]$Folders = @('local/analysis', 'local/notes'),
    [switch]$WhatIf
)
# Copies game-derived working files (analysis databases, hand patches, notes) between this worktree and a private
# store that is NOT part of this repository, so two machines can share them without committing them (docs/NATIVE_ANALYSIS.md,
# "Working on two machines"). Additive only: robocopy /E /XO never deletes and never replaces a newer file with an older one.
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if (!$Store) { throw 'Pass -Store <folder> or set OPENWILLOW_PRIVATE (a folder outside this repository)' }
$store = [IO.Path]::GetFullPath($Store)
if ($store.TrimEnd('\') -ieq $repo.TrimEnd('\') -or $store.StartsWith($repo.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw "The store $store is inside the repository $repo; game-derived files must stay out of it"
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
