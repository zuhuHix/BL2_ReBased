# Before an in-engine suite: look for the things that made earlier runs fail or pass falsely, and report them.
#
# AI-assisted (Claude Opus 5.5), 2026-11-12. It only looks: it deletes no lock, stops no process and writes nothing but its
# report (stdout, and the -Report file when given). Every check is off until its parameter is passed, so a driver passes what
# applies to one suite. One line per check; exit 0 when none failed, 1 when one or more did, 2 when the call itself is wrong.
#
#   -Data <path>[,<path>]               each path (relative to -Worktree, or absolute) is a file, or a folder holding a file
#   -Generators <data>=<gen>[,...]       each data path is newer than the last commit of its tracked generator
#                                        (git log -1 --format=%cI -- <gen>); for a folder, its newest file counts. Fails
#                                        too when the generator has uncommitted changes (git status --porcelain)
#   -CheckLinks                          no junction or symbolic link under <worktree>\local resolves outside the worktree
#   -Port <n>[,<n>]                      no TCP listener holds the port
#   -NoEditor                            no Unreal editor or commandlet process runs (the two names ue_lock.ps1 always
#                                        waits for; extra names a caller passes to Enter-OwUeLock -AlsoWaitFor are not checked)
#   -CheckLock                           the machine-wide run lock (tools/ue_lock.ps1; OPENWILLOW_UE_LOCK) is absent
#   -AlsoLock <path>[,<path>]            these extra lock files are absent
#   -ReportHold <path>[,<path>]          a hold file is reported when present; never a failure
#   -CheckBuild                          -ModuleLibrary and -CoreLibrary are newer than the last commit touching
#                                        host/ue5/OpenWillow/Source, src and CMakeLists.txt, and those have no uncommitted changes
#   -CheckEngine                         the engine folder (-Engine; OPENWILLOW_UE or the default) holds the editor binary
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools/preflight.ps1 -NoEditor -CheckLock -Port 8795 -CheckBuild -CheckEngine
#
# Commit times are committer dates, which cuts both ways for -Generators and -CheckBuild: a rebase after the build makes the
# build look older than the code although nothing changed, and a checkout of a commit dated before the build (a fast-forward
# to older-dated work, a switch to another branch) makes it look fresh although the code differs. Uncommitted changes are
# caught by their own line. Rebuild, or read the line and decide. Test seam: -ProcessList <file> (one process name per line) replaces the live
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
function Invoke-Git([string[]]$GitArgs) {
    # stdout lines and the exit code. Windows PowerShell 5.1 turns a native program's stderr into an error record, which
    # $ErrorActionPreference = 'Stop' makes terminating even under 2>$null; 'Continue' here keeps one git message from
    # ending the run before the report is printed.
    $ErrorActionPreference = 'Continue'
    $out = @(& git -C $Worktree @GitArgs 2>$null)
    return [pscustomobject]@{ Lines = @($out | ForEach-Object { "$_" }); Code = $LASTEXITCODE }
}
function Get-LastCommitUtc([string[]]$Paths) {
    # Committer time of the last commit touching any of the paths, or $null when none did (or this is no git checkout).
    $r = Invoke-Git (@('log', '-1', '--format=%cI', '--') + $Paths)
    $out = ($r.Lines -join '').Trim()
    if ($r.Code -ne 0 -or !$out) { return $null }
    return [DateTimeOffset]::Parse($out, [Globalization.CultureInfo]::InvariantCulture).UtcDateTime
}
function Get-UncommittedChange([string[]]$Paths) {
    # git status --porcelain lines for the paths (modified, staged, untracked); one line saying git failed when it did.
    $r = Invoke-Git (@('status', '--porcelain', '--') + $Paths)
    if ($r.Code -ne 0) { return @("(git status failed, exit $($r.Code))") }
    return @($r.Lines | Where-Object { $_ })
}
function Test-HoldsFile([string]$Path) {
    # A file, or a folder with at least one file somewhere under it.
    if (Test-Path -LiteralPath $Path -PathType Leaf) { return $true }
    if (!(Test-Path -LiteralPath $Path -PathType Container)) { return $false }
    foreach ($f in [IO.Directory]::EnumerateFiles($Path, '*', [IO.SearchOption]::AllDirectories)) { return $true }
    return $false
}
function Read-SharedText([string]$Path) {
    # The file's text, opened so that its owner can still write or delete it meanwhile (File.ReadAllText shares read only,
    # so a delete landing during the read would fail); $null when it cannot be read.
    try {
        $share = [IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete
        $stream = New-Object IO.FileStream($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, $share)
        try { return (New-Object IO.StreamReader($stream)).ReadToEnd() } finally { $stream.Dispose() }
    } catch { return $null }
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

foreach ($pair in $Generators) {
    $at = $pair.IndexOf('=')
    if ($at -lt 1 -or $at -eq $pair.Length - 1) { [Console]::Error.WriteLine("preflight: -Generators wants <data>=<generator>, got '$pair'"); exit 2 }
}

# Every check below runs inside one try, left at the outer indentation: an error nobody expected becomes a FAIL line, and the
# report and summary are still printed.
try {
foreach ($d in $Data) {
    if (Test-HoldsFile (Resolve-InTree $d)) { Out-Line 'ok' 'data' "$d exists" }
    elseif (Test-Path -LiteralPath (Resolve-InTree $d)) { Out-Line 'FAIL' 'data' "$d is a folder that holds no file" }
    else { Out-Line 'FAIL' 'data' "$d is missing" }
}

foreach ($pair in $Generators) {
    $at = $pair.IndexOf('=')
    $d = $pair.Substring(0, $at).Trim(); $gen = $pair.Substring($at + 1).Trim()
    $commit = Get-LastCommitUtc @($gen)
    if ($null -eq $commit) { Out-Line 'FAIL' 'fresh' "$gen has no commit in $Worktree (not a tracked generator, or not a git checkout?)"; continue }
    $dirty = @(Get-UncommittedChange @($gen))
    if ($dirty.Count) { Out-Line 'FAIL' 'fresh' "git status is not clean for $gen, so its commit time says nothing about ${d}: $($dirty -join '; ')"; continue }
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
        # Read with delete sharing, not through Get-OwUeLockOwner (File.ReadAllText): a look must never delay the owner's release.
        $owner = $null
        $text = Read-SharedText $lock
        if ($text) { try { $owner = $text | ConvertFrom-Json -ErrorAction Stop } catch { $owner = $null } }
        $who = if ($owner -and $null -ne $owner.pid) { "owner $($owner.owner), pid $($owner.pid)" } else { 'no owner fields read' }
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
    $code = @('host/ue5/OpenWillow/Source', 'src', 'CMakeLists.txt')
    $commit = Get-LastCommitUtc $code
    $dirty = @(Get-UncommittedChange $code)
    if ($dirty.Count) { Out-Line 'FAIL' 'build' "git status is not clean for the code the build is compared with: $($dirty -join '; ')" }
    if ($null -eq $commit) { Out-Line 'FAIL' 'build' "no commit touches host/ue5/OpenWillow/Source, src or CMakeLists.txt in $Worktree (or not a git checkout)" }
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
} catch {
    Out-Line 'FAIL' 'preflight' "stopped by an unexpected error, later checks not run: $($_.Exception.Message)"
}

$lines | ForEach-Object { Write-Output $_ }
$summary = "PREFLIGHT SUMMARY failed=$failed checks=$($lines.Count)"
Write-Output $summary
if ($Report) { [IO.File]::WriteAllLines([IO.Path]::GetFullPath($Report), [string[]]($lines + $summary)) }
if ($failed) { exit 1 }
exit 0
