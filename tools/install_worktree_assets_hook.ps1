[CmdletBinding()]
param()

# Installs or upgrades the worktree seed: refreshes the cached provisioner and folder list in the seed, fills the seed from
# this checkout (the provisioner's own fill mode: the same folders, exclusions and checks), and installs the post-checkout hook.
# Rerun it after pulling a change to tools/provision_worktree_assets.ps1 or tools/worktree_seed_folders.txt.
$ErrorActionPreference = 'Stop'
$RepositoryRoot = Split-Path -Parent $PSScriptRoot
$CommonDir = (& git -C $RepositoryRoot rev-parse --path-format=absolute --git-common-dir).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Could not find the shared Git directory.' }
$CommonDir = [IO.Path]::GetFullPath($CommonDir)
$SharedRepositoryRoot = Split-Path -Parent $CommonDir
$SeedRoot = Join-Path $SharedRepositoryRoot 'local\worktree-seed'

function Test-Reparse([string]$Path) {
    $Item = Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
    return ($null -ne $Item -and (($Item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0))
}

# Validate the whole seed path before anything is created or written: the cached copies, the fill and the manifest all come after this.
foreach ($Part in @((Join-Path $SharedRepositoryRoot 'local'), $SeedRoot)) {
    if (Test-Reparse $Part) {
        throw "Refusing to seed through a junction or symlink: $Part. The seed must be a real folder (nothing was written)."
    }
}

$GitBashPath = Join-Path ${env:ProgramFiles} 'Git\bin\bash.exe'
if (!(Test-Path -LiteralPath $GitBashPath -PathType Leaf)) {
    $GitBashCommand = Get-Command git.exe -ErrorAction SilentlyContinue
    if ($GitBashCommand) {
        $GitInstallRoot = Split-Path -Parent (Split-Path -Parent $GitBashCommand.Source)
        $GitBashPath = Join-Path $GitInstallRoot 'bin\bash.exe'
    }
}
if (!(Test-Path -LiteralPath $GitBashPath -PathType Leaf)) {
    throw 'Git for Windows Bash was not found. The Windows bash.exe alias is not Git Bash.'
}

[IO.Directory]::CreateDirectory($SeedRoot) | Out-Null

# 1. The cached copies first, so the seed fill and every later hook run use this version of the provisioner and list.
$Provisioner = Join-Path $PSScriptRoot 'provision_worktree_assets.ps1'
$SeedProvisioner = Join-Path $SeedRoot 'provision_worktree_assets.ps1'
Copy-Item -LiteralPath $Provisioner -Destination $SeedProvisioner -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'worktree_seed_folders.txt') -Destination (Join-Path $SeedRoot 'worktree_seed_folders.txt') -Force

# 2. Fill the seed from this checkout, through the cached provisioner's fill mode (missing files only, never overwritten).
& $SeedProvisioner -FillSeedFrom $RepositoryRoot -SeedDirectory $SeedRoot
if ($LASTEXITCODE -ne 0) { throw 'Filling the worktree seed failed (see the messages above).' }

# 3. The post-checkout hook.
$HookSource = Join-Path $RepositoryRoot '.githooks\post-checkout'
$HooksDirectory = (& git -C $RepositoryRoot rev-parse --git-path hooks).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Could not resolve the Git hooks directory.' }
if (![IO.Path]::IsPathRooted($HooksDirectory)) {
    $HooksDirectory = Join-Path $RepositoryRoot $HooksDirectory
}
$HooksDirectory = [IO.Path]::GetFullPath($HooksDirectory)
[IO.Directory]::CreateDirectory($HooksDirectory) | Out-Null
$HookDestination = Join-Path $HooksDirectory 'post-checkout'

if (Test-Path -LiteralPath $HookDestination) {
    $ExistingHook = [IO.File]::ReadAllText($HookDestination)
    if ($ExistingHook -notmatch 'OPENWILLOW_WORKTREE_ASSETS') {
        throw "An existing post-checkout hook was left untouched: $HookDestination"
    }
}
Copy-Item -LiteralPath $HookSource -Destination $HookDestination -Force

$env:OPENWILLOW_HOOK_PATH = $HookDestination
$HookUnixPath = (& $GitBashPath -lc 'cygpath -u "$OPENWILLOW_HOOK_PATH"').Trim()
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($HookUnixPath)) {
    throw "Could not convert the hook path for Git Bash: $HookDestination"
}
$env:OPENWILLOW_HOOK_UNIX_PATH = $HookUnixPath
& $GitBashPath -lc 'chmod +x "$OPENWILLOW_HOOK_UNIX_PATH"'
if ($LASTEXITCODE -ne 0) { throw "Could not make the hook executable: $HookDestination" }
Remove-Item Env:\OPENWILLOW_HOOK_PATH
Remove-Item Env:\OPENWILLOW_HOOK_UNIX_PATH

# 4. A manifest of what the seed holds (informational; nothing reads it).
$Folders = [ordered]@{}
foreach ($Name in @('Content') + @(Get-Content -LiteralPath (Join-Path $PSScriptRoot 'worktree_seed_folders.txt') -Encoding UTF8 |
        ForEach-Object { ($_ -replace '#.*$', '').Trim() } | Where-Object { $_ })) {
    $Dir = Join-Path $SeedRoot $Name
    $Folders[$Name] = if (Test-Path -LiteralPath $Dir -PathType Container) { (Get-ChildItem -LiteralPath $Dir -Recurse -File -Force).Count } else { 0 }
}
$Manifest = [ordered]@{
    format = 2
    created_utc = [DateTimeOffset]::UtcNow.ToString('o')
    source_worktree = $RepositoryRoot
    source_commit = (& git -C $RepositoryRoot rev-parse HEAD).Trim()
    files = $Folders
}
$Manifest | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $SeedRoot 'manifest.json') -Encoding utf8

Write-Output "Installed shared post-checkout asset seeding at $HookDestination"
Write-Output "Seed cache: $SeedRoot (Content: $($Folders['Content']) files)"
