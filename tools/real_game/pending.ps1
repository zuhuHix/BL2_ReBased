# Pending records for real-game runs (dot-sourced by realgame.ps1; needs its Get-TreeHashes, Compare-Hashes, Restore-VerifiedBackup).
#
# A run that blocks saving, launches the game or turns SDK mods off first writes a record under %LOCALAPPDATA%\OpenWillow\: the script, its
# PID and start time, the backup folder, each root's live folder and tree hash, and what it changed. The record survives a forced kill or a
# power cut, which skip every finally. It is removed only when the live roots are the backup again by tree hash, the backup itself is
# unchanged, no game runs and the driver folder is gone. Every capture starts with Assert-NoPendingRecords: any record left by an earlier run
# (this module's, the showcase capture's showcase_*pending*.json, the mission screen branch's mission_screen_mod_pending.json) stops it, with
# the exact restore steps, before it takes a lock or touches a file. Contains no game data.

$script:PendingDir = Join-Path $env:LOCALAPPDATA 'OpenWillow'
# Tests only. The folder moves only in an explicit synthetic test mode: OW_TEST_PENDING_DIR under the system temp folder AND
# OW_TEST_MODE=synthetic-real-game (tests/realgame_test_env.py sets both for every PowerShell run). In that mode the save folders and the game
# folder must be under the temp folder too (Assert-PendingTestRoots, checked at first use), so a run on the maintainer's saves can never keep its
# records in Temp. Either variable without the other, or a test folder elsewhere, stops the harness from loading.
$script:PendingTestMode = $false
$script:PendingTempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
if ($env:OW_TEST_PENDING_DIR -or $env:OW_TEST_MODE) {
    if (!$env:OW_TEST_PENDING_DIR -or $env:OW_TEST_MODE -cne 'synthetic-real-game') {
        throw "refusing to start: OW_TEST_PENDING_DIR='$env:OW_TEST_PENDING_DIR' and OW_TEST_MODE='$env:OW_TEST_MODE' are test settings and must come together (OW_TEST_MODE=synthetic-real-game). A real run must have neither: clear them, or real pending records would go to the wrong folder."
    }
    $testDir = [IO.Path]::GetFullPath($env:OW_TEST_PENDING_DIR)
    if (!$testDir.StartsWith($script:PendingTempRoot, [StringComparison]::OrdinalIgnoreCase)) { throw "refusing to start: OW_TEST_PENDING_DIR must be under $($script:PendingTempRoot), not $testDir" }
    $script:PendingDir = $testDir
    $script:PendingTestMode = $true
}
# In test mode every folder a record covers must be synthetic (under Temp): the saves, the game folder and the driver folder.
function Assert-PendingTestRoots([string[]]$More = @()) {
    if (!$script:PendingTestMode) { return }
    foreach ($dir in @($script:WillowDir, $script:Game, $script:DriverDir) + $More) {
        if (!$dir) { continue }
        $full = [IO.Path]::GetFullPath($dir)
        if (!$full.StartsWith($script:PendingTempRoot, [StringComparison]::OrdinalIgnoreCase)) {
            throw "refusing: test mode (OW_TEST_MODE) is on, but $full is not under $($script:PendingTempRoot). Test mode only runs on synthetic folders; a real run must not set OW_TEST_PENDING_DIR or OW_TEST_MODE."
        }
    }
}
$script:PendingRecord = $null   # this run's record, once written
$script:PendingKind = 'openwillow-realgame-pending'

# One SHA-256 over every file's relative path and SHA-256, in ordinal path order: the tree hash. 'absent' when the folder is gone.
function Get-TreeDigest([string]$Root) {
    if (!(Test-Path -LiteralPath $Root)) { return 'absent' }
    $hashes = Get-TreeHashes $Root
    [string[]]$names = @($hashes.Keys)
    [Array]::Sort($names, [StringComparer]::Ordinal)
    $text = ($names | ForEach-Object { "$_`t$($hashes[$_])" }) -join "`n"
    $sha = [Security.Cryptography.SHA256]::Create()
    try { -join ($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($text)) | ForEach-Object { $_.ToString('x2') }) } finally { $sha.Dispose() }
}

