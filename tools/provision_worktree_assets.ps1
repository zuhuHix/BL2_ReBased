[CmdletBinding()]
param(
    [string]$WorktreeRoot,
    # Copy missing ignored data from another populated worktree (Content and every local/ folder)
    # instead of the older shared seed. Existing files are never overwritten.
    [string]$SourceWorktree,
    # Fill the seed (-SeedDirectory) from this checkout instead of provisioning a worktree: Content and the seed folders.
    # tools/install_worktree_assets_hook.ps1 runs this, so the seed and the installer share this one code path.
    [string]$FillSeedFrom,
    # The default is the list in worktree_seed_folders.txt next to this script (a seed's copy reads its own copy).
    [string[]]$SeedFolders,
    # Added to SeedFolders, for one run that needs a big folder, e.g. -ExtraSeedFolders sanctuary.
    [string[]]$ExtraSeedFolders = @(),
    # Per-run output: never copied from the seed or from -SourceWorktree, at any depth, and a pair whose own name is here is skipped.
    [string[]]$ExcludeFolderNames = @('shots', 'screenshots', 'logs', 'captures', 'realgame', 'h4x', 'z3x', 'runs'),
    # The seed folder (default: local\worktree-seed of the shared repository). The tests point it at a synthetic folder.
    [string]$SeedDirectory
)

$ErrorActionPreference = 'Stop'

if (!$SeedFolders) {
    $ListFile = Join-Path $PSScriptRoot 'worktree_seed_folders.txt'
    if (!(Test-Path -LiteralPath $ListFile -PathType Leaf)) {
        throw "The seed folder list is missing: $ListFile. Rerun tools/install_worktree_assets_hook.ps1 from a populated worktree."
    }
    $SeedFolders = @(Get-Content -LiteralPath $ListFile -Encoding UTF8 |
        ForEach-Object { ($_ -replace '#.*$', '').Trim() } | Where-Object { $_ })
}

# Folder names become paths under local\, so only plain names are accepted.
$AllFolders = @($SeedFolders + $ExtraSeedFolders) | Select-Object -Unique
foreach ($Name in $AllFolders) {
    if ($Name -notmatch '^[A-Za-z0-9_.-]+$' -or $Name -eq '.' -or $Name -eq '..') {
        throw "Not a plain local/ folder name: '$Name'"
    }
}

