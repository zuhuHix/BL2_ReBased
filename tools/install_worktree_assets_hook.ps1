[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$RepositoryRoot = Split-Path -Parent $PSScriptRoot
$CommonDir = (& git -C $RepositoryRoot rev-parse --path-format=absolute --git-common-dir).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Could not find the shared Git directory.' }
$CommonDir = [IO.Path]::GetFullPath($CommonDir)
$SharedRepositoryRoot = Split-Path -Parent $CommonDir
$SeedRoot = Join-Path $SharedRepositoryRoot 'local\worktree-seed'

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

function Copy-MissingTree([string]$Source, [string]$Destination) {
    if (!(Test-Path -LiteralPath $Source -PathType Container)) {
        throw "Required local asset directory is missing: $Source"
    }

    $SourceRoot = [IO.Path]::GetFullPath($Source).TrimEnd([char[]]@('\', '/'))
    $DestinationRoot = [IO.Path]::GetFullPath($Destination)
    [IO.Directory]::CreateDirectory($DestinationRoot) | Out-Null
    foreach ($File in Get-ChildItem -LiteralPath $SourceRoot -Recurse -File -Force) {
        $RelativePath = $File.FullName.Substring($SourceRoot.Length).TrimStart([char[]]@('\', '/'))
        $Target = Join-Path $DestinationRoot $RelativePath
        if ([IO.File]::Exists($Target)) { continue }
        [IO.Directory]::CreateDirectory((Split-Path -Parent $Target)) | Out-Null
        [IO.File]::Copy($File.FullName, $Target, $false)
        [IO.File]::SetLastWriteTimeUtc($Target, $File.LastWriteTimeUtc)
    }
}

$AssetPairs = @(
    @{ Name = 'Content'; Source = (Join-Path $RepositoryRoot 'host\ue5\OpenWillow\Content'); Seed = (Join-Path $SeedRoot 'Content') },
    @{ Name = 'items'; Source = (Join-Path $RepositoryRoot 'local\items'); Seed = (Join-Path $SeedRoot 'items') },
    @{ Name = 'character'; Source = (Join-Path $RepositoryRoot 'local\character'); Seed = (Join-Path $SeedRoot 'character') },
    @{ Name = 'ui'; Source = (Join-Path $RepositoryRoot 'local\ui'); Seed = (Join-Path $SeedRoot 'ui') }
)

foreach ($Pair in $AssetPairs) {
    Copy-MissingTree $Pair.Source $Pair.Seed
}

$Provisioner = Join-Path $PSScriptRoot 'provision_worktree_assets.ps1'
Copy-Item -LiteralPath $Provisioner -Destination (Join-Path $SeedRoot 'provision_worktree_assets.ps1') -Force

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

$SeedContentFiles = (Get-ChildItem -LiteralPath (Join-Path $SeedRoot 'Content') -Recurse -File -Force).Count
$Manifest = [ordered]@{
    format = 1
    created_utc = [DateTimeOffset]::UtcNow.ToString('o')
    source_worktree = $RepositoryRoot
    source_commit = (& git -C $RepositoryRoot rev-parse HEAD).Trim()
    content_files = $SeedContentFiles
    item_files = (Get-ChildItem -LiteralPath (Join-Path $SeedRoot 'items') -Recurse -File -Force).Count
    character_files = (Get-ChildItem -LiteralPath (Join-Path $SeedRoot 'character') -Recurse -File -Force).Count
    ui_files = (Get-ChildItem -LiteralPath (Join-Path $SeedRoot 'ui') -Recurse -File -Force).Count
}
$Manifest | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $SeedRoot 'manifest.json') -Encoding utf8

Write-Output "Installed shared post-checkout asset seeding at $HookDestination"
Write-Output "Seed cache: $SeedRoot ($SeedContentFiles Content files)"