# The capture script that is running (the outermost script on the call stack), for the record and for the live-writer check.
function Get-RunScriptName {
    $top = Get-PSCallStack | Where-Object { $_.ScriptName } | Select-Object -Last 1
    if ($top) { [IO.Path]::GetFileNameWithoutExtension($top.ScriptName) } else { 'interactive' }
}

function Write-PendingFile([string]$Path, $Record) {
    New-Item -ItemType Directory -Force (Split-Path -Parent $Path) | Out-Null
    $tmp = "$Path.tmp"
    $Record | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $tmp -Encoding UTF8
    Move-Item -LiteralPath $tmp $Path -Force
    $back = Get-Content -Raw -LiteralPath $Path | ConvertFrom-Json
    if ($back.backup -cne $Record.backup -or @($back.changes).Count -ne @($Record.changes).Count) { throw "the pending record $Path did not read back: not going on" }
}

# Write (or rewrite, with more roots) this run's record. Called by Save-VerifiedBackupRecord, so every run that backs up has one before it
# installs the driver or launches. $Roots maps a root name to its live folder.
function Write-PendingRecord([string]$Backup, $Roots, [string[]]$Changes = @()) {
    Assert-PendingTestRoots @($Roots.Values)
    $rootRows = [ordered]@{}   # not $roots: PowerShell names are case-insensitive, and that would hide the parameter
    foreach ($name in $Roots.Keys) { $rootRows[$name] = [ordered]@{ live = [string]$Roots[$name]; tree_sha256 = (Get-TreeDigest $Roots[$name]) } }
    $old = if ($script:PendingRecord -and (Test-Path -LiteralPath $script:PendingRecord)) { Get-Content -Raw -LiteralPath $script:PendingRecord | ConvertFrom-Json } else { $null }
    $script_ = Get-RunScriptName
    $path = if ($script:PendingRecord) { $script:PendingRecord } else { Join-Path $script:PendingDir "realgame_run_pending-$script_-$PID.json" }
    $record = [ordered]@{
        kind = $script:PendingKind; version = 1; script = $script_; pid = $PID
        process_start = (Get-Process -Id $PID).StartTime.ToString('o')
        process_start_utc = (Get-Process -Id $PID).StartTime.ToUniversalTime().ToString('o')   # with pid, the writer's identity (Get-WriterState)
        written = $(if ($old) { $old.written } else { (Get-Date).ToString('o') }); updated = (Get-Date).ToString('o')
        backup = $Backup; roots = $rootRows; driver_dir = $script:DriverDir; game = $env:OPENWILLOW_BL2
        hub_lock = [string](Get-Variable -Name HubLock -ValueOnly -ErrorAction SilentlyContinue)
        lane = [string](Get-Variable -Name Lane -ValueOnly -ErrorAction SilentlyContinue)
        changes = @(@(if ($old) { @($old.changes) }) + @($Changes) | Where-Object { $_ })
    }
    Write-PendingFile $path $record
    $script:PendingRecord = $path
    $path
}

# Add one line to this run's record before the change it names (the driver's save block, a launch, mods off).
function Add-PendingChange([string]$What) {
    Assert-PendingRecorded $What
    $record = Get-Content -Raw -LiteralPath $script:PendingRecord | ConvertFrom-Json
    $record.changes = @(@($record.changes) + "$((Get-Date).ToString('o')) $What")
    $record.updated = (Get-Date).ToString('o')
    Write-PendingFile $script:PendingRecord $record
}
function Assert-PendingRecorded([string]$Doing) {
    if (!$script:PendingRecord -or !(Test-Path -LiteralPath $script:PendingRecord)) {
        throw "refusing $Doing`: no pending record has been written (Backup-Saves or Save-VerifiedBackupRecord writes it first)"
    }
}

