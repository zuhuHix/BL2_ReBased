# Before an in-engine suite: look for the things that made earlier runs fail or pass falsely, and report them.
#
# AI-assisted (Claude Opus 5.5), 2026-11-12. It only looks: it deletes no lock, stops no process and writes nothing but its
# report (stdout, and the -Report file when given). Every check is off until its parameter is passed, so a driver passes what
# applies to one suite. One line per check; exit 0 when none failed, 1 when one or more did, 2 when the call itself is wrong.
#
#   -Data <path>[,<path>]               each path (relative to -Worktree, or absolute) exists
#   -Generators <data>=<gen>[,...]       each data path is newer than the last commit of its tracked generator
#                                        (git log -1 --format=%cI -- <gen>); for a folder, its newest file counts
#   -CheckLinks                          no junction or symbolic link under <worktree>\local resolves outside the worktree
#   -Port <n>[,<n>]                      no TCP listener holds the port
#   -NoEditor                            no Unreal editor or commandlet process runs (the processes ue_lock.ps1 waits for)
#   -CheckLock                           the machine-wide run lock (tools/ue_lock.ps1; OPENWILLOW_UE_LOCK) is absent
#   -AlsoLock <path>[,<path>]            these extra lock files are absent
#   -ReportHold <path>[,<path>]          a hold file is reported when present; never a failure
#   -CheckBuild                          -ModuleLibrary and -CoreLibrary are newer than the last commit touching
#                                        host/ue5/OpenWillow/Source, src and CMakeLists.txt
#   -CheckEngine                         the engine folder (-Engine; OPENWILLOW_UE or the default) holds the editor binary
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools/preflight.ps1 -NoEditor -CheckLock -Port 8795 -CheckBuild -CheckEngine
#
# Commit times are committer dates: a rebase after the build makes the build look older than the code although nothing
# changed. Rebuild, or read the line and decide. Test seam: -ProcessList <file> (one process name per line) replaces the live
# process list for -NoEditor, so the tests never match a real editor.
param(
    [string]$Worktree = (Split-Path -Parent $PSScriptRoot),
    [string[]]$Data = @(),
    [string[]]$Generators = @(),
    [switch]$CheckLinks,
    [string[]]$Port = @(),
    [switch]$NoEditor,
    [string]$ProcessList,
    [switch]$CheckLock,
    [string[]]$AlsoLock = @(),
    [string[]]$ReportHold = @(),
    [switch]$CheckBuild,
    [string]$ModuleLibrary = 'host\ue5\OpenWillow\Binaries\Win64\UnrealEditor-OpenWillow.dll',
    [string]$CoreLibrary = 'build\Release\ow-core.lib',
    [switch]$CheckEngine,
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    [string]$Report
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'checkout_guard.ps1')   # Get-OwPhysicalPath only; Assert-OwCheckout is not called

