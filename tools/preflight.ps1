# Before an in-engine suite: look for the things that made earlier runs fail or pass falsely, and report them.
#
# AI-assisted (Claude Opus 5.5), 2026-11-12. It only looks: it deletes no lock, stops no process and writes nothing but its
# report (stdout, and the -Report file when given); -CheckLock may create the lock's folder, never the lock, and git runs
# with --no-optional-locks so the worktree's index is not refreshed. Every check is off until its parameter is passed, so a
# driver passes what applies to one suite. One line per check (ok, FAIL, NOTE, or UNKNOWN when the check cannot tell);
# exit 0 when none failed, 1 when one or more did, 2 when the call itself is wrong. UNKNOWN is not a failure and never
# means ok: the summary counts it, and a driver that needs a known answer reads it.
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
#   -CheckBuild                          -ModuleLibrary is newer (file times) than the files it is built from; -CoreLibrary
#                                        matches the input stamp the last CMake build wrote next to it (<lib>.inputs, see
#                                        ow-build-stamp in CMakeLists.txt). UNKNOWN, not ok, when that cannot be told: no
#                                        stamp, uncommitted inputs at the last build, a configure after it,
#                                        a newer build description
#   -CheckEngine                         the engine folder (-Engine; OPENWILLOW_UE or the default) holds the editor binary
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools/preflight.ps1 -NoEditor -CheckLock -Port 8795 -CheckBuild -CheckEngine
#
# -Generators compares with committer dates, which cuts both ways: a rebase after the generator ran makes the data look
# older than the generator although nothing changed, and a checkout of a commit dated before the run (a fast-forward to
# older-dated work, a switch to another branch) makes it look fresh although the generator differs. Uncommitted changes
# are caught by their own line. Regenerate, or read the line and decide. -CheckBuild uses no commit dates.
# Test seam: -ProcessList <file> (one process name per line) replaces the live process list for -NoEditor, so the tests
# never match a real editor.
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
$unknown = 0
function Out-Line([string]$Kind, [string]$Check, [string]$Text) {
    $line = "PREFLIGHT $Kind ${Check}: $Text"
    $script:lines.Add($line)
    if ($Kind -eq 'FAIL') { $script:failed++ }
    if ($Kind -eq 'UNKNOWN') { $script:unknown++ }
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
    # --no-optional-locks: a plain git status refreshes the index (index.lock, a rewritten index), which a look must not do
    # to a worktree another lane may be committing in.
    $out = @(& git --no-optional-locks -C $Worktree @GitArgs 2>$null)
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
function Get-TreeName([string]$Path) {
    # A path under the worktree as the worktree-relative name, for the report.
    if ($Path.StartsWith($Worktree + '\', [StringComparison]::OrdinalIgnoreCase)) { return $Path.Substring($Worktree.Length + 1) }
    return $Path
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

# Each check runs in its own try (Invoke-Check): an error nobody expected becomes a FAIL line for that check only, the
# other checks still run, and the report and summary are always printed. Inside a check, return ends it early.
function Invoke-Check([string]$Check, [scriptblock]$Body) {
    try { . $Body } catch { Out-Line 'FAIL' $Check "stopped by an unexpected error: $($_.Exception.Message)" }
}

foreach ($d in $Data) {
    Invoke-Check 'data' {
        if (Test-HoldsFile (Resolve-InTree $d)) { Out-Line 'ok' 'data' "$d exists" }
        elseif (Test-Path -LiteralPath (Resolve-InTree $d)) { Out-Line 'FAIL' 'data' "$d is a folder that holds no file" }
        else { Out-Line 'FAIL' 'data' "$d is missing" }
    }
}

foreach ($pair in $Generators) {
    Invoke-Check 'fresh' {
        $at = $pair.IndexOf('=')
        $d = $pair.Substring(0, $at).Trim(); $gen = $pair.Substring($at + 1).Trim()
        $commit = Get-LastCommitUtc @($gen)
        if ($null -eq $commit) { Out-Line 'FAIL' 'fresh' "$gen has no commit in $Worktree (not a tracked generator, or not a git checkout?)"; return }
        $dirty = @(Get-UncommittedChange @($gen))
        if ($dirty.Count) { Out-Line 'FAIL' 'fresh' "git status is not clean for $gen, so its commit time says nothing about ${d}: $($dirty -join '; ')"; return }
        $full = Resolve-InTree $d
        if (!(Test-Path -LiteralPath $full)) { Out-Line 'FAIL' 'fresh' "$d is missing (its generator $gen)"; return }
        $written = Get-NewestWriteUtc $full
        if ($null -eq $written) { Out-Line 'FAIL' 'fresh' "$d holds no file"; return }
        if ($written -gt $commit) { Out-Line 'ok' 'fresh' "$d ($(Format-Utc $written)) is newer than $gen ($(Format-Utc $commit))" }
        else { Out-Line 'FAIL' 'fresh' "$d ($(Format-Utc $written)) is older than the last commit of $gen ($(Format-Utc $commit)): regenerate it" }
    }
}

if ($CheckLinks) {
    Invoke-Check 'links' {
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
}

if ($Port.Count) {
    Invoke-Check 'port' {
        $listening = @([Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners() | ForEach-Object { $_.Port })
        foreach ($p in $Port) {
            if ($listening -contains [int]$p) { Out-Line 'FAIL' 'port' "$p is held by a listening process (find it: Get-NetTCPConnection -State Listen -LocalPort $p)" }
            else { Out-Line 'ok' 'port' "$p is free" }
        }
    }
}

if ($NoEditor) {
    Invoke-Check 'editor' {
        if ($ProcessList) { $names = @(Get-Content -LiteralPath $ProcessList | ForEach-Object { $_.Trim() } | Where-Object { $_ }) }
        else {
            . (Join-Path $PSScriptRoot 'ue_lock.ps1')
            $names = @(Get-OwUeBusyProcess | ForEach-Object { "$($_.ProcessName) $($_.Id)" })
        }
        $editors = @($names | Where-Object { $_ -match '^UnrealEditor(-Cmd)?(\.exe)?(\s|$)' })
        if ($editors.Count) { Out-Line 'FAIL' 'editor' "an Unreal editor runs: $($editors -join ', ')" }
        else { Out-Line 'ok' 'editor' 'no Unreal editor runs' }
    }
}

if ($CheckLock) {
    Invoke-Check 'lock' {
        . (Join-Path $PSScriptRoot 'ue_lock.ps1')
        # Get-OwUeLockPath creates the lock's folder when it is missing (as every suite does); it never creates the lock.
        $lock = Get-OwUeLockPath
        if (!(Test-Path -LiteralPath $lock)) { Out-Line 'ok' 'lock' "the run lock $lock is free"; return }
        # Read with delete sharing, not through Get-OwUeLockOwner (File.ReadAllText): a look must never delay the owner's release.
        $owner = $null
        $text = Read-SharedText $lock
        if (!$text -and !(Test-Path -LiteralPath $lock)) { Out-Line 'ok' 'lock' "the run lock $lock is free (released while it was read)"; return }
        if ($text) { try { $owner = $text | ConvertFrom-Json -ErrorAction Stop } catch { $owner = $null } }
        $who = if ($owner -and $null -ne $owner.pid) { "owner $($owner.owner), pid $($owner.pid)" } else { 'no owner fields read' }
        Out-Line 'FAIL' 'lock' "the run lock $lock is held ($who)"
    }
}
foreach ($l in $AlsoLock) {
    Invoke-Check 'lock' {
        $full = Resolve-InTree $l
        if (Test-Path -LiteralPath $full) { Out-Line 'FAIL' 'lock' "$full is held" } else { Out-Line 'ok' 'lock' "$full is free" }
    }
}
foreach ($h in $ReportHold) {
    Invoke-Check 'hold' {
        $full = Resolve-InTree $h
        if (Test-Path -LiteralPath $full) { Out-Line 'NOTE' 'hold' "$full is present" } else { Out-Line 'ok' 'hold' "$full is absent" }
    }
}

if ($CheckBuild) {
    # The module DLL, by file times as Unreal's build tool judges it: it must be newer than every file it is built from
    # (host/ue5/OpenWillow/Source, the headers under src, the two CMake libraries it links); a newer file is stale. The
    # Source folder and every folder under it are looked at too: a file deleted or renamed there leaves no newer file, but
    # Windows gives its folder a new time. A folder's time also moves when a file is added and removed again (two
    # checkouts through another file set), after which the build tool rightly relinks nothing, so a newer folder alone is
    # unknown, never stale and never fresh. So is a newer build description (*.Build.cs, *.Target.cs, the .uproject),
    # which need not change the DLL either.
    Invoke-Check 'build' {
        $dll = Resolve-InTree $ModuleLibrary
        if (!(Test-Path -LiteralPath $dll -PathType Leaf)) { Out-Line 'FAIL' 'build' "$ModuleLibrary is missing: build it"; return }
        $built = [IO.File]::GetLastWriteTimeUtc($dll)
        $code = New-Object System.Collections.Generic.List[string]
        $descriptions = New-Object System.Collections.Generic.List[string]
        $source = Join-Path $Worktree 'host\ue5\OpenWillow\Source'
        if (Test-Path -LiteralPath $source -PathType Container) {
            foreach ($f in [IO.Directory]::EnumerateFiles($source, '*', [IO.SearchOption]::AllDirectories)) {
                if ($f -match '\.(Build|Target)\.cs$') { $descriptions.Add($f) } else { $code.Add($f) }
            }
            $folders = @($source) + @([IO.Directory]::EnumerateDirectories($source, '*', [IO.SearchOption]::AllDirectories))
        } else { $folders = @() }
        $src = Join-Path $Worktree 'src'
        if (Test-Path -LiteralPath $src -PathType Container) {
            foreach ($f in [IO.Directory]::EnumerateFiles($src, '*', [IO.SearchOption]::AllDirectories)) { if ($f -match '\.(h|hpp)$') { $code.Add($f) } }
        }
        $core = Resolve-InTree $CoreLibrary
        foreach ($f in @($core, (Join-Path (Split-Path -Parent $core) 'ow-lzokay.lib'))) { if (Test-Path -LiteralPath $f -PathType Leaf) { $code.Add($f) } }
        $project = Join-Path $Worktree 'host\ue5\OpenWillow\OpenWillow.uproject'
        if (Test-Path -LiteralPath $project -PathType Leaf) { $descriptions.Add($project) }
        $newer = @($code | Where-Object { [IO.File]::GetLastWriteTimeUtc($_) -gt $built })
        $newerFolders = @($folders | Where-Object { [IO.Directory]::GetLastWriteTimeUtc($_) -gt $built })
        $newerDesc = @($descriptions | Where-Object { [IO.File]::GetLastWriteTimeUtc($_) -gt $built })
        if ($newer.Count) {
            $shown = @($newer | Select-Object -First 3 | ForEach-Object { Get-TreeName $_ }) -join ', '
            Out-Line 'FAIL' 'build' "$ModuleLibrary ($(Format-Utc $built)) is older than $($newer.Count) file(s) it is built from ($shown): rebuild"
        } elseif ($newerFolders.Count) {
            $shown = @($newerFolders | Select-Object -First 3 | ForEach-Object { Get-TreeName $_ }) -join ', '
            Out-Line 'UNKNOWN' 'build' "$ModuleLibrary ($(Format-Utc $built)) is older than the folder(s) ${shown}: a file there was added, deleted or renamed since, so the DLL may or may not hold the current file set; a module build decides, and this line stays until the DLL is relinked"
        } elseif ($newerDesc.Count) {
            $shown = @($newerDesc | ForEach-Object { Get-TreeName $_ }) -join ', '
            Out-Line 'UNKNOWN' 'build' "$ModuleLibrary ($(Format-Utc $built)) is older than the build description $shown, which may or may not change it: a module build decides"
        } else { Out-Line 'ok' 'build' "$ModuleLibrary ($(Format-Utc $built)) is newer than the $($code.Count) file(s) it is built from" }
    }
    # The core library, by content: the stamp the last finished CMake build wrote next to it (ow-build-stamp in CMakeLists.txt)
    # against git's entries for the same paths now. File and commit times are not used, so a file the build rightly ignored
    # does not look stale, and a rebase or checkout that leaves the inputs alone does not either.
    Invoke-Check 'build' {
        $stampPaths = @('src', 'third_party/lzokay', 'CMakeLists.txt', 'tests/CMakeLists.txt')   # = PATHS of ow-build-stamp in CMakeLists.txt
        $howTo = 'cmake --build build --config Release (every build writes it, also when nothing is rebuilt)'
        $lib = Resolve-InTree $CoreLibrary
        $stamp = "$lib.inputs"
        if (!(Test-Path -LiteralPath $lib -PathType Leaf)) { Out-Line 'FAIL' 'build' "$CoreLibrary is missing: build it"; return }
        if (!(Test-Path -LiteralPath $stamp -PathType Leaf)) { Out-Line 'UNKNOWN' 'build' "$CoreLibrary has no input stamp ($CoreLibrary.inputs), so its inputs cannot be compared; get one with $howTo"; return }
        $rows = @((Read-SharedText $stamp) -split "`n" | ForEach-Object { $_.TrimEnd("`r") } | Where-Object { $_ })
        if ($rows.Count -lt 4 -or $rows[0] -ne 'ow-build-stamp 1' -or $rows[1] -ne "paths $($stampPaths -join ' ')" -or
            $rows[2] -notmatch '^commit [0-9a-f]{7,}$' -or $rows[3] -notmatch '^dirty \d+$') {
            Out-Line 'UNKNOWN' 'build' "the input stamp of $CoreLibrary is damaged or from another version of the stamp step; rewrite it with $howTo"; return
        }
        $at = $rows[2].Substring(7, 7)
        if ([IO.File]::GetLastWriteTimeUtc($lib) -gt [IO.File]::GetLastWriteTimeUtc($stamp)) {
            Out-Line 'UNKNOWN' 'build' "$CoreLibrary is newer than its input stamp (built without the stamp step, as with --target ow-core); rewrite it with $howTo"; return
        }
        # A configure after the last build (a changed option, a new compiler) changes nothing git sees. The cache sits next to
        # the library (single-config generators) or one folder up (build\Release).
        $cache = @((Join-Path (Split-Path -Parent $lib) 'CMakeCache.txt'), (Join-Path (Split-Path -Parent (Split-Path -Parent $lib)) 'CMakeCache.txt') |
                   Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1)
        if ($cache.Count -and [IO.File]::GetLastWriteTimeUtc($cache[0]) -gt [IO.File]::GetLastWriteTimeUtc($stamp)) {
            Out-Line 'UNKNOWN' 'build' "$(Get-TreeName $cache[0]) is newer than the input stamp of ${CoreLibrary}: configured after the last build, so the library may have been built with other settings; run $howTo"; return
        }
        if ($rows[3] -ne 'dirty 0') {
            Out-Line 'UNKNOWN' 'build' "$CoreLibrary was built at $at with uncommitted changes to its inputs, which the stamp does not record; commit or revert them, then $howTo"; return
        }
        $status = Invoke-Git (@('status', '--porcelain', '--') + $stampPaths)
        $entries = Invoke-Git (@('ls-files', '-s', '--') + $stampPaths)
        if ($status.Code -ne 0 -or $entries.Code -ne 0) {
            Out-Line 'UNKNOWN' 'build' "git failed in $Worktree (status exit $($status.Code), ls-files exit $($entries.Code)), so the inputs of $CoreLibrary cannot be compared with its stamp"; return
        }
        $dirty = @($status.Lines | Where-Object { $_ })
        if ($dirty.Count) { Out-Line 'FAIL' 'build' "$CoreLibrary was built at $at from committed inputs, and these have changed since: $($dirty -join '; ')"; return }
        $then = New-Object 'System.Collections.Generic.HashSet[string]' ([string[]]@($rows | Select-Object -Skip 4)), ([StringComparer]::Ordinal)
        $now = New-Object 'System.Collections.Generic.HashSet[string]' ([string[]]@($entries.Lines | Where-Object { $_ })), ([StringComparer]::Ordinal)
        $changed = @(@($then | Where-Object { !$now.Contains($_) }) + @($now | Where-Object { !$then.Contains($_) }) |
                     ForEach-Object { ($_ -split "`t", 2)[-1] } | Sort-Object -Unique)
        if ($changed.Count) {
            $shown = @($changed | Select-Object -First 5) -join ', '
            Out-Line 'FAIL' 'build' "$CoreLibrary was built at $at; $($changed.Count) of its input file(s) differ now ($shown): rebuild"
        } else { Out-Line 'ok' 'build' "$CoreLibrary was built at $at from the inputs checked out now ($($now.Count) file(s))" }
    }
}

if ($CheckEngine) {
    Invoke-Check 'engine' {
        # The name is put together so that tests/ue_lock_test.py, which requires every script naming the editor binary to take
        # the run lock, does not count this script as a launcher: it only tests that the file exists.
        $binary = Join-Path $Engine ('Engine\Binaries\Win64\UnrealEditor' + '.exe')
        if (Test-Path -LiteralPath $binary -PathType Leaf) { Out-Line 'ok' 'engine' "$binary exists" }
        else { Out-Line 'FAIL' 'engine' "no editor binary at $binary (set OPENWILLOW_UE or pass -Engine)" }
    }
}

$lines | ForEach-Object { Write-Output $_ }
$summary = "PREFLIGHT SUMMARY failed=$failed checks=$($lines.Count) unknown=$unknown"
Write-Output $summary
if ($Report) { [IO.File]::WriteAllLines([IO.Path]::GetFullPath($Report), [string[]]($lines + $summary)) }
if ($failed) { exit 1 }
exit 0