# Why the live state is not yet what the record's run started from. Empty: restored and verified. The backup's own verified_hashes.json is the
# per-file truth (a backup that changed since proves nothing); this module's records also hold each root's tree hash, checked as well.
function Get-PendingProblems($Record) {
    $problems = @()
    $backup = [string]$Record.backup
    $file = if ($backup) { Join-Path $backup 'verified_hashes.json' } else { '' }
    if (!$backup -or !(Test-Path -LiteralPath $file)) { $problems += "no verified_hashes.json in '$backup'" }
    else {
        foreach ($root in (Get-Content -Raw -LiteralPath $file | ConvertFrom-Json).PSObject.Properties) {
            $state = $root.Value.snapshot; $live = $root.Value.live
            if ($state -is [string]) {
                if ($state -cne 'absent') { $problems += "$($root.Name): no usable snapshot ($state)" }
                elseif (Test-Path -LiteralPath $live) { $problems += "$($root.Name): absent at the backup, present now: $live" }
                continue
            }
            if ($null -eq $state) { $problems += "$($root.Name): no snapshot"; continue }
            $hashes = [hashtable]::new([StringComparer]::Ordinal)   # paths compare case-sensitively
            foreach ($f in $state.PSObject.Properties) { $hashes[$f.Name] = $f.Value }
            $inBackup = @(Compare-Hashes $hashes (Get-TreeHashes (Join-Path $backup $root.Name)))
            if ($inBackup.Count) { $problems += "$($root.Name): the backup changed after it was verified ($($inBackup.Count) files, first $($inBackup[0]))" }
            $inLive = @(Compare-Hashes $hashes (Get-TreeHashes $live))
            if ($inLive.Count) { $problems += "$($root.Name): differs from the backup ($($inLive.Count) files, first $($inLive[0]))" }
        }
    }
    if ($Record.kind -ceq $script:PendingKind -and $Record.roots) {
        foreach ($root in $Record.roots.PSObject.Properties) {
            $now = Get-TreeDigest $root.Value.live
            if ($now -cne $root.Value.tree_sha256) { $problems += "$($root.Name): tree hash $now, recorded $($root.Value.tree_sha256)" }
        }
    }
    $driver = if ($Record.driver_dir) { [string]$Record.driver_dir } else { $script:DriverDir }
    if ($driver -and (Test-Path -LiteralPath $driver)) { $problems += "the driver folder is still there: $driver" }
    @($problems | Select-Object -Unique)
}

# True only for a folder that holds this checkout's driver and nothing a person added: the one thing safe to remove without asking.
function Test-IsOurDriverCopy([string]$Dir) {
    if (!(Test-Path -LiteralPath $Dir)) { return $false }
    $mine = Join-Path $PSScriptRoot 'openwillow_realgame/__init__.py'
    $theirs = Join-Path $Dir '__init__.py'
    if (!(Test-Path -LiteralPath $theirs) -or (Get-FileSha256 $theirs) -cne (Get-FileSha256 $mine)) { return $false }
    $known = '__init__.py', 'cmd_dir.txt', 'autoblock.flag', '.owrun', '__pycache__'
    !@(Get-ChildItem -LiteralPath $Dir -Force | Where-Object { $_.Name -notin $known }).Count
}

# Is the run that wrote a record still running? Identity is the PID and the process start time in UTC, never the command line (a run from an
# ISE or an interactive shell has no script name there). 'gone': no process has that PID, or the one that has it started at another time.
# 'alive': same PID, same start time. 'unknown': it cannot be shown either way (no PID, no start time, an unreadable process); treated as alive.
function Get-WriterState($Record) {
    if (!$Record.pid) { return 'unknown' }
    $id = 0
    if (![int]::TryParse([string]$Record.pid, [ref]$id) -or $id -le 0) { return 'unknown' }
    $p = Get-Process -Id $id -ErrorAction SilentlyContinue
    if (!$p) { return 'gone' }
    $started = try { $p.StartTime.ToUniversalTime() } catch { $null }
    if ($null -eq $started) { return 'unknown' }
    $recorded = $null
    if ($Record.process_start_utc) {
        $parsed = [datetime]::MinValue
        if ([datetime]::TryParse([string]$Record.process_start_utc, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::RoundtripKind, [ref]$parsed)) { $recorded = $parsed.ToUniversalTime() }
    }
    if ($null -eq $recorded) { return 'unknown' }
    if ([math]::Abs(($started - $recorded).TotalSeconds) -le 1) { 'alive' } else { 'gone' }
}