function Test-Reparse([string]$Path) {
    # Get-Item does not follow a junction or symlink, so a broken one is seen too.
    $Item = Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
    return ($null -ne $Item -and (($Item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0))
}

function Assert-Under([string]$Path, [string]$Root) {
    if (!$Path.StartsWith($Root + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to write outside $Root`: $Path"
    }
}

function Assert-NoReparse([string]$Anchor, [string]$Path) {
    # Every component from the anchor down to the path (both included) must be a real folder or file: a junction or
    # symlink there would send the copy somewhere else, e.g. out of local\.
    $Root = [IO.Path]::GetFullPath($Anchor).TrimEnd('\')
    $Full = [IO.Path]::GetFullPath($Path).TrimEnd('\')
    if (!$Full.Equals($Root, [StringComparison]::OrdinalIgnoreCase)) { Assert-Under $Full $Root }
    $Current = $Root
    if (Test-Reparse $Current) { throw "Refusing to copy through a junction or symlink: $Current (remove it or copy by hand; see tools/worktree-assets.md)" }
    $Relative = $Full.Substring($Root.Length).TrimStart('\')
    if ($Relative -eq '') { return }
    foreach ($Part in $Relative.Split('\')) {
        $Current = Join-Path $Current $Part
        if (Test-Reparse $Current) { throw "Refusing to copy through a junction or symlink: $Current (remove it or copy by hand; see tools/worktree-assets.md)" }
    }
}

function Copy-MissingTree([string]$Source, [string]$Destination, [string]$SourceAnchor, [string]$DestinationAnchor) {
    if (!(Test-Path -LiteralPath $Source -PathType Container)) { return 0 }

    $SourceRoot = [IO.Path]::GetFullPath($Source).TrimEnd('\')
    $DestinationRoot = [IO.Path]::GetFullPath($Destination).TrimEnd('\')
    Assert-NoReparse $SourceAnchor $SourceRoot
    Assert-NoReparse $DestinationAnchor $DestinationRoot
    [IO.Directory]::CreateDirectory($DestinationRoot) | Out-Null
    $Copied = 0
    $CheckedSource = @{}
    $CheckedTarget = @{}

    foreach ($File in Get-ChildItem -LiteralPath $SourceRoot -Recurse -File -Force) {
        $RelativePath = $File.FullName.Substring($SourceRoot.Length).TrimStart([char[]]@('\', '/'))
        # Per-run output folders are skipped at any depth (the directory parts of the path, not the file name).
        $Folders = @([IO.Path]::GetDirectoryName($RelativePath) -split '[\\/]')
        if (@($Folders | Where-Object { $ExcludeFolderNames -contains $_ }).Count -gt 0) { continue }

        $SourceDirectory = $File.DirectoryName
        if (!$CheckedSource.ContainsKey($SourceDirectory)) {
            Assert-NoReparse $SourceAnchor $SourceDirectory
            $CheckedSource[$SourceDirectory] = $true
        }
        if (Test-Reparse $File.FullName) { throw "Refusing to copy a junction or symlink: $($File.FullName)" }

        $Target = [IO.Path]::GetFullPath((Join-Path $DestinationRoot $RelativePath))
        Assert-Under $Target $DestinationRoot
        $TargetDirectory = Split-Path -Parent $Target
        if (!$CheckedTarget.ContainsKey($TargetDirectory)) {
            Assert-NoReparse $DestinationAnchor $TargetDirectory
            $CheckedTarget[$TargetDirectory] = $true
        }
        if ([IO.File]::Exists($Target)) { continue }

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

if ($FillSeedFrom) {
    # Seed mode: the source is a checkout, the destination is the seed folder. No target worktree.
    if (!$SeedDirectory) { throw '-FillSeedFrom needs -SeedDirectory.' }
    $SourceRoot = [IO.Path]::GetFullPath($FillSeedFrom).TrimEnd('\')
    $SeedRoot = [IO.Path]::GetFullPath($SeedDirectory).TrimEnd('\')
    if (!(Test-Path -LiteralPath (Join-Path $SourceRoot 'host\ue5\OpenWillow\Content') -PathType Container)) {
        throw "Required local asset directory is missing: $SourceRoot\host\ue5\OpenWillow\Content"
    }
    $Pairs = @(@{ Name = 'Content'; Folder = ''; Source = (Join-Path $SourceRoot 'host\ue5\OpenWillow\Content'); SourceAnchor = $SourceRoot;
                  Destination = (Join-Path $SeedRoot 'Content'); DestinationAnchor = $SeedRoot })
    foreach ($Name in $AllFolders) {
        $Pairs += @{ Name = "seed/$Name"; Folder = $Name; Source = (Join-Path $SourceRoot "local\$Name"); SourceAnchor = $SourceRoot;
                     Destination = (Join-Path $SeedRoot $Name); DestinationAnchor = $SeedRoot }
    }
    $Summary = foreach ($Pair in $Pairs) {
        if ($Pair.Folder -and ($ExcludeFolderNames -contains $Pair.Folder)) { '{0}: skipped (excluded name)' -f $Pair.Name; continue }
        $Count = Copy-MissingTree $Pair.Source $Pair.Destination $Pair.SourceAnchor $Pair.DestinationAnchor
        $Note = ''
        if (!(Test-Path -LiteralPath $Pair.Source -PathType Container)) { $Note = ' (not in this checkout)' }
        '{0}: {1} missing files added{2}' -f $Pair.Name, $Count, $Note
    }
    Write-Output ("OpenWillow worktree seed filled at {0} from {1}`n{2}" -f $SeedRoot, $SourceRoot, ($Summary -join "`n"))
    return
}

if ([string]::IsNullOrWhiteSpace($WorktreeRoot)) {
    $WorktreeRoot = (& git -C $PSScriptRoot rev-parse --show-toplevel).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Could not find the current Git worktree.' }
}

$WorktreeRoot = [IO.Path]::GetFullPath($WorktreeRoot).TrimEnd('\')
$ProjectFile = Join-Path $WorktreeRoot 'host\ue5\OpenWillow\OpenWillow.uproject'
if (!(Test-Path -LiteralPath $ProjectFile)) {
    throw "Not an OpenWillow worktree: $WorktreeRoot"
}

$CommonDir = (& git -C $WorktreeRoot rev-parse --path-format=absolute --git-common-dir).Trim()
if ($LASTEXITCODE -ne 0) { throw "Could not find the shared Git directory for $WorktreeRoot" }
$CommonDir = [IO.Path]::GetFullPath($CommonDir)
$RepositoryRoot = Split-Path -Parent $CommonDir
if ($SeedDirectory) {
    $SeedRoot = [IO.Path]::GetFullPath($SeedDirectory).TrimEnd('\')
} else {
    $SeedRoot = Join-Path $RepositoryRoot 'local\worktree-seed'
}

$Pairs = @(
    @{ Name = 'Content'; Folder = ''; Source = (Join-Path $SeedRoot 'Content'); SourceAnchor = $SeedRoot;
       Destination = (Join-Path $WorktreeRoot 'host\ue5\OpenWillow\Content'); DestinationAnchor = $WorktreeRoot }
)
foreach ($Name in $AllFolders) {
    $Pairs += @{ Name = "local/$Name"; Folder = $Name; Source = (Join-Path $SeedRoot $Name); SourceAnchor = $SeedRoot;
                 Destination = (Join-Path $WorktreeRoot "local\$Name"); DestinationAnchor = $WorktreeRoot }
}

if ($SourceWorktree) {
    $SourceWorktree = [IO.Path]::GetFullPath($SourceWorktree).TrimEnd('\')
    if ($SourceWorktree -eq $WorktreeRoot) { throw 'SourceWorktree is the worktree being provisioned.' }
    $Pairs = @(@{ Name = 'Content'; Folder = ''; Source = (Join-Path $SourceWorktree 'host\ue5\OpenWillow\Content'); SourceAnchor = $SourceWorktree;
                  Destination = (Join-Path $WorktreeRoot 'host\ue5\OpenWillow\Content'); DestinationAnchor = $WorktreeRoot })
    $SourceLocal = Join-Path $SourceWorktree 'local'
    if (Test-Path -LiteralPath $SourceLocal -PathType Container) {
        foreach ($Dir in Get-ChildItem -LiteralPath $SourceLocal -Directory -Force) {
            if ($Dir.Name -eq 'worktree-seed') { continue }
            # The pair's own folder name is checked too, so a top-level local\logs is skipped, not only the folders inside it.
            $Pairs += @{ Name = "local/$($Dir.Name)"; Folder = $Dir.Name; Source = $Dir.FullName; SourceAnchor = $SourceWorktree;
                         Destination = (Join-Path $WorktreeRoot "local\$($Dir.Name)"); DestinationAnchor = $WorktreeRoot }
        }
    }
} elseif (!(Test-Path -LiteralPath (Join-Path $SeedRoot 'Content') -PathType Container)) {
    throw "OpenWillow asset seed is missing at $SeedRoot; run tools/install_worktree_assets_hook.ps1 from a populated worktree."
}

$Summary = foreach ($Pair in $Pairs) {
    if ($Pair.Folder -and ($ExcludeFolderNames -contains $Pair.Folder)) { '{0}: skipped (excluded name)' -f $Pair.Name; continue }
    $Count = Copy-MissingTree $Pair.Source $Pair.Destination $Pair.SourceAnchor $Pair.DestinationAnchor
    $Note = ''
    if (!$SourceWorktree -and !(Test-Path -LiteralPath $Pair.Source -PathType Container)) { $Note = ' (not in the seed)' }
    '{0}: {1} missing files added{2}' -f $Pair.Name, $Count, $Note
}
Write-Output ("OpenWillow worktree assets provisioned for {0}`n{1}" -f $WorktreeRoot, ($Summary -join "`n"))
