[CmdletBinding()]
param(
    [string]$WorktreeRoot,
    # Copy missing ignored data from another populated worktree (Content and every local/ folder)
    # instead of the older shared seed. Existing files are never overwritten.
    [string]$SourceWorktree,
    # The default seed copies Content and these local/ folders (seed\<name> -> local\<name>). They are the inputs the suites
    # read (docs/TOOLING.md, "What a new worktree gets"). Not here: sanctuary (about 1.7 GB in the main checkout) and external
    # (UModel exports), which are added by name with -ExtraSeedFolders, and weapon_library and gestalt, read only by import tools.
    [string[]]$SeedFolders = @(
        'items', 'character', 'ui', 'slice', 'doors', 'range', 'quest', 'menus', 'lootables', 'phaselock', 'weapons',
        'vendors', 'interactives', 'minimap', 'ambient', 'skillhud', 'audio', 'firing', 'inventory-actions'),
    # Added to SeedFolders, for one run that needs a big folder, e.g. -ExtraSeedFolders sanctuary.
    [string[]]$ExtraSeedFolders = @(),
    # Per-run output: never copied from the seed or from -SourceWorktree, at any depth.
    [string[]]$ExcludeFolderNames = @('shots', 'screenshots', 'logs', 'captures', 'realgame', 'h4x', 'z3x', 'runs'),
    # The seed folder (default: local\worktree-seed of the shared repository). The tests point it at a synthetic folder.
    [string]$SeedDirectory
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
if ($SeedDirectory) {
    $SeedRoot = [IO.Path]::GetFullPath($SeedDirectory)
} else {
    $SeedRoot = Join-Path $RepositoryRoot 'local\worktree-seed'
}

# Folder names become paths under local\, so only plain names are accepted.
$AllFolders = @($SeedFolders + $ExtraSeedFolders) | Select-Object -Unique
foreach ($Name in $AllFolders) {
    if ($Name -notmatch '^[A-Za-z0-9_.-]+$' -or $Name -eq '.' -or $Name -eq '..') {
        throw "Not a plain local/ folder name: '$Name'"
    }
}

function Copy-MissingTree([string]$Source, [string]$Destination) {
    if (!(Test-Path -LiteralPath $Source -PathType Container)) { return 0 }

    $SourceRoot = [IO.Path]::GetFullPath($Source).TrimEnd([char[]]@('\', '/'))
    $DestinationRoot = [IO.Path]::GetFullPath($Destination)
    [IO.Directory]::CreateDirectory($DestinationRoot) | Out-Null
    $Copied = 0

    foreach ($File in Get-ChildItem -LiteralPath $SourceRoot -Recurse -File -Force) {
        $RelativePath = $File.FullName.Substring($SourceRoot.Length).TrimStart([char[]]@('\', '/'))
        # Per-run output folders are skipped at any depth (the directory parts of the path, not the file name).
        $Folders = @([IO.Path]::GetDirectoryName($RelativePath) -split '[\\/]')
        if (@($Folders | Where-Object { $ExcludeFolderNames -contains $_ }).Count -gt 0) { continue }

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
    @{ Name = 'Content'; Source = (Join-Path $SeedRoot 'Content'); Destination = (Join-Path $WorktreeRoot 'host\ue5\OpenWillow\Content') }
)
foreach ($Name in $AllFolders) {
    $SeedPairs += @{ Name = "local/$Name"; Source = (Join-Path $SeedRoot $Name); Destination = (Join-Path $WorktreeRoot "local\$Name") }
}

if ($SourceWorktree) {
    $SourceWorktree = [IO.Path]::GetFullPath($SourceWorktree)
    if ($SourceWorktree -eq $WorktreeRoot) { throw 'SourceWorktree is the worktree being provisioned.' }
    $SeedPairs = @(@{ Name = 'Content'; Source = (Join-Path $SourceWorktree 'host\ue5\OpenWillow\Content'); Destination = (Join-Path $WorktreeRoot 'host\ue5\OpenWillow\Content') })
    $SourceLocal = Join-Path $SourceWorktree 'local'
    if (Test-Path -LiteralPath $SourceLocal -PathType Container) {
        foreach ($Dir in Get-ChildItem -LiteralPath $SourceLocal -Directory -Force) {
            if ($Dir.Name -eq 'worktree-seed') { continue }
            $SeedPairs += @{ Name = "local/$($Dir.Name)"; Source = $Dir.FullName; Destination = (Join-Path $WorktreeRoot "local\$($Dir.Name)") }
        }
    }
} elseif (!(Test-Path -LiteralPath (Join-Path $SeedRoot 'Content') -PathType Container)) {
    throw "OpenWillow asset seed is missing at $SeedRoot; run tools/install_worktree_assets_hook.ps1 from a populated worktree."
}

$Summary = foreach ($Pair in $SeedPairs) {
    $Count = Copy-MissingTree $Pair.Source $Pair.Destination
    $Note = ''
    if (!$SourceWorktree -and !(Test-Path -LiteralPath $Pair.Source -PathType Container)) { $Note = ' (not in the seed)' }
    '{0}: {1} missing files added{2}' -f $Pair.Name, $Count, $Note
}
Write-Output ("OpenWillow worktree assets provisioned for {0}`n{1}" -f $WorktreeRoot, ($Summary -join "`n"))