function Get-PendingFiles { Assert-PendingTestRoots; if (Test-Path -LiteralPath $script:PendingDir) { @(Get-ChildItem -LiteralPath $script:PendingDir -Filter '*pending*.json' -File -Force) } else { @() } }

# The exact steps for a record this run cannot complete by itself.
function Format-PendingRecovery([string]$File, $Record, [string[]]$Problems) {
    $nl = [Environment]::NewLine
    $lines = @("PENDING RECORD: an earlier real-game run did not finish: $File",
        "  Written by $($Record.script) (pid $($Record.pid)) at $($Record.written); backup $($Record.backup).")
    foreach ($c in @($Record.changes)) { $lines += "  It changed: $c" }
    foreach ($p in $Problems) { $lines += "  Not yet back: $p" }
    $lines += @('  This run changed nothing and starts nothing until the record is gone. To recover:',
        '  1. Make sure Borderlands 2 is closed. The restore puts the saves back as they were before that run. It first copies the current saves and',
        '     settings to a pre-restore folder beside the backup and verifies that copy by tree hash, so anything played since is kept there, not lost.',
        "  2. In a new PowerShell window: `$env:OPENWILLOW_BL2 = '$(if ($Record.game) { $Record.game } else { $env:OPENWILLOW_BL2 })'; . '$PSScriptRoot\realgame.ps1'",
        "     Restore-PendingRecord '$File'",
        '     It restores every root from the verified backup (a backup that changed since is refused), removes the driver folder only if it is this',
        '     driver''s untouched copy, compares every root by tree hash and removes the record only when all match. Anything else it prints stays yours to judge.')
    $locks = @()
    if ($Record.hub_lock) { $locks += "'$($Record.hub_lock)' (if it names '$($Record.lane)')" }
    $locks += "'$(Get-OwUeLockPath)' (if it is that run's)"
    $lines += "  3. Only then remove the locks that run left: $($locks -join ' and '). Then run the capture again."
    $lines -join $nl
}

# First thing in every capture, before any lock, backup or launch. Refuses while any record from an earlier run is there, unless what is left
# can be completed and verified here: a record whose roots already match its backup by tree hash, or one whose only leftover is this driver's
# untouched copy. The saves are never rolled back here: a person may have played since, and only they can say that is fine (Restore-PendingRecord).
function Assert-NoPendingRecords {
    foreach ($f in (Get-PendingFiles)) {
        if ($script:PendingRecord -and $f.FullName -eq [IO.Path]::GetFullPath($script:PendingRecord)) { continue }
        $text = Get-Content -Raw -LiteralPath $f.FullName
        $record = try { $text | ConvertFrom-Json } catch { $null }
        if (!$record -or !$record.backup) {
            throw "PENDING RECORD: an earlier run left $($f.FullName) and it names no backup, so no restore can be printed or checked. Read it, recover what it names by hand (or with the script that wrote it), then move it out of $script:PendingDir. This run changed nothing. Its contents:$([Environment]::NewLine)$text"
        }
        # Nothing is completed while the writer may still run: alive, or not shown gone (no start time, unreadable). Fail closed.
        $writer = Get-WriterState $record
        if ($writer -eq 'alive') { throw "Pid $($record.pid) is still the run that wrote $($f.FullName) (same start time). Wait for it to finish. This run changed nothing." }
        if (@(Get-GameProcesses).Count) { throw "$($f.FullName) is there and Borderlands 2 is running: nothing can be verified under a live game. Close it first. This run changed nothing." }
        if ($writer -ne 'gone') {
            throw (Format-PendingRecovery $f.FullName $record @("the run that wrote it (pid $($record.pid)) cannot be shown to have ended: no recorded start time, or its process cannot be read. Nothing is completed automatically; when you are sure it has ended, use Restore-PendingRecord with -WriterGone"))
        }
        $problems = @(Get-PendingProblems $record)
        $driver = if ($record.driver_dir) { [string]$record.driver_dir } else { $script:DriverDir }
        $driverOnly = $problems.Count -and !@($problems | Where-Object { $_ -notlike 'the driver folder is still there*' }).Count
        if ($driverOnly -and (Test-IsOurDriverCopy $driver)) {
            Remove-Item -LiteralPath $driver -Recurse -Force
            "Earlier run's record $($f.FullName): its roots match the backup; removed the driver copy it left at $driver."
            $problems = @(Get-PendingProblems $record)
        }
        if ($problems.Count) { throw (Format-PendingRecovery $f.FullName $record $problems) }
        Remove-Item -LiteralPath $f.FullName -Force
        "Earlier run's record $($f.FullName): its backup $($record.backup) matches the live roots by tree hash. Record removed."
    }
}

