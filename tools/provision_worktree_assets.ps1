[CmdletBinding()]
param(
    [string]$WorktreeRoot
)

$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($WorktreeRoot)) {
    $WorktreeRoot = (& git -C $PSScriptRoot rev-parse --show-toplevel).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Could not find the current Git worktree.' }
}

$WorktreeRoot = [IO.Path]::GetFullPath($WorktreeRoot)
$ProjectFile = Join-Path $WorktreeRoot 'host\ue5\OpenWillow\OpenWillow.uproject'
if (!(Test-Path -LiteralPath $ProjectFile)) {
    throw "Not an OpenWillow worktree: $WorktreeRoot"
}

$CommonDir = (& git -C $WorktreeRoot rev-parse --path-format=absolute --git-common-dir).Trim()
if ($LASTEXITCODE -ne 0) { throw "Could not find the shared Git directory for $WorktreeRoot" }
$CommonDir = [IO.Path]::GetFullPath($CommonDir)
$RepositoryRoot = Split-Path -Parent $CommonDir
$SeedRoot = Join-Path $RepositoryRoot 'local\worktree-seed'

function Copy-MissingTree([string]$Source, [string]$Destination) {
    if (!(Test-Path -LiteralPath $Source -PathType Container)) { return 0 }

    $SourceRoot = [IO.Path]::GetFullPath($Source).TrimEnd([char[]]@('\', '/'))
    $DestinationRoot = [IO.Path]::GetFullPath($Destination)
    [IO.Directory]::CreateDirectory($DestinationRoot) | Out-Null
    $Copied = 0

    foreach ($File in Get-ChildItem -LiteralPath $SourceRoot -Recurse -File -Force) {
        $RelativePath = $File.FullName.Substring($SourceRoot.Length).TrimStart([char[]]@('\', '/'))
        $Target = Join-Path $DestinationRoot $RelativePath
        if ([IO.File]::Exists($Target)) { continue }

        $TargetDirectory = Split-Path -Parent $Target
        [IO.Directory]::CreateDirectory($TargetDirectory) | Out-Null
        try {
            [IO.File]::Copy($File.FullName, $Target, $false)
            [IO.File]::SetLastWriteTimeUtc($Target, $File.LastWriteTimeUtc)
            $Copied++
        } catch [IO.IOException] {
            # Another provisioning process may have created the same file.
            if (![IO.File]::Exists($Target)) { throw }
        }
    }

    return $Copied
}

$SeedPairs = @(
    @{ Name = 'Content';    Source = (Join-Path $SeedRoot 'Content');    Destination = (Join-Path $WorktreeRoot 'host\ue5\OpenWillow\Content') },
    @{ Name = 'local/items'; Source = (Join-Path $SeedRoot 'items');      Destination = (Join-Path $WorktreeRoot 'local\items') },
    @{ Name = 'local/character'; Source = (Join-Path $SeedRoot 'character'); Destination = (Join-Path $WorktreeRoot 'local\character') },
    @{ Name = 'local/ui';   Source = (Join-Path $SeedRoot 'ui');          Destination = (Join-Path $WorktreeRoot 'local\ui') }
)

if (!(Test-Path -LiteralPath (Join-Path $SeedRoot 'Content') -PathType Container)) {
    throw "OpenWillow asset seed is missing at $SeedRoot; run tools/install_worktree_assets_hook.ps1 from a populated worktree."
}

$Summary = foreach ($Pair in $SeedPairs) {
    $Count = Copy-MissingTree $Pair.Source $Pair.Destination
    '{0}: {1} missing files added' -f $Pair.Name, $Count
}
Write-Output ("OpenWillow worktree assets provisioned for {0}`n{1}" -f $WorktreeRoot, ($Summary -join "`n"))
