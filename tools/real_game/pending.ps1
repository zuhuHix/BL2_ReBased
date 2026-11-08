# Pending records for real-game runs (dot-sourced by realgame.ps1; needs its Get-TreeHashes, Compare-Hashes, Restore-VerifiedBackup).
#
# A run that blocks saving, launches the game or turns SDK mods off first writes a record under %LOCALAPPDATA%\OpenWillow\: the script, its
# PID and start time, the backup folder, each root's live folder and tree hash, and what it changed. The record survives a forced kill or a
# power cut, which skip every finally. It is removed only when the live roots are the backup again by tree hash, the backup itself is
# unchanged, no game runs and the driver folder is gone. Every capture starts with Assert-NoPendingRecords: any record left by an earlier run
# (this module's, the showcase capture's showcase_*pending*.json, the mission screen branch's mission_screen_mod_pending.json) stops it, with
# the exact restore steps, before it takes a lock or touches a file. Contains no game data.

$script:PendingDir = Join-Path $env:LOCALAPPDATA 'OpenWillow'
# Tests only: OW_TEST_PENDING_DIR moves the folder, and only to somewhere under the system temp folder, so a test never reads or writes the
# maintainer's records (tests/realgame_test_env.py sets a fresh one for every PowerShell run).
if ($env:OW_TEST_PENDING_DIR) {
    $testDir = [IO.Path]::GetFullPath($env:OW_TEST_PENDING_DIR); $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    if (!$testDir.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase)) { throw "OW_TEST_PENDING_DIR must be under $tempRoot, not $testDir" }
    $script:PendingDir = $testDir
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
    $rootRows = [ordered]@{}   # not $roots: PowerShell names are case-insensitive, and that would hide the parameter
    foreach ($name in $Roots.Keys) { $rootRows[$name] = [ordered]@{ live = [string]$Roots[$name]; tree_sha256 = (Get-TreeDigest $Roots[$name]) } }
    $old = if ($script:PendingRecord -and (Test-Path -LiteralPath $script:PendingRecord)) { Get-Content -Raw -LiteralPath $script:PendingRecord | ConvertFrom-Json } else { $null }
    $script_ = Get-RunScriptName
    $path = if ($script:PendingRecord) { $script:PendingRecord } else { Join-Path $script:PendingDir "realgame_run_pending-$script_-$PID.json" }
    $record = [ordered]@{
        kind = $script:PendingKind; version = 1; script = $script_; pid = $PID
        process_start = (Get-Process -Id $PID).StartTime.ToString('o')
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
            $hashes = @{}
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
    if (!(Test-Path -LiteralPath $theirs) -or (Get-FileHash -LiteralPath $theirs).Hash -ne (Get-FileHash -LiteralPath $mine).Hash) { return $false }
    $known = '__init__.py', 'cmd_dir.txt', 'autoblock.flag', '.owrun', '__pycache__'
    !@(Get-ChildItem -LiteralPath $Dir -Force | Where-Object { $_.Name -notin $known }).Count
}

function Get-PendingFiles { if (Test-Path -LiteralPath $script:PendingDir) { @(Get-ChildItem -LiteralPath $script:PendingDir -Filter '*pending*.json' -File -Force) } else { @() } }

# The exact steps for a record this run cannot complete by itself.
function Format-PendingRecovery([string]$File, $Record, [string[]]$Problems) {
    $nl = [Environment]::NewLine
    $lines = @("PENDING RECORD: an earlier real-game run did not finish: $File",
        "  Written by $($Record.script) (pid $($Record.pid)) at $($Record.written); backup $($Record.backup).")
    foreach ($c in @($Record.changes)) { $lines += "  It changed: $c" }
    foreach ($p in $Problems) { $lines += "  Not yet back: $p" }
    $lines += @('  This run changed nothing and starts nothing until the record is gone. To recover:',
        '  1. Make sure Borderlands 2 is closed. The restore puts the saves back as they were before that run: anything played since is lost, so copy',
        "     '$([Environment]::GetFolderPath('MyDocuments'))\My Games\Borderlands 2\WillowGame\SaveData' aside first if you want to keep it.",
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
        if ($record.pid -and [int]$record.pid -ne $PID) {
            $alive = Get-CimInstance Win32_Process -Filter "ProcessId=$([int]$record.pid)" -ErrorAction SilentlyContinue
            $by = if ($record.script) { [string]$record.script } else { ($f.BaseName -split '_')[0] }
            if ($alive -and $alive.CommandLine -and $alive.CommandLine.Contains($by)) { throw "Pid $($record.pid) still looks like the run ($by) that wrote $($f.FullName). Wait for it to finish. This run changed nothing." }
        }
        if (@(Get-GameProcesses).Count) { throw "$($f.FullName) is there and Borderlands 2 is running: nothing can be verified under a live game. Close it first. This run changed nothing." }
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
function Restore-PendingRecord([string]$File) {
    if (!(Test-Path -LiteralPath $File)) { throw "no record at $File" }
    $record = Get-Content -Raw -LiteralPath $File | ConvertFrom-Json
    if (@(Get-GameProcesses).Count) { throw 'Borderlands 2 is running: close it first. Nothing was changed.' }
    if ($record.pid -and [int]$record.pid -ne $PID) {
        $alive = Get-CimInstance Win32_Process -Filter "ProcessId=$([int]$record.pid)" -ErrorAction SilentlyContinue
        if ($alive -and $alive.CommandLine -and $record.script -and $alive.CommandLine.Contains([string]$record.script)) { throw "pid $($record.pid) still looks like the run that wrote $File; wait for it. Nothing was changed." }
    }
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