# The manual way back for a record (Format-PendingRecovery prints the call): restore from the verified backup, verify, then remove the record.
# Before anything is restored, the current state of every root is copied to a new pre-restore folder beside the backup and that copy is verified
# by tree hash: progress made after the failed run is never lost, only set aside. -WriterGone: the person confirms the writer has ended when that
# cannot be shown (Get-WriterState 'unknown'); a writer shown alive is always refused.
function Restore-PendingRecord([string]$File, [switch]$WriterGone) {
    if (!(Test-Path -LiteralPath $File)) { throw "no record at $File" }
    $record = Get-Content -Raw -LiteralPath $File | ConvertFrom-Json
    if (@(Get-GameProcesses).Count) { throw 'Borderlands 2 is running: close it first. Nothing was changed.' }
    $writer = Get-WriterState $record
    if ($writer -eq 'alive') { throw "pid $($record.pid) is still the run that wrote $File (same start time); wait for it. Nothing was changed." }
    if ($writer -ne 'gone' -and !$WriterGone) { throw "the run that wrote $File (pid $($record.pid)) cannot be shown to have ended. If you are sure it has, run Restore-PendingRecord '$File' -WriterGone. Nothing was changed." }
    $aside = Save-PreRestoreSnapshot ([string]$record.backup)
    "the current saves and settings were copied, verified by tree hash, to $aside (anything played since that run is there)"
    $driver = if ($record.driver_dir) { [string]$record.driver_dir } else { $script:DriverDir }
    if (Test-Path -LiteralPath $driver) {
        if (Test-IsOurDriverCopy $driver) { Remove-Item -LiteralPath $driver -Recurse -Force; "removed the driver copy at $driver" }
        else { "the driver folder $driver is not this driver's untouched copy: left for you" }
    }
    Restore-VerifiedBackup ([string]$record.backup)
    $problems = @(Get-PendingProblems $record)
    if ($problems.Count) { throw "record kept at ${File}: $($problems -join '; ')" }
    Remove-Item -LiteralPath $File -Force
    "verified by tree hash; record $File removed"
}

# Copy every live root the backup's verified_hashes.json names to <backup's folder>\pre-restore-<time>\<root> and prove each copy by tree
# hash, or throw before anything is restored. Returns the folder.
function Save-PreRestoreSnapshot([string]$Backup) {
    $file = Join-Path $Backup 'verified_hashes.json'
    if (!(Test-Path -LiteralPath $file)) { throw "no verified_hashes.json in $Backup; not restoring from it. Nothing was changed." }
    $aside = Join-Path (Split-Path -Parent $Backup) ('pre-restore-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + "-$PID")
    New-Item -ItemType Directory -Path $aside -ErrorAction Stop | Out-Null
    $proof = [ordered]@{}
    foreach ($root in (Get-Content -Raw -LiteralPath $file | ConvertFrom-Json).PSObject.Properties) {
        $live = [string]$root.Value.live
        if (!(Test-Path -LiteralPath $live)) { $proof[$root.Name] = 'absent'; continue }
        $copy = Join-Path $aside $root.Name
        Copy-Item -Recurse -Force -LiteralPath $live $copy
        $want = Get-TreeDigest $live; $got = Get-TreeDigest $copy
        if ($want -cne $got) { throw "the pre-restore copy of $live in $copy does not match it by tree hash; not restoring. Nothing was restored." }
        $proof[$root.Name] = [ordered]@{ live = $live; tree_sha256 = $got }
    }
    $proof | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $aside 'pre_restore.json') -Encoding UTF8
    $aside
}

