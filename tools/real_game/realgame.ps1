# OpenWillow real-game helpers (dot-source: . tools/real_game/realgame.ps1).
# Drives the installed Borderlands 2 for ground-truth captures: lock, launch, window capture,
# scan-code key input and the openwillow_realgame SDK command channel. Contains no game data;
# everything it writes goes under the repository's ignored local/realgame/.
$ErrorActionPreference = 'Stop'
$script:Repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$script:RG = Join-Path $script:Repo 'local/realgame'
$script:CmdDir = Join-Path $script:RG 'cmd'
. (Join-Path $PSScriptRoot '../ue_lock.ps1')
$script:Lock = Get-OwUeLockPath
$script:Game = $env:OPENWILLOW_BL2
# The game's WillowGame folder under the shell's Documents folder (which may be redirected away from %USERPROFILE%).
$script:WillowDir = Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'My Games/Borderlands 2/WillowGame'
$script:DriverDir = Join-Path $script:Game 'sdk_mods/openwillow_realgame'
# Pending records: what a run changed, written before it changes it (pending.ps1).
. (Join-Path $PSScriptRoot 'pending.ps1')
New-Item -ItemType Directory -Force $script:CmdDir | Out-Null

Add-Type -AssemblyName System.Drawing
if (-not ('OWRG.Native' -as [type])) {
Add-Type -Namespace OWRG -Name Native -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
[DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
[DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
[DllImport("user32.dll")] public static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
[DllImport("user32.dll")] public static extern bool AllowSetForegroundWindow(int pid);
[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
[StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
'@
}
[OWRG.Native]::SetProcessDPIAware() | Out-Null

function Get-QpcSeconds { [double][Diagnostics.Stopwatch]::GetTimestamp() / [Diagnostics.Stopwatch]::Frequency }

function Enter-RunLock([int]$WaitSeconds = 600) {
    # The machine-wide claim every editor script uses (tools/ue_lock.ps1): no Unreal editor or Borderlands 2, lock free or stale, CreateNew.
    Enter-OwUeLock -Owner 'realgame' -TimeoutSeconds $WaitSeconds -AlsoWaitFor Borderlands2   # no game or editor already running
}
function Exit-RunLock { Exit-OwUeLock }   # removes the lock only when this process owns it

# Every folder a game launch can rewrite: the saves, the config, the SDK's mod settings and Sanity Saver's store (an earlier session saw it
# rewrite one). The names match the captures' own $roots, so their later Save-VerifiedBackupRecord covers the same folders.
function Get-RunRoots {
    [ordered]@{ SaveData = (Join-Path $script:WillowDir 'SaveData'); Config = (Join-Path $script:WillowDir 'Config')
        sdk_settings = (Join-Path $script:Game 'sdk_mods/settings'); SanitySaver = (Join-Path $script:Game 'sdk_mods/SanitySaver') }
}
# Copy every run root to a new backup folder, prove each copy by hash (a root that does not exist is recorded as absent), and write the backup's
# verified_hashes.json and this run's pending record (Save-VerifiedBackupRecord). Refuses first while an earlier run's record is left
# (Assert-NoPendingRecords). Returns the folder.
function Backup-Saves {
    Assert-NoPendingRecords | ForEach-Object { Write-Host $_ }
    $dst = Join-Path $script:RG ('save-backup-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
    New-Item -ItemType Directory -Force $dst | Out-Null
    $roots = Get-RunRoots
    $snapshots = @{}
    foreach ($name in $roots.Keys) {
        if (!(Test-Path -LiteralPath $roots[$name])) { $snapshots[$name] = 'absent'; continue }
        $snapshots[$name] = New-VerifiedSnapshot $roots[$name] (Join-Path $dst $name)
    }
    Save-VerifiedBackupRecord $dst $roots $snapshots
    $dst
}

# Copy the command-channel mod into the game's sdk_mods (removed again by Remove-Driver). Always exclusive: refuse when bl-sdk is absent or the
# folder already exists and is not this run's, so a folder this run did not install is never overwritten or removed. -Exclusive is still
# accepted and changes nothing. -ClaimedBy <id>: the caller created the folder first, holding only a .owrun file with that id (the claim in
# projectiles_capture and weapon_parity2_capture); that folder is this run's. -BlockSavesAtStart: the driver blocks the save functions as soon
# as it loads (its autoblock.flag). The install is written into the pending record before the folder is touched.
$script:DriverInstalled = $false   # true once this process installed (or claimed) the folder; Remove-Driver removes nothing otherwise
$script:DriverOwner = ''           # the owner id in its .owrun and in the pending record
function Install-Driver([switch]$BlockSavesAtStart, [switch]$Exclusive, [string]$ClaimedBy = '') {
    $target = $script:DriverDir
    if (!(Test-Path -LiteralPath (Join-Path $script:Game 'sdk_mods'))) { throw "bl-sdk is not installed: $(Join-Path $script:Game 'sdk_mods') is missing" }
    $claimed = $false
    if (Test-Path -LiteralPath $target) {
        $claimed = [bool]$ClaimedBy -and (Test-DriverClaim $target $ClaimedBy)
        if (!$claimed) { throw "$target already exists and is not this run's; not overwriting it (remove it by hand if it is a stale copy of this driver)" }
    }
    # The record first: the driver's save block and command channel stay in the game's folder if this run is killed. Its owner id goes into the
    # record and the folder's .owrun: only a folder whose .owrun matches the record is ever removed without asking.
    $owner = if ($claimed) { $ClaimedBy } else { "$(Get-RunScriptName)-$PID-$([guid]::NewGuid().ToString('N'))" }
    Set-PendingField 'driver_owner' $owner
    Add-PendingChange "driver installed at $target (save block from the first frame: $([bool]$BlockSavesAtStart); owner $owner)"
    try {
        if (!$claimed) {
            New-Item -ItemType Directory -Path $target -ErrorAction Stop | Out-Null   # no -Force: a folder that appeared since is refused
            [IO.File]::WriteAllText((Join-Path $target '.owrun'), $owner)
        }
        $script:DriverInstalled = $true; $script:DriverOwner = $owner
        Copy-Item (Join-Path $PSScriptRoot 'openwillow_realgame/__init__.py') $target -Force
        Set-Content -LiteralPath (Join-Path $target 'cmd_dir.txt') -Value $script:CmdDir -NoNewline
        if ($BlockSavesAtStart) { Set-Content -LiteralPath (Join-Path $target 'autoblock.flag') -Value '1' -NoNewline }
    } catch {
        # Only this run's folder is removed again (a partial driver would make the next run refuse).
        if ($script:DriverInstalled) { Remove-Item -LiteralPath $target -Recurse -Force -ErrorAction SilentlyContinue; $script:DriverInstalled = $false }
        throw
    }
}
# A caller's claim: the folder holds only .owrun, and it reads exactly $Id.
function Test-DriverClaim([string]$Dir, [string]$Id) {
    $items = @(Get-ChildItem -LiteralPath $Dir -Force)
    $owner = Join-Path $Dir '.owrun'
    $items.Count -eq 1 -and $items[0].Name -ceq '.owrun' -and ([IO.File]::ReadAllText($owner) -ceq $Id)
}
# Remove the driver folder only when this process installed it and it still holds nothing but this driver's files. Anything else is left and
# reported, and the pending record's check (driver folder still there) keeps the record and the locks.
function Remove-Driver {
    $target = $script:DriverDir
    if (!(Test-Path -LiteralPath $target)) { $script:DriverInstalled = $false; return }
    if (!$script:DriverInstalled) { Write-Warning "not removing ${target}: this run did not install it"; return }
    if (!(Test-IsOurDriverCopy $target $script:DriverOwner -AllowPartial)) { Write-Warning "not removing ${target}: its owner file or contents are not this run's"; return }
    Remove-Item -LiteralPath $target -Recurse -Force
    $script:DriverInstalled = $false
}

# SHA-256 of a file as upper-case hex (what Get-FileHash printed, so older verified_hashes.json files still compare). .NET only, so the
# harness does not depend on the Microsoft.PowerShell.Utility module loading in a child shell.
function Get-FileSha256([string]$Path) {
    $full = (Resolve-Path -LiteralPath $Path).ProviderPath
    $sha = [Security.Cryptography.SHA256]::Create(); $stream = [IO.File]::OpenRead($full)
    try { -join ($sha.ComputeHash($stream) | ForEach-Object { $_.ToString('X2') }) } finally { $stream.Dispose(); $sha.Dispose() }
}

# Make the live tree $Live identical to the backup tree $Backup: files added during the run are removed, missing and changed
# files are copied back, directories created during the run are removed when empty. Then every file is compared again by
# SHA-256. Returns one row per difference found; a row with status 'MISMATCH after restore' means the restore failed.
# A missing backup folder is refused before $Live is touched: Get-TreeHashes reads it as empty, which would delete every live file.
function Restore-Tree([string]$Backup, [string]$Live, [string]$Label) {
    $Backup = [IO.Path]::GetFullPath($Backup).TrimEnd('\'); $Live = [IO.Path]::GetFullPath($Live).TrimEnd('\')
    if (!(Test-Path -LiteralPath $Backup -PathType Container)) { throw "backup folder $Backup is missing; $Live was not touched" }
    $rel = { param($Root, $Path) $Path.Substring($Root.Length + 1) }
    $row = { param($File, $Status) [pscustomobject]@{ file = "$Label\$File"; status = $Status } }
    if (!(Test-Path -LiteralPath $Live)) { New-Item -ItemType Directory -Force $Live | Out-Null }
    # Paths compare case-sensitively: a file renamed only in case counts as added (removed) and missing (copied back under its old name).
    $want = Get-TreeHashes $Backup
    foreach ($f in @(Get-ChildItem -LiteralPath $Live -Recurse -File -Force)) {
        $r = & $rel $Live $f.FullName
        if (!$want.ContainsKey($r)) { Remove-Item -LiteralPath $f.FullName -Force; & $row $r 'added, removed' }
    }
    foreach ($f in @(Get-ChildItem -LiteralPath $Backup -Recurse -File -Force)) {
        $r = & $rel $Backup $f.FullName; $dst = Join-Path $Live $r
        if (!(Test-Path -LiteralPath $dst -PathType Leaf)) {
            New-Item -ItemType Directory -Force (Split-Path $dst) | Out-Null; Copy-Item -LiteralPath $f.FullName $dst -Force; & $row $r 'missing, restored'
        } elseif ($want[$r] -cne (Get-FileSha256 $dst)) {
            Copy-Item -LiteralPath $f.FullName $dst -Force; & $row $r 'changed, restored'
        }
    }
    Get-ChildItem -LiteralPath $Live -Recurse -Directory -Force | Sort-Object { $_.FullName.Length } -Descending | ForEach-Object {
        $r = & $rel $Live $_.FullName
        if (!(Test-Path -LiteralPath (Join-Path $Backup $r)) -and !(Get-ChildItem -LiteralPath $_.FullName -Force)) { Remove-Item -LiteralPath $_.FullName -Force }
    }
    foreach ($r in (Compare-TreeHashes $Backup $Live)) { & $row $r 'MISMATCH after restore' }
}
# Relative paths whose presence or SHA-256 differs between two trees (empty when they are identical).
function Compare-TreeHashes([string]$A, [string]$B) {
    Compare-Hashes (Get-TreeHashes $A) (Get-TreeHashes $B)
}
# Relative path to SHA-256 for every file under $Root; empty when the folder is gone. Keys are ordinal (case-sensitive): two paths that differ
# only in case are two entries, and a rename that changes only case is a difference.
function Get-TreeHashes([string]$Root) {
    $Root = [IO.Path]::GetFullPath($Root).TrimEnd('\'); $h = [hashtable]::new([StringComparer]::Ordinal)
    if (Test-Path -LiteralPath $Root) {
        Get-ChildItem -LiteralPath $Root -Recurse -File -Force | ForEach-Object { $h[$_.FullName.Substring($Root.Length + 1)] = Get-FileSha256 $_.FullName }
    }
    $h
}
# Every path, compared ordinally, whose presence or hash differs between two path-to-hash tables.
function Compare-Hashes([hashtable]$A, [hashtable]$B) {
    $keys = [Collections.Generic.SortedSet[string]]::new([StringComparer]::Ordinal)
    foreach ($k in $A.Keys) { [void]$keys.Add([string]$k) }
    foreach ($k in $B.Keys) { [void]$keys.Add([string]$k) }
    foreach ($k in $keys) { if (!(Test-HashKey $A $k) -or !(Test-HashKey $B $k) -or [string]$A[$k] -cne [string]$B[$k]) { $k } }
}
# Exact (ordinal) key presence, whatever comparer the table was made with.
function Test-HashKey([hashtable]$Table, [string]$Key) { foreach ($k in $Table.Keys) { if ([string]$k -ceq $Key) { return $true } }; $false }

# Copy $Live to the new folder $Snapshot and prove the copy matches, or throw. Returns the snapshot's hashes: store them for
# Restore-VerifiedRoots, because only a root that passed may be restored from. Restore-Tree deletes live files absent from its backup,
# so a partial snapshot, or one that changed later, would lose them.
# A snapshot folder that already exists (Backup-Saves made it) is not copied into again: it is only proved against the live folder.
function New-VerifiedSnapshot([string]$Live, [string]$Snapshot) {
    if (!(Test-Path -LiteralPath $Snapshot)) { Copy-Item -Recurse -Force -LiteralPath $Live $Snapshot }
    Assert-SnapshotMatches $Live $Snapshot
}
function Assert-SnapshotMatches([string]$Live, [string]$Snapshot) {
    $hashes = Get-TreeHashes $Snapshot
    $diff = @(Compare-Hashes (Get-TreeHashes $Live) $hashes)
    if ($diff.Count) { throw "snapshot $Snapshot differs from $Live in $($diff.Count) files, first $($diff[0])" }
    $hashes
}

# Restore each root from $Backup\<name>. $Roots maps name to live folder; $Snapshots says what the backup holds for it: the hashes
# New-VerifiedSnapshot returned (re-checked first; a backup that changed since is not restored from), 'absent' (the root did not exist
# at the backup) or anything else (no usable snapshot: never restored from). A root absent at the backup and present now is reported and
# not deleted. Returns rows, a summary, changed (the roots whose backup changed) and clean: true only when every root is identical to its
# backup or stayed absent. A root reported and left alone, or one with no usable snapshot, is not clean.
function Restore-VerifiedRoots([string]$Backup, $Roots, $Snapshots) {
    Assert-PendingTestRoots (@($Backup) + @($Roots.Values))   # in test mode, every folder it may write must be synthetic
    $rows = @(); $notes = @(); $changed = @()
    foreach ($name in $Roots.Keys) {
        $live = $Roots[$name]
        try {
            if ($Snapshots[$name] -is [hashtable]) {
                $b = Join-Path $Backup $name
                $diff = @(Compare-Hashes $Snapshots[$name] (Get-TreeHashes $b))
                if (!(Test-Path -LiteralPath $b -PathType Container)) {
                    # Get-TreeHashes reads a missing folder as empty: a verified empty root would otherwise pass and reach Restore-Tree.
                    $rows += [pscustomobject]@{ file = $name; status = "BACKUP CHANGED / MISSING after verification ($b is gone); not restored" }
                    $changed += $name
                } elseif ($diff.Count) {
                    $rows += [pscustomobject]@{ file = $name; status = "BACKUP CHANGED after verification ($($diff.Count) files, first $($diff[0])); not restored" }
                    $changed += $name
                } else { $rows += @(Restore-Tree $b $live $name) }
            } elseif ($Snapshots[$name] -eq 'absent') {
                if (Test-Path -LiteralPath $live) {
                    $rows += [pscustomobject]@{ file = $name; status = 'created during the run; left for the maintainer' }
                    $notes += "$name created during the run"
                }
            } else {
                $rows += [pscustomobject]@{ file = $name; status = 'no verified snapshot; not restored' }
                $notes += "$name not backed up"
            }
        } catch { $rows += [pscustomobject]@{ file = $name; status = "RESTORE FAILED: $_" } }
    }
    $bad = @($rows | Where-Object { $_.status -like 'MISMATCH*' -or $_.status -like 'RESTORE FAILED*' -or $_.status -like 'BACKUP CHANGED*' })
    $summary = if ($bad.Count) { "NOT IDENTICAL to $Backup ($($bad.Count) problems)" }
    elseif ($notes.Count) { "NOT IDENTICAL to ${Backup}: $($notes -join '; ')" }
    else { "identical to $Backup after $($rows.Count) restores" }
    [pscustomobject]@{ rows = $rows; summary = $summary; changed = $changed; clean = ($bad.Count -eq 0 -and $notes.Count -eq 0) }
}

# The record that outlives this process: each root's live folder and its snapshot state, in the backup folder. Then this run's pending record
# under %LOCALAPPDATA%\OpenWillow (pending.ps1), with every root's tree hash: from here on a kill leaves something the next run refuses on.
function Save-VerifiedBackupRecord([string]$Backup, $Roots, $Snapshots) {
    $record = [ordered]@{}
    foreach ($name in $Roots.Keys) { $record[$name] = [ordered]@{ live = $Roots[$name]; snapshot = $Snapshots[$name] } }
    $record | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $Backup 'verified_hashes.json') -Encoding UTF8
    Write-PendingRecord $Backup $Roots | Out-Null
}
# Restore by hand from a backup no pending record names. While any pending record exists it refuses: Restore-PendingRecord is the way then.
# Like it, it holds the run lock, copies the current roots aside (verified by tree hash) first, and checks for a running game before and after.
function Restore-VerifiedBackup([string]$Backup) {
    Assert-PendingTestRoots @($Backup)
    $records = @(Get-PendingFiles)
    if ($records.Count) { $name = $records[0].FullName -replace '\.tmp$', ''; throw "a pending record exists ($($records[0].FullName)): use Restore-PendingRecord '$name' instead. Nothing was changed." }
    Invoke-RecoveryLocked {
        Assert-NoGameForRecovery 'before anything is copied'
        $aside = Save-PreRestoreSnapshot $Backup
        "the current roots were copied, verified by tree hash, to $aside"
        Assert-NoGameForRecovery 'before restoring'
        $restored = Restore-FromVerifiedRecord $Backup
        Format-RestoreRows $restored.rows
        $restored.summary
        Assert-NoGameAfterRestore $aside
    }
}
# Restore-VerifiedRoots over the roots and snapshot states the backup's verified_hashes.json names. Returns its result object.
# -Record (Restore-PendingRecord passes the pending record): the manifest is not trusted alone. A root is refused (row 'REFUSED', not restored,
# result not clean) unless the record names it with the same live folder and its manifest snapshot gives the record's tree hash for that root
# (Get-HashesDigest, the scheme of Get-TreeDigest). A root the record names and the manifest does not is refused the same way.
function Restore-FromVerifiedRecord([string]$Backup, $Record = $null) {
    $file = Join-Path $Backup 'verified_hashes.json'
    if (!(Test-Path -LiteralPath $file)) { throw "no verified_hashes.json in $Backup; not restoring from it" }
    $roots = [ordered]@{}; $snapshots = @{}
    foreach ($root in (Get-Content -Raw -LiteralPath $file | ConvertFrom-Json).PSObject.Properties) {
        $roots[$root.Name] = $root.Value.live
        $state = $root.Value.snapshot
        if ($state -is [string]) { $snapshots[$root.Name] = $state }
        elseif ($null -ne $state) {
            $hashes = [hashtable]::new([StringComparer]::Ordinal)
            foreach ($f in $state.PSObject.Properties) { $hashes[$f.Name] = $f.Value }
            $snapshots[$root.Name] = $hashes
        }
    }
    if ($null -eq $Record) { return (Restore-VerifiedRoots $Backup $roots $snapshots) }
    $refused = @(); $recorded = @{}
    if ($Record.roots) { foreach ($r in $Record.roots.PSObject.Properties) { $recorded[$r.Name] = $r.Value } }
    $full = { param($Path) if ($Path) { [IO.Path]::GetFullPath([string]$Path).TrimEnd('\') } else { '' } }
    foreach ($name in @($roots.Keys)) {
        $mine = $recorded[$name]; $state = $snapshots[$name]; $why = $null
        if ($null -eq $mine -or !@($recorded.Keys | Where-Object { $_ -ceq $name }).Count) { $why = 'the pending record does not name this root' }
        elseif ((& $full $roots[$name]) -cne (& $full $mine.live)) { $why = "the manifest's live folder '$($roots[$name])' is not the record's '$($mine.live)'" }
        else {
            $got = if ($state -is [hashtable]) { Get-HashesDigest $state } elseif ($state -ceq 'absent') { 'absent' } else { $null }   # else: never restored
            if ($null -ne $got -and $got -cne [string]$mine.tree_sha256) { $why = "the manifest snapshot's tree hash $got is not the record's $($mine.tree_sha256)" }
        }
        if ($why) { $refused += [pscustomobject]@{ file = $name; status = "REFUSED: $why; not restored" }; $roots.Remove($name) }
    }
    foreach ($name in $recorded.Keys) {
        if (!@($roots.Keys | Where-Object { $_ -ceq $name }).Count -and !@($refused | Where-Object { $_.file -ceq $name }).Count) {
            $refused += [pscustomobject]@{ file = $name; status = 'REFUSED: the pending record names this root and the manifest does not; not restored' }
        }
    }
    $result = Restore-VerifiedRoots $Backup $roots $snapshots
    if ($refused.Count) {
        $result.rows = @($refused) + @($result.rows)
        $result.summary = "NOT IDENTICAL to $Backup ($($refused.Count) roots refused: the manifest does not match the pending record)"
        $result.clean = $false
    }
    $result | Add-Member -NotePropertyName refused -NotePropertyValue @($refused) -Force
    $result
}

# The cleanup of the shorter captures (Backup-Saves, one game at a time): call it after the game is stopped and the driver removed, before the
# locks are released. With no game running, every root goes back to the verified backup and is compared again; the comparison goes to
# $CompareFile; this run's pending record goes only when that is clean. Returns clean (false: keep both locks) and the lines to print, which
# hold the exact recovery steps when it is not clean.
function Complete-BackupRun([string]$Backup, [string]$CompareFile) {
    $lines = @(); $clean = $false; $safe = $true
    try {
        $games = @(Get-GameProcesses)
        if ($games.Count) { $safe = $false; $lines += "saves: NOT RESTORED: Borderlands 2 is running (pid $(@($games | ForEach-Object { $_.Id }) -join ', ')) and could overwrite them; backup kept at $Backup" }
        else {
            $restored = Restore-FromVerifiedRecord $Backup
            if ($CompareFile) { @{ backup = $Backup; compared = (Get-Date).ToString('o'); changes = @($restored.rows) } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $CompareFile }
            $lines += Format-RestoreRows $restored.rows
            $lines += "saves: $($restored.summary)"
            $clean = $restored.clean
            # A game started during the restore could have written what the restore then overwrote: kept for a person, not completed.
            $after = @(Get-GameProcesses)
            if ($after.Count) { $safe = $false; $clean = $false; $lines += "saves: A GAME STARTED DURING THE RESTORE (pid $(@($after | ForEach-Object { $_.Id }) -join ', ')); the record is kept" }
        }
    } catch { $lines += "saves: RESTORE FAILED: $_ (backup kept at $Backup)" }
    $pendingFile = $script:PendingRecord
    $pending = Complete-PendingRecord $safe
    $lines += "pending record: $pending"
    if ($pending -notlike 'removed*' -and $pending -ne 'none written') { $clean = $false }
    if (!$clean) {
        $record = if ($pendingFile -and (Test-Path -LiteralPath $pendingFile)) { Get-Content -Raw -LiteralPath $pendingFile | ConvertFrom-Json } else { $null }
        if ($record) { $lines += Format-PendingRecovery $pendingFile $record @(Get-PendingProblems $record) }
        else { $lines += "MANUAL RECOVERY: close Borderlands 2, then: . '$PSScriptRoot\realgame.ps1'; Restore-VerifiedBackup '$Backup'" }
        $lines += 'Both locks are kept until that is done.'
    }
    [pscustomobject]@{ clean = $clean; lines = $lines }
}

# The restore rows as text. Plain objects go through a deferred formatter: when a script ends on a rethrown error, they and every
# string after them never reach the screen, and those can be the recovery steps. Text prints in order either way.
function Format-RestoreRows($Rows) { if (@($Rows).Count) { @($Rows) | Format-Table -AutoSize | Out-String -Width 200 } }

# Why a finalizer must keep its locks: every reason a run leaves something for a person. Empty when nothing is left. Any reason holds both
# locks: an unconfirmed or untracked game can overwrite restored files, a restore that is not clean needs recovery, and a leftover driver
# keeps its save-block flag and command channel in the game's folder for the next launch.
function Get-RecoveryReason([bool]$GameGone, [string]$GameState, $Restored, [string]$RestoreError, [string]$DriverState, [string]$UntrackedGames = '') {
    $reasons = @()
    if (!$GameGone) { $reasons += "game exit not confirmed ($GameState)" }
    if ($UntrackedGames) { $reasons += "a game this run did not start is running (pid $UntrackedGames), and it can write the saves" }
    if ($RestoreError) { $reasons += "restore failed ($RestoreError)" }
    elseif ($Restored -and !$Restored.clean) { $reasons += "restore did not compare clean ($($Restored.summary))" }
    if ($GameGone -and $DriverState -match '^(STILL PRESENT|error)') { $reasons += "driver not removed ($DriverState)" }
    $reasons -join '; '
}
# The one recovery text: what happened, then only the steps this run needs. Restore-VerifiedBackup refuses a backup that changed.
function Format-ManualRecovery([string]$Reason, [string]$Backup, [string]$HubLock, [string]$Lane) {
    $lines = @("MANUAL RECOVERY: $Reason. Both locks are held and the backup is kept at $Backup.",
        '  1. Make sure Borderlands 2 is closed.',
        "  2. In a new PowerShell window: `$env:OPENWILLOW_BL2 = '$env:OPENWILLOW_BL2'; . '$PSScriptRoot\realgame.ps1'")
    if ($script:PendingRecord -and (Test-Path -LiteralPath $script:PendingRecord)) {
        # The only step while a record exists: it sets the current state aside first, removes only this run's driver copy, restores and verifies.
        $lines += "     Restore-PendingRecord '$script:PendingRecord'"
        $lines += "     It copies the current saves and settings aside (verified by tree hash), removes the driver folder only if its owner file is this run's, restores every root from the verified backup and removes the record only when all compare clean. Anything it reports instead is yours to judge."
    } elseif ($Backup) {
        if (Test-Path -LiteralPath $script:DriverDir) { $lines += "     Remove-Item -Recurse -Force '$script:DriverDir'   # first check it is this driver's copy, not someone's" }
        $lines += "     Restore-VerifiedBackup '$Backup'   # copies the current state aside first"
        $lines += "     Every row must read 'added, removed', 'missing, restored' or 'changed, restored'. A 'MISMATCH after restore', 'RESTORE FAILED' or 'BACKUP CHANGED' row means that root is not recovered; a changed backup is refused, so compare it with the live files by hand. A root reported as created or not backed up is yours to judge."
    }
    $lines += "  3. Only when nothing above reports a problem, remove the locks this run left: '$HubLock' (if it names '$Lane') and '$script:Lock'."
    $lines -join [Environment]::NewLine
}

# block_saves() answers the number of save functions it hooked. Only a positive whole number means it ran: anything else, a traceback that
# happens to contain a word included, stops the run before any gameplay.
function Assert-SavesBlocked($Reply) {
    $text = "$Reply".Trim()
    if ($text -notmatch '^[1-9][0-9]*$') { throw "saves are not blocked: block_saves() answered '$text'" }
    [int]$text
}

# A capture counts only when it holds frames and every named shot was saved.
function Test-CaptureComplete($Capture, [hashtable]$Shots = @{}) {
    if (!$Capture -or $Capture.frames -le 0) { return $false }
    foreach ($name in $Shots.Keys) { if (!$Capture.shots.ContainsKey($name)) { return $false } }
    $true
}
# 'completed' only when nothing needs recovery, no error stopped the run and every requested stage passed.
function Get-RunStatus([string[]]$Requested, [string[]]$Completed, [string]$RunError, [string]$Recovery = '') {
    if ($Recovery) { return "recovery needed: $Recovery" }
    if ($RunError) { return "failed: $RunError" }
    $missing = @($Requested | Where-Object { $_ -notin $Completed })
    if ($missing.Count) { "incomplete: $($missing -join ', ')" } else { 'completed' }
}

# Stop the game process this run started, only if the PID still names that process: same image path, same start time and
# a command line containing $Marker. Returns what it did ('stopped', 'exited', or why it refused).
function Stop-OwnGame([Diagnostics.Process]$Started, [datetime]$StartTime, [string]$Marker) {
    $exe = [IO.Path]::GetFullPath((Join-Path $script:Game 'Binaries/Win32/Borderlands2.exe'))
    $c = Get-CimInstance Win32_Process -Filter "ProcessId=$($Started.Id)" -ErrorAction SilentlyContinue
    if (!$c) { return 'exited' }
    if (!$c.ExecutablePath -or [IO.Path]::GetFullPath($c.ExecutablePath) -ne $exe) { return "refused: pid $($Started.Id) is $($c.ExecutablePath)" }
    if ([math]::Abs(($c.CreationDate - $StartTime).TotalSeconds) -gt 2) { return "refused: pid $($Started.Id) started $($c.CreationDate), ours $StartTime" }
    if (!$c.CommandLine -or !$c.CommandLine.Contains($Marker)) { return "refused: pid $($Started.Id) command line lacks '$Marker'" }
    Stop-Process -Id $Started.Id -Force; 'stopped'
}

function Start-Game([string[]]$Arguments = @()) {
    $exe = Join-Path $script:Game 'Binaries/Win32/Borderlands2.exe'
    Add-PendingChange "game launched: $exe $($Arguments -join ' ')"   # a game can write the saves and profile; recorded before it starts
    $script:OwnGame = Start-Process -FilePath $exe -ArgumentList $Arguments -WorkingDirectory (Split-Path $exe) -PassThru
    $script:OwnGame
}
function Get-GameProcess { Get-Process Borderlands2 -ErrorAction SilentlyContinue | Select-Object -First 1 }

# Every running game, and those that are not the one this run started. Reading only: nothing here stops a process.
function Get-GameProcesses { @(Get-Process Borderlands2 -ErrorAction SilentlyContinue) }
# Is this live process the game this run started? Id, start time and image path, as Stop-OwnGame checks. Once ours has exited, nothing is:
# Windows can give its id to another process.
function Test-IsRunGame($Process, $Own) {
    if (!$Own -or $Own.HasExited -or $Process.Id -ne $Own.Id -or !$Process.Path) { return $false }
    if ([IO.Path]::GetFullPath($Process.Path) -ne [IO.Path]::GetFullPath((Join-Path $script:Game 'Binaries/Win32/Borderlands2.exe'))) { return $false }
    [math]::Abs(($Process.StartTime - $Own.StartTime).TotalSeconds) -le 2
}
function Get-UntrackedGames($Own) { @(Get-GameProcesses | Where-Object { !(Test-IsRunGame $_ $Own) }) }
# Every game seen so far that this run did not start, as ids joined by commas. It stays remembered once that game has gone, because it may have
# saved, and restoring would roll that progress back. The limit: only these checks see a game, so one started and closed between them goes
# unnoticed. The rule that covers that gap is that nobody launches the game while a capture runs.
$script:ForeignGamesSeen = @()
function Get-ForeignGamesSeen($Own) {
    $script:ForeignGamesSeen = @(@($script:ForeignGamesSeen) + @(Get-UntrackedGames $Own | ForEach-Object { $_.Id }) | Select-Object -Unique)
    $script:ForeignGamesSeen -join ', '
}
# The game this run started, while it lives. Every script that sends input starts its own game (Start-Game), so nothing attaches to one by name.
function Get-RunGameProcess {
    if (!$script:OwnGame) { return $null }
    $script:OwnGame.Refresh()
    if ($script:OwnGame.HasExited) { return $null }
    $script:OwnGame
}
# Its Alt tap can activate the foreground app's menu when it is retried; Assert-GameForeground's three bounded tries accept that.
function Focus-Game {
    $p = Get-RunGameProcess; if (!$p) { throw 'game not running' }
    $h = $p.MainWindowHandle
    # A harmless Alt tap lets SetForegroundWindow succeed from a background process.
    try { Invoke-NativeKey 0x12 0x38 0 } finally { Invoke-NativeKey 0x12 0x38 2 }
    [OWRG.Native]::ShowWindow($h, 9) | Out-Null
    [OWRG.Native]::SetForegroundWindow($h) | Out-Null
    Start-Sleep -Milliseconds 200
    return ([OWRG.Native]::GetForegroundWindow() -eq $h)
}

function Get-ForegroundWindowHandle { [OWRG.Native]::GetForegroundWindow() }
# Input goes to whatever has focus, on a PC someone may be using, so every key press and mouse action asks this first: the game must be the
# foreground window, after up to three focus attempts, or the step fails.
function Assert-GameForeground {
    for ($attempt = 0; $attempt -le 3; $attempt++) {
        $p = Get-RunGameProcess
        if (!$p) { throw 'refusing to send input: the game is not running' }
        if ((Get-ForegroundWindowHandle) -eq $p.MainWindowHandle) { return }
        if ($attempt -lt 3) { Focus-Game | Out-Null }
    }
    throw 'refusing to send input: Borderlands 2 is not the foreground window after 3 focus attempts (is someone using this PC?)'
}

function Get-GameRect {
    $h = (Get-GameProcess).MainWindowHandle
    $r = New-Object OWRG.Native+RECT; [OWRG.Native]::GetClientRect($h, [ref]$r) | Out-Null
    $pt = New-Object OWRG.Native+POINT; [OWRG.Native]::ClientToScreen($h, [ref]$pt) | Out-Null
    [Drawing.Rectangle]::new($pt.X, $pt.Y, $r.Right - $r.Left, $r.Bottom - $r.Top)
}

function Save-Shot([string]$Name, [Drawing.Rectangle]$Rect = (Get-GameRect)) {
    $bmp = [Drawing.Bitmap]::new($Rect.Width, $Rect.Height)
    $g = [Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($Rect.Location, [Drawing.Point]::Empty, $Rect.Size); $g.Dispose()
    $path = Join-Path $script:RG $Name
    New-Item -ItemType Directory -Force (Split-Path $path) | Out-Null
    $bmp.Save($path, [Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose(); $path
}

# Scan-code key input (DirectInput-style games ignore virtual-key-only events).
$script:Scan = @{ Esc=0x01; '1'=0x02; '2'=0x03; '3'=0x04; '4'=0x05; Tab=0x0F; Q=0x10; W=0x11; E=0x12; R=0x13; I=0x17;
    A=0x1E; S=0x1F; D=0x20; F=0x21; G=0x22; Enter=0x1C; Space=0x39; LShift=0x2A; LCtrl=0x1D; M=0x32; L=0x26; K=0x25; Tilde=0x29; F6=0x40;
    C=0x2E; N=0x31; B=0x30; V=0x2F; X=0x2D; Z=0x2C; T=0x14; Y=0x15; U=0x16; O=0x18; P=0x19; H=0x23; J=0x24; '5'=0x06 }
function Assert-Key([string]$Key) { if (-not ($script:Scan.ContainsKey($Key) -or $script:ScanExt.ContainsKey($Key))) { throw "no scan code for key '$Key'" } }
# Arrow keys are extended scan codes (KEYEVENTF_EXTENDEDKEY).
$script:ScanExt = @{ Up=0x48; Down=0x50; Left=0x4B; Right=0x4D }
# Guarded input. Every press is remembered until its release, and Release-HeldInputs lets go of whatever is still held: every capture calls it
# first in its finally, so an error, a timeout or Ctrl+C (PowerShell runs finally blocks on a stop) leaves no key or button down. The native calls
# go through Invoke-NativeKey and Invoke-NativeMouse only, so a test can replace them.
$script:HeldInputs = [Collections.Generic.List[string]]::new()
function Invoke-NativeKey([byte]$Vk, [byte]$Scan, [uint32]$Flags) { [OWRG.Native]::keybd_event($Vk, $Scan, $Flags, [UIntPtr]::Zero) }
function Invoke-NativeMouse([uint32]$Flags, [int]$Dx = 0, [int]$Dy = 0, [uint32]$Data = 0) { [OWRG.Native]::mouse_event($Flags, $Dx, $Dy, $Data, [UIntPtr]::Zero) }
# Each mouse button's down flag, up flag and the name it is held under.
$script:MouseButtons = @(@(0x2, 0x4, 'left'), @(0x8, 0x10, 'right'), @(0x20, 0x40, 'middle'))
# The held set is also written into this run's pending record before each press (Save-HeldInputs), so a run killed with a key down leaves the
# key named: the next run or the recovery sends its release while the game window is there. No record, no press.
function Save-HeldInputs([switch]$Release) {
    if ($Release) { try { if ($script:PendingRecord -and (Test-Path -LiteralPath $script:PendingRecord)) { Set-PendingField 'held_inputs' @($script:HeldInputs) } } catch { Write-Warning "held inputs not recorded: $_" }; return }
    Set-PendingField 'held_inputs' @($script:HeldInputs)
}
function Send-KeyDown([string]$Key) {
    Assert-Key $Key
    Assert-GameForeground
    if (!$script:HeldInputs.Contains("key $Key")) { $script:HeldInputs.Add("key $Key") }   # before the send: releasing a key never pressed is harmless
    try { Save-HeldInputs } catch { [void]$script:HeldInputs.Remove("key $Key"); throw "refusing to press ${Key}: $_" }
    if ($script:ScanExt.ContainsKey($Key)) { Invoke-NativeKey 0 ([byte]$script:ScanExt[$Key]) 0x9 }
    else { Invoke-NativeKey 0 ([byte]$script:Scan[$Key]) 0x8 }
}
# A release is never refused: it lets go of what a press took. It goes to whatever has focus then, which is not guaranteed harmless but is
# better than a key left held.
function Send-KeyUp([string]$Key) {
    Assert-Key $Key
    if ($script:ScanExt.ContainsKey($Key)) { Invoke-NativeKey 0 ([byte]$script:ScanExt[$Key]) 0xB }
    else { Invoke-NativeKey 0 ([byte]$script:Scan[$Key]) 0xA }
    if ($script:HeldInputs.Remove("key $Key")) { Save-HeldInputs -Release }
}
function Send-Key([string]$Key, [int]$HoldMs = 60) {
    Send-KeyDown $Key
    try { Start-Sleep -Milliseconds $HoldMs } finally { Send-KeyUp $Key }   # a press is always released
}
function Send-MouseEvent([int]$Flags, [int]$Dx = 0, [int]$Dy = 0) {
    $added = @()
    foreach ($b in $script:MouseButtons) { if (($Flags -band $b[0]) -and !$script:HeldInputs.Contains("button $($b[2])")) { $script:HeldInputs.Add("button $($b[2])"); $added += "button $($b[2])" } }
    if ($added.Count) { try { Save-HeldInputs } catch { foreach ($a in $added) { [void]$script:HeldInputs.Remove($a) }; throw "refusing to press a mouse button: $_" } }
    Invoke-NativeMouse $Flags $Dx $Dy
    $released = $false
    foreach ($b in $script:MouseButtons) { if ($Flags -band $b[1]) { if ($script:HeldInputs.Remove("button $($b[2])")) { $released = $true } } }
    if ($released) { Save-HeldInputs -Release }
}
# Let go of every key and button this run still holds. Never throws: each release is tried. Returns what it did, for the run's log.
function Release-HeldInputs {
    $held = @($script:HeldInputs)
    if (!$held.Count) { return 'none held' }
    $failed = @()
    foreach ($item in $held) {
        try {
            $kind, $name = $item -split ' ', 2
            if ($kind -eq 'key') { Send-KeyUp $name }
            else { Send-MouseEvent (@($script:MouseButtons | Where-Object { $_[2] -eq $name })[0][1]) }
        } catch { $failed += "$item ($_)" }
    }
    if ($failed.Count) { "released $($held -join ', '); FAILED: $($failed -join ', ')" } else { "released $($held -join ', ')" }
}
# Move the cursor to a point given in game client pixels and left-click (menus).
function Send-ClickAt([int]$X, [int]$Y) {
    Assert-GameForeground
    $r = Get-GameRect; [OWRG.Native]::SetCursorPos($r.X + $X, $r.Y + $Y) | Out-Null; Start-Sleep -Milliseconds 150; Send-Click
}
function Send-Wheel([int]$Notches) {
    Assert-GameForeground
    $data = [BitConverter]::ToUInt32([BitConverter]::GetBytes([int32](120 * $Notches)), 0)  # negative = toward the user
    Invoke-NativeMouse 0x800 0 0 $data
}
# Hold the left button and move in steps (inspect-view rotation).
function Send-Drag([int]$Dx, [int]$Dy, [int]$Steps = 20) {
    Assert-GameForeground
    Send-MouseEvent 0x2
    try {
        Start-Sleep -Milliseconds 50
        for ($i = 0; $i -lt $Steps; $i++) { Assert-GameForeground; Send-MouseEvent 0x1 ([int]($Dx / $Steps)) ([int]($Dy / $Steps)); Start-Sleep -Milliseconds 15 }
    } finally { Send-MouseEvent 0x4 }   # the button is always let go
}
function Send-Mouse([int]$Dx, [int]$Dy) { Assert-GameForeground; Invoke-NativeMouse 0x1 $Dx $Dy }
function Send-Click([switch]$Right) {
    Assert-GameForeground
    $down, $up = if ($Right) { 0x8, 0x10 } else { 0x2, 0x4 }
    Send-MouseEvent $down
    try { Start-Sleep -Milliseconds 50 } finally { Send-MouseEvent $up }   # a press is always released
}

# Run Python inside the game through the openwillow_realgame mod and return its printed output.
function Invoke-GamePy([string]$Code, [int]$TimeoutSeconds = 30) {
    # The time first: the driver runs pending commands in name order. The GUID makes the name this call's own, and any reply already under it is removed before
    # the command is published, so an old reply can't pass as the answer to this one.
    $name = 'c' + (Get-Date -Format 'HHmmssfff') + '-' + (New-Guid).ToString('N')
    $py = Join-Path $script:CmdDir "$name.py"; $out = Join-Path $script:CmdDir "$name.out"
    if (Test-Path -LiteralPath $out) { Remove-Item -LiteralPath $out -Force -ErrorAction Stop }
    [IO.File]::WriteAllText("$py.tmp", $Code); Move-Item "$py.tmp" $py
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while (!(Test-Path $out)) { if ((Get-Date) -gt $deadline) { throw "no reply to $name" }; Start-Sleep -Milliseconds 100 }
    Start-Sleep -Milliseconds 50
    Get-Content -Raw $out
}

# Run a command script file (e.g. tools/real_game/scripts/*.py) after a short prelude; RG_OUT is set
# to local/realgame/<OutFolder> so the script's helpers know where to write.
function Invoke-GamePyFile([string]$Path, [string]$OutFolder = 'out', [int]$TimeoutSeconds = 30) {
    $out = (Join-Path $script:RG $OutFolder) -replace '\\', '/'
    Invoke-GamePy ("RG_OUT = r'$out'`n" + [IO.File]::ReadAllText((Resolve-Path $Path))) $TimeoutSeconds
}

# After the Enter on the main menu that continues the save: wait for the player pawn, reads only (the map name and whether a pawn is
# there), no key or click. The first read comes 45 s after the Enter, as every capture's fixed wait did, so a normal load is read at the
# same moment as before. While the game is still on the menu map without a pawn (where it shows "Creating online session / Please
# wait..."; usually a few seconds, but two launches on 2026-11-12 never left it; the timings are in the private dossier), it reads
# again every 5 s up to $MenuSeconds after the Enter. Once the menu is left, the load
# gets $LoadSeconds more; replies that do not come (the load holds the command channel) are waited out for $LoadSeconds in a row at most.
# Returns ok, the last read (map), why it failed (naming the wait that ran out) and the seconds from the Enter.
function Wait-PlayerPawn([datetime]$EnterAt, $Process, [int]$MenuSeconds = 240, [int]$LoadSeconds = 240) {
    $read = 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)'
    $first = 45 - ((Get-Date) - $EnterAt).TotalSeconds
    if ($first -gt 0) { Start-Sleep -Milliseconds ([int]($first * 1000)) }
    $leftMenu = $null; $last = ''; $lastReply = Get-Date
    $result = { param($Ok, $Why) [pscustomobject]@{ ok = $Ok; map = $last; why = $Why; after_enter_s = [math]::Round(((Get-Date) - $EnterAt).TotalSeconds, 1) } }
    while ($true) {
        if ($Process -and $Process.HasExited) { return (& $result $false "the game exited before a player pawn (last read: $last)") }
        $replied = $false
        try { $last = "$(Invoke-GamePy $read 30)".Trim(); $lastReply = Get-Date; $replied = $true }
        catch { $last = "$_" }
        $now = Get-Date; $since = [math]::Round(($now - $EnterAt).TotalSeconds)
        if ($replied -and $last -match 'WillowPlayerPawn' -and $last -notmatch 'Traceback') { return (& $result $true '') }
        if ($replied -and $last -match '^menumap\b') {
            if ($since -ge $MenuSeconds) { return (& $result $false "still on the main menu $since s after the Enter: the wait for the game's online session (shown on the menu as 'Creating online session') ran out at $MenuSeconds s; last read: $last") }
        } elseif ($replied -and $null -eq $leftMenu) { $leftMenu = $now }   # another map, or a read that failed in the game: the load is under way
        $noReply = [math]::Round(($now - $lastReply).TotalSeconds)
        if ($noReply -ge $LoadSeconds) { return (& $result $false "the driver did not answer for $noReply s while the save loaded (limit $LoadSeconds s): $last") }
        if ($leftMenu -and ($now - $leftMenu).TotalSeconds -ge $LoadSeconds) { return (& $result $false "the menu was left, but no player pawn $([math]::Round(($now - $leftMenu).TotalSeconds)) s later (limit $LoadSeconds s); last read: $last") }
        Start-Sleep 5
    }
}

# Rapid capture: grab frames into memory for $Seconds, then write them with QPC timestamps.
function Invoke-Burst([string]$Folder, [double]$Seconds = 3, [int]$IntervalMs = 50, [scriptblock]$Trigger, [double]$Scale = 0.5) {
    $rect = Get-GameRect; $frames = New-Object Collections.Generic.List[object]
    $w = [int]($rect.Width * $Scale); $hgt = [int]($rect.Height * $Scale)
    $t0 = Get-QpcSeconds; $triggerAt = $null
    while (((Get-QpcSeconds) - $t0) -lt $Seconds) {
        $ts = Get-QpcSeconds
        $bmp = [Drawing.Bitmap]::new($rect.Width, $rect.Height)
        $g = [Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($rect.Location, [Drawing.Point]::Empty, $rect.Size); $g.Dispose()
        $small = [Drawing.Bitmap]::new($bmp, $w, $hgt); $bmp.Dispose()
        $frames.Add([pscustomobject]@{ t = $ts; bmp = $small })
        if ($Trigger -and $null -eq $triggerAt -and $frames.Count -ge 3) { $triggerAt = Get-QpcSeconds; & $Trigger }
        $next = $ts + $IntervalMs / 1000.0
        while ((Get-QpcSeconds) -lt $next) { Start-Sleep -Milliseconds 1 }
    }
    $dir = Join-Path $script:RG $Folder; New-Item -ItemType Directory -Force $dir | Out-Null
    $index = for ($i = 0; $i -lt $frames.Count; $i++) {
        $f = $frames[$i]; $file = ('f{0:D3}.jpg' -f $i)
        $f.bmp.Save((Join-Path $dir $file), [Drawing.Imaging.ImageFormat]::Jpeg); $f.bmp.Dispose()
        [pscustomobject]@{ i = $i; file = $file; t = $f.t; rel = [math]::Round($f.t - $triggerAt, 4) }
    }
    $index | ConvertTo-Json | Set-Content (Join-Path $dir 'frames.json')
    [pscustomobject]@{ dir = $dir; trigger = $triggerAt; frames = $frames.Count }
}