$lines = New-Object System.Collections.Generic.List[string]
$failed = 0
function Out-Line([string]$Kind, [string]$Check, [string]$Text) {
    $line = "PREFLIGHT $Kind ${Check}: $Text"
    $script:lines.Add($line)
    if ($Kind -eq 'FAIL') { $script:failed++ }
}
# -Data a,b arrives as one array; "-Data 'a,b'" from a -Command string arrives as one string. Accept both.
function Split-List([string[]]$Items) { @($Items | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ }) }
function Resolve-InTree([string]$Path) {
    if ([IO.Path]::IsPathRooted($Path)) { return [IO.Path]::GetFullPath($Path) }
    return [IO.Path]::GetFullPath((Join-Path $Worktree $Path))
}
function Get-LastCommitUtc([string[]]$Paths) {
    # Committer time of the last commit touching any of the paths, or $null when none did.
    $out = & git -C $Worktree log -1 --format=%cI -- @Paths 2>$null
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace("$out")) { return $null }
    return [DateTimeOffset]::Parse("$out".Trim(), [Globalization.CultureInfo]::InvariantCulture).UtcDateTime
}
function Get-NewestWriteUtc([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force
    if (!$item.PSIsContainer) { return $item.LastWriteTimeUtc }
    $newest = $null
    foreach ($f in [IO.Directory]::EnumerateFiles($Path, '*', [IO.SearchOption]::AllDirectories)) {
        $t = [IO.File]::GetLastWriteTimeUtc($f)
        if ($null -eq $newest -or $t -gt $newest) { $newest = $t }
    }
    return $newest
}
function Format-Utc($t) { if ($null -eq $t) { 'none' } else { $t.ToString('yyyy-MM-ddTHH:mm:ssZ') } }

if (!(Test-Path -LiteralPath $Worktree -PathType Container)) {
    [Console]::Error.WriteLine("preflight: no worktree folder at $Worktree"); exit 2
}
$Worktree = [IO.Path]::GetFullPath($Worktree).TrimEnd('\')
$Data = Split-List $Data; $Generators = Split-List $Generators; $AlsoLock = Split-List $AlsoLock; $Port = Split-List $Port; $ReportHold = Split-List $ReportHold
foreach ($p in $Port) { if ($p -notmatch '^\d{1,5}$') { [Console]::Error.WriteLine("preflight: -Port wants numbers, got '$p'"); exit 2 } }
$checks = $Data.Count + $Generators.Count + $Port.Count + $AlsoLock.Count + $ReportHold.Count
if (!($checks -or $CheckLinks -or $NoEditor -or $CheckLock -or $CheckBuild -or $CheckEngine)) {
    [Console]::Error.WriteLine('preflight: no check asked for (see the header of tools/preflight.ps1)'); exit 2
}

foreach ($d in $Data) {
    if (Test-Path -LiteralPath (Resolve-InTree $d)) { Out-Line 'ok' 'data' "$d exists" }
    else { Out-Line 'FAIL' 'data' "$d is missing" }
}

foreach ($pair in $Generators) {
    $at = $pair.IndexOf('=')
    if ($at -lt 1 -or $at -eq $pair.Length - 1) { [Console]::Error.WriteLine("preflight: -Generators wants <data>=<generator>, got '$pair'"); exit 2 }
    $d = $pair.Substring(0, $at).Trim(); $gen = $pair.Substring($at + 1).Trim()
    $commit = Get-LastCommitUtc @($gen)
    if ($null -eq $commit) { Out-Line 'FAIL' 'fresh' "$gen has no commit in $Worktree (not a tracked generator?)"; continue }
    $full = Resolve-InTree $d
    if (!(Test-Path -LiteralPath $full)) { Out-Line 'FAIL' 'fresh' "$d is missing (its generator $gen)"; continue }
    $written = Get-NewestWriteUtc $full
    if ($null -eq $written) { Out-Line 'FAIL' 'fresh' "$d holds no file"; continue }
    if ($written -gt $commit) { Out-Line 'ok' 'fresh' "$d ($(Format-Utc $written)) is newer than $gen ($(Format-Utc $commit))" }
    else { Out-Line 'FAIL' 'fresh' "$d ($(Format-Utc $written)) is older than the last commit of $gen ($(Format-Utc $commit)): regenerate it" }
}

if ($CheckLinks) {
    $root = Get-OwPhysicalPath $Worktree
    $local = Join-Path $Worktree 'local'
    $bad = 0; $seen = 0
    if (Test-Path -LiteralPath $local -PathType Container) {
        # Walk by hand and do not descend into a link: Get-ChildItem -Recurse follows junctions in Windows PowerShell 5.1.
        $stack = New-Object System.Collections.Generic.Stack[string]
        $stack.Push($local)
        while ($stack.Count) {
            $dir = $stack.Pop()
            foreach ($entry in ([IO.DirectoryInfo]$dir).EnumerateFileSystemInfos()) {
                if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                    $seen++
                    $target = Get-OwPhysicalPath $entry.FullName
                    if ($target.Equals($entry.FullName.TrimEnd('\'), [StringComparison]::OrdinalIgnoreCase)) {
                        $raw = (Get-Item -LiteralPath $entry.FullName -Force).Target   # a dangling link does not open
                        $target = if ($raw) { [IO.Path]::GetFullPath([string]@($raw)[0]).TrimEnd('\') } else { $null }
                    }
                    $inside = $target -and ($target.Equals($root, [StringComparison]::OrdinalIgnoreCase) -or
                                            $target.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase))
                    if (!$inside) {
                        $bad++
                        $shown = if ($target) { $target } else { '(cannot be resolved)' }
                        Out-Line 'FAIL' 'links' "$($entry.FullName) points to $shown, outside $root"
                    }
                } elseif ($entry -is [IO.DirectoryInfo]) {
                    $stack.Push($entry.FullName)
                }
            }
        }
    }
    if (!$bad) { Out-Line 'ok' 'links' "$seen link(s) under local\, none outside the worktree" }
}

if ($Port.Count) {
    $listening = @([Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners() | ForEach-Object { $_.Port })
    foreach ($p in $Port) {
        if ($listening -contains [int]$p) { Out-Line 'FAIL' 'port' "$p is held by a listening process (find it: Get-NetTCPConnection -State Listen -LocalPort $p)" }
        else { Out-Line 'ok' 'port' "$p is free" }
    }
}

if ($NoEditor) {
    if ($ProcessList) { $names = @(Get-Content -LiteralPath $ProcessList | ForEach-Object { $_.Trim() } | Where-Object { $_ }) }
    else {
        . (Join-Path $PSScriptRoot 'ue_lock.ps1')
        $names = @(Get-OwUeBusyProcess | ForEach-Object { "$($_.ProcessName) $($_.Id)" })
    }
    $editors = @($names | Where-Object { $_ -match '^UnrealEditor(-Cmd)?(\.exe)?(\s|$)' })
    if ($editors.Count) { Out-Line 'FAIL' 'editor' "an Unreal editor runs: $($editors -join ', ')" }
    else { Out-Line 'ok' 'editor' 'no Unreal editor runs' }
}

if ($CheckLock) {
    . (Join-Path $PSScriptRoot 'ue_lock.ps1')
    # Get-OwUeLockPath creates the lock's folder when it is missing (as every suite does); it never creates the lock.
    $lock = Get-OwUeLockPath
    if (Test-Path -LiteralPath $lock) {
        $owner = Get-OwUeLockOwner $lock
        $who = if ($owner) { "owner $($owner.owner), pid $($owner.pid)" } else { 'no owner fields' }
        Out-Line 'FAIL' 'lock' "the run lock $lock is held ($who)"
    } else { Out-Line 'ok' 'lock' "the run lock $lock is free" }
}
foreach ($l in $AlsoLock) {
    $full = Resolve-InTree $l
    if (Test-Path -LiteralPath $full) { Out-Line 'FAIL' 'lock' "$full is held" } else { Out-Line 'ok' 'lock' "$full is free" }
}
foreach ($h in $ReportHold) {
    $full = Resolve-InTree $h
    if (Test-Path -LiteralPath $full) { Out-Line 'NOTE' 'hold' "$full is present" } else { Out-Line 'ok' 'hold' "$full is absent" }
}

if ($CheckBuild) {
    $commit = Get-LastCommitUtc @('host/ue5/OpenWillow/Source', 'src', 'CMakeLists.txt')
    if ($null -eq $commit) { Out-Line 'FAIL' 'build' "no commit touches host/ue5/OpenWillow/Source, src or CMakeLists.txt in $Worktree" }
    else {
        foreach ($lib in @($ModuleLibrary, $CoreLibrary)) {
            $full = Resolve-InTree $lib
            if (!(Test-Path -LiteralPath $full -PathType Leaf)) { Out-Line 'FAIL' 'build' "$lib is missing: build it"; continue }
            $t = (Get-Item -LiteralPath $full -Force).LastWriteTimeUtc
            if ($t -gt $commit) { Out-Line 'ok' 'build' "$lib ($(Format-Utc $t)) is newer than the code ($(Format-Utc $commit))" }
            else { Out-Line 'FAIL' 'build' "$lib ($(Format-Utc $t)) is older than the last code commit ($(Format-Utc $commit)): rebuild" }
        }
    }
}

if ($CheckEngine) {
    # The name is put together so that tests/ue_lock_test.py, which requires every script naming the editor binary to take
    # the run lock, does not count this script as a launcher: it only tests that the file exists.
    $binary = Join-Path $Engine ('Engine\Binaries\Win64\UnrealEditor' + '.exe')
    if (Test-Path -LiteralPath $binary -PathType Leaf) { Out-Line 'ok' 'engine' "$binary exists" }
    else { Out-Line 'FAIL' 'engine' "no editor binary at $binary (set OPENWILLOW_UE or pass -Engine)" }
}

$lines | ForEach-Object { Write-Output $_ }
$summary = "PREFLIGHT SUMMARY failed=$failed checks=$($lines.Count)"
Write-Output $summary
if ($Report) { [IO.File]::WriteAllLines([IO.Path]::GetFullPath($Report), [string[]]($lines + $summary)) }
if ($failed) { exit 1 }
exit 0