# In a run's cleanup, after its restore: the record goes only when every check passes. Returns what happened, for the run's log; anything but
# 'removed...' or 'none written' is a reason for recovery.
function Complete-PendingRecord([bool]$Safe = $true) {
    if (!$script:PendingRecord) { return 'none written' }
    if (!(Test-Path -LiteralPath $script:PendingRecord)) { $script:PendingRecord = $null; return 'none written' }
    if (!$Safe) { return "KEPT at $($script:PendingRecord): a game may still run" }
    try {
        if (@(Get-GameProcesses).Count) { return "KEPT at $($script:PendingRecord): Borderlands 2 is running" }
        $record = Get-Content -Raw -LiteralPath $script:PendingRecord | ConvertFrom-Json
        $problems = @(Get-PendingProblems $record)
        if ($problems.Count) { return "KEPT at $($script:PendingRecord): $($problems -join '; ')" }
        Remove-Item -LiteralPath $script:PendingRecord -Force
        $done = "removed: verified by tree hash ($($script:PendingRecord))"
        $script:PendingRecord = $null
        $done
    } catch { "KEPT at $($script:PendingRecord): the check failed: $_" }
}

# SDK mods off, the shared way: the settings root must be in this run's verified backup and record (it is restored from there), and the record
# says so before the first write. Every setting that says "enabled": true is rewritten to say false, all checked before any is written. Returns
# the stems turned off.
function Set-SdkModsOff([string]$SettingsDir = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods\settings')) {
    Assert-PendingRecorded 'turning SDK mods off'
    $record = Get-Content -Raw -LiteralPath $script:PendingRecord | ConvertFrom-Json
    $full = [IO.Path]::GetFullPath($SettingsDir).TrimEnd('\')
    $covered = @($record.roots.PSObject.Properties | Where-Object { [IO.Path]::GetFullPath($_.Value.live).TrimEnd('\') -eq $full -and $_.Value.tree_sha256 -ne 'absent' })
    if (!$covered.Count) { throw "refusing to turn SDK mods off: $SettingsDir is not a backed-up root of this run's record" }
    $plan = @()
    foreach ($file in Get-ChildItem -LiteralPath $SettingsDir -Filter *.json -File) {
        $text = [Text.UTF8Encoding]::new($false, $true).GetString([IO.File]::ReadAllBytes($file.FullName))
        $json = $text | ConvertFrom-Json
        if ($json.PSObject.Properties.Name -notcontains 'enabled' -or $json.enabled -ne $true) { continue }
        $found = [regex]::Matches($text, '"enabled"\s*:\s*true')
        if ($found.Count -ne 1) { throw "$($file.Name) has $($found.Count) enabled values; not changing any setting" }
        $new = $text.Substring(0, $found[0].Index) + ($found[0].Value -replace 'true$', 'false') + $text.Substring($found[0].Index + $found[0].Length)
        $plan += [pscustomobject]@{ file = $file.FullName; stem = $file.BaseName; bytes = [Text.UTF8Encoding]::new($false).GetBytes($new) }
    }
    if (!$plan.Count) { return @() }
    Add-PendingChange "SDK mods off in ${SettingsDir}: $(@($plan | ForEach-Object { $_.stem }) -join ', ') (restored from the backup's $($covered[0].Name))"
    foreach ($row in $plan) { [IO.File]::WriteAllBytes($row.file, $row.bytes) }
    @($plan | ForEach-Object { $_.stem })
}
