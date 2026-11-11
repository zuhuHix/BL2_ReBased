[CmdletBinding()]
param(
    [string]$WorktreeRoot,
    # Copy missing ignored data from another populated worktree (Content and every local/ folder)
    # instead of the older shared seed. Existing files are never overwritten (unless -RefreshStale).
    [string]$SourceWorktree,
    # Fill the seed (-SeedDirectory) from this checkout instead of provisioning a worktree: Content and the seed folders.
    # tools/install_worktree_assets_hook.ps1 runs this, so the seed and the installer share this one code path.
    [string]$FillSeedFrom,
    # The default is the list in worktree_seed_folders.txt next to this script (a seed's copy reads its own copy).
    [string[]]$SeedFolders,
    # Added to SeedFolders, for one run that needs a big folder, e.g. -ExtraSeedFolders sanctuary.
    [string[]]$ExtraSeedFolders = @(),
    # Per-run output: never copied from the seed or from -SourceWorktree, at any depth, and a pair whose own name is here is skipped.
    # worktree-refresh-backup holds the files -RefreshStale replaced: this checkout's own, never copied anywhere.
    [string[]]$ExcludeFolderNames = @('shots', 'screenshots', 'logs', 'captures', 'realgame', 'h4x', 'z3x', 'runs', 'worktree-refresh-backup'),
    # Generated files that are per-run or regenerable and that no suite or host code reads: never copied. observed_gear.json is
    # written by tools/prepare_inventory_gear.py from traces; the host reads only gear_manifest.json.
    # *.resolved.json beside a job is the per-checkout copy the editor launchers write (tools/checkout_guard.ps1); it holds absolute paths.
    [string[]]$ExcludeFileNames = @('observed_gear.json', 'editor_job.resolved.json', 'attach_job.resolved.json'),
    # Also replace a file that exists when the source's copy is newer and differs (size or content). The old file is first moved to
    # local\worktree-refresh-backup\<time>\<path> (in seed mode: <seed>\worktree-refresh-backup\...), and both moves are printed as they
    # happen. Off by default: without the switch existing files are never overwritten. A target that is a link is refused.
    [switch]$RefreshStale,
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
    # Every component from the anchor down to the path (both included) must be a real folder or file. Paths are checked
    # lexically from the anchor; this is a local setup step, not a security boundary.
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

function Test-Stale($SourceFile, [string]$Target) {
    # Stale: the source's copy is strictly newer and differs (size first; the hash only when the sizes match).
    $Existing = Get-Item -LiteralPath $Target -Force
    if ($SourceFile.LastWriteTimeUtc -le $Existing.LastWriteTimeUtc) { return $false }
    if ($SourceFile.Length -ne $Existing.Length) { return $true }
    return ((Get-FileHash -LiteralPath $SourceFile.FullName -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath $Target -Algorithm SHA256).Hash)
}

function Get-CopyPlan([string]$Source, [string]$Destination, [string]$SourceAnchor, [string]$DestinationAnchor) {
    # Checks only, no writes: every file that would be copied, with its target. Refusals are thrown here, in the preflight.
    $SourceRoot = [IO.Path]::GetFullPath($Source).TrimEnd('\')
    $DestinationRoot = [IO.Path]::GetFullPath($Destination).TrimEnd('\')
    Assert-NoReparse $SourceAnchor $SourceRoot
    Assert-NoReparse $DestinationAnchor $DestinationRoot
    $CheckedSource = @{}
    $CheckedTarget = @{}
    $Plan = New-Object System.Collections.ArrayList

    foreach ($File in Get-ChildItem -LiteralPath $SourceRoot -Recurse -File -Force) {
        $RelativePath = $File.FullName.Substring($SourceRoot.Length).TrimStart([char[]]@('\', '/'))
        # Per-run output folders are skipped at any depth (the directory parts of the path, not the file name).
        $Folders = @([IO.Path]::GetDirectoryName($RelativePath) -split '[\\/]')
        if (@($Folders | Where-Object { $ExcludeFolderNames -contains $_ }).Count -gt 0) { continue }
        if ($ExcludeFileNames -contains $File.Name) { continue }

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
        $Backup = $null
        if ([IO.File]::Exists($Target)) {
            if (!$RefreshStale) { continue }   # missing files only, unless -RefreshStale
            # Writing through a file symlink would land outside the destination; refused before it is read or hashed.
            if (Test-Reparse $Target) { throw "Refusing to replace a junction or symlink: $Target (remove it or copy by hand)" }
            if (!(Test-Stale $File $Target)) { continue }
            $Backup = [IO.Path]::GetFullPath((Join-Path $BackupBase ($Target.Substring($DestinationAnchor.TrimEnd('\').Length).TrimStart('\'))))
            Assert-NoReparse $DestinationAnchor (Split-Path -Parent $Backup)
        }

        [void]$Plan.Add(@{ Source = $File.FullName; Target = $Target; TargetDirectory = $TargetDirectory; Stamp = $File.LastWriteTimeUtc;
                           Backup = $Backup })
    }
    return ,$Plan.ToArray()
}

function Copy-Plan([string]$DestinationRoot, $Plan) {
    [IO.Directory]::CreateDirectory($DestinationRoot) | Out-Null
    $Copied = 0
    $Replaced = 0
    foreach ($Item in $Plan) {
        if ($Item.Backup) {
            # Printed as each step happens, so a failure (a file the editor holds open) still leaves the record of what was done.
            [IO.Directory]::CreateDirectory((Split-Path -Parent $Item.Backup)) | Out-Null
            [IO.File]::Move($Item.Target, $Item.Backup)
            Write-Host "  backed up: $($Item.Target) -> $($Item.Backup)"
            [IO.File]::Copy($Item.Source, $Item.Target, $false)
            [IO.File]::SetLastWriteTimeUtc($Item.Target, $Item.Stamp)
            Write-Host "  replaced (the source's copy is newer): $($Item.Target)"
            $Replaced++
            continue
        }
        if ([IO.File]::Exists($Item.Target)) { continue }
        [IO.Directory]::CreateDirectory($Item.TargetDirectory) | Out-Null
        try {
            [IO.File]::Copy($Item.Source, $Item.Target, $false)
            [IO.File]::SetLastWriteTimeUtc($Item.Target, $Item.Stamp)
            $Copied++
        } catch [IO.IOException] {
            # Another provisioning process may have created the same file.
            if (![IO.File]::Exists($Item.Target)) { throw }
        }
    }
    return @{ Copied = $Copied; Replaced = $Replaced }
}

function Invoke-Pairs($Pairs, [bool]$SkipLocal, [string]$MissingNote) {
    # Preflight first: every pair is checked before the first write, so a refusal leaves every destination untouched.
    $Plans = New-Object System.Collections.ArrayList
    $Lines = New-Object System.Collections.ArrayList
    foreach ($Pair in $Pairs) {
        if ($Pair.Folder -and ($ExcludeFolderNames -contains $Pair.Folder)) {
            [void]$Lines.Add(('{0}: skipped (excluded name)' -f $Pair.Name)); continue
        }
        if ($SkipLocal -and $Pair.Name.StartsWith('local/')) { continue }
        if (!(Test-Path -LiteralPath $Pair.Source -PathType Container)) {
            [void]$Plans.Add(@{ Pair = $Pair; Plan = @(); Missing = $true }); continue
        }
        $Plan = Get-CopyPlan $Pair.Source $Pair.Destination $Pair.SourceAnchor $Pair.DestinationAnchor
        [void]$Plans.Add(@{ Pair = $Pair; Plan = $Plan; Missing = $false })
    }
    # Every pair passed the preflight; only now does anything get written.
    foreach ($Entry in $Plans) {
        if ($Entry.Missing) {
            [void]$Lines.Add(('{0}: 0 missing files added{1}' -f $Entry.Pair.Name, $MissingNote)); continue
        }
        $Result = Copy-Plan $Entry.Pair.Destination $Entry.Plan
        $Line = '{0}: {1} missing files added' -f $Entry.Pair.Name, $Result.Copied
        if ($RefreshStale) { $Line += (', {0} stale files replaced' -f $Result.Replaced) }
        [void]$Lines.Add($Line)
    }
    return ,$Lines.ToArray()
}

# Where -RefreshStale moves the files it replaces: one folder per run, below the destination (set per mode below).
$RefreshStamp = [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss')
$BackupBase = $null

if ($FillSeedFrom) {
    # Seed mode: the source is a checkout, the destination is the seed folder. No target worktree.
    if (!$SeedDirectory) { throw '-FillSeedFrom needs -SeedDirectory.' }
    $SourceRoot = [IO.Path]::GetFullPath($FillSeedFrom).TrimEnd('\')
    $SeedRoot = [IO.Path]::GetFullPath($SeedDirectory).TrimEnd('\')
    $BackupBase = Join-Path $SeedRoot "worktree-refresh-backup\$RefreshStamp"
    if (!(Test-Path -LiteralPath (Join-Path $SourceRoot 'host\ue5\OpenWillow\Content') -PathType Container)) {
        throw "Required local asset directory is missing: $SourceRoot\host\ue5\OpenWillow\Content"
    }
    $Pairs = @(@{ Name = 'Content'; Folder = ''; Source = (Join-Path $SourceRoot 'host\ue5\OpenWillow\Content'); SourceAnchor = $SourceRoot;
                  Destination = (Join-Path $SeedRoot 'Content'); DestinationAnchor = $SeedRoot })
    foreach ($Name in $AllFolders) {
        $Pairs += @{ Name = "seed/$Name"; Folder = $Name; Source = (Join-Path $SourceRoot "local\$Name"); SourceAnchor = $SourceRoot;
                     Destination = (Join-Path $SeedRoot $Name); DestinationAnchor = $SeedRoot }
    }
    $Lines = Invoke-Pairs $Pairs $false ' (not in this checkout)'
    Write-Output ("OpenWillow worktree seed filled at {0} from {1}`n{2}" -f $SeedRoot, $SourceRoot, ($Lines -join "`n"))
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
$MissingNote = ' (not in the seed)'

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
    $MissingNote = ''
} elseif (!(Test-Path -LiteralPath (Join-Path $SeedRoot 'Content') -PathType Container)) {
    throw "OpenWillow asset seed is missing at $SeedRoot; run tools/install_worktree_assets_hook.ps1 from a populated worktree."
}

# A worktree whose local\ is itself a junction (a shared data root, e.g. on a runner) keeps that data: its local pairs are
# skipped with one line, Content is still provisioned. Any other junction is refused by the preflight.
$LocalIsJunction = Test-Reparse (Join-Path $WorktreeRoot 'local')
$BackupBase = Join-Path $WorktreeRoot "local\worktree-refresh-backup\$RefreshStamp"
$Lines = Invoke-Pairs $Pairs $LocalIsJunction $MissingNote
$Header = "OpenWillow worktree assets provisioned for $WorktreeRoot"
if ($LocalIsJunction) { $Lines = @('local\ is a junction (shared data root); not provisioning local') + $Lines }
# The marker the post-checkout hook reads: in this worktree's own git directory (per worktree, never tracked, and not under local\,
# which may be a shared junctioned root). It is written only after every copy above succeeded.
$GitDir = (& git -C $WorktreeRoot rev-parse --path-format=absolute --git-dir).Trim()
if ($LASTEXITCODE -eq 0 -and $GitDir) {
    Set-Content -LiteralPath (Join-Path $GitDir 'openwillow-provisioned') -Value ([DateTimeOffset]::UtcNow.ToString('o')) -Encoding ascii
}
Write-Output ($Header + "`n" + ($Lines -join "`n"))
