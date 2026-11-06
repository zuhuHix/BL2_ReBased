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

function Backup-Saves {
    $src = $script:WillowDir
    $dst = Join-Path $script:RG ('save-backup-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
    New-Item -ItemType Directory -Force $dst | Out-Null
    Copy-Item -Recurse (Join-Path $src 'SaveData') $dst
    Copy-Item -Recurse (Join-Path $src 'Config') $dst
    $dst
}

function Install-Driver([switch]$BlockSavesAtStart, [switch]$Exclusive) {
    # Copy the command-channel mod into the game's sdk_mods (removed again by Remove-Driver).
    # -BlockSavesAtStart: the driver blocks the save functions as soon as it loads (see the driver's autoblock.flag).
    # -Exclusive: refuse when bl-sdk is absent or the mod folder already exists (then this run did not create it and must
    # neither overwrite nor remove it); otherwise the old behaviour (create or overwrite).
    $target = $script:DriverDir
    if ($Exclusive) {
        if (!(Test-Path -LiteralPath (Join-Path $script:Game 'sdk_mods'))) { throw "bl-sdk is not installed: $(Join-Path $script:Game 'sdk_mods') is missing" }
        if (Test-Path -LiteralPath $target) { throw "$target already exists; not overwriting it (remove it by hand if it is a stale copy of this driver)" }
    }
    # A folder this call creates is removed again if the install fails part-way (a partial driver would make the next
    # -Exclusive run refuse); a folder that already existed is left as it is.
    $created = !(Test-Path -LiteralPath $target)
    try {
        New-Item -ItemType Directory -Force $target | Out-Null
        Copy-Item (Join-Path $PSScriptRoot 'openwillow_realgame/__init__.py') $target -Force
        Set-Content -LiteralPath (Join-Path $target 'cmd_dir.txt') -Value $script:CmdDir -NoNewline
        $flag = Join-Path $target 'autoblock.flag'
        if ($BlockSavesAtStart) { Set-Content -LiteralPath $flag -Value '1' -NoNewline } else { Remove-Item $flag -ErrorAction SilentlyContinue }
    } catch {
        if ($created) { Remove-Item -LiteralPath $target -Recurse -Force -ErrorAction SilentlyContinue }
        throw
    }
}
function Remove-Driver { Remove-Item -Recurse -Force (Join-Path $script:Game 'sdk_mods/openwillow_realgame') -ErrorAction SilentlyContinue }

# Make the live tree $Live identical to the backup tree $Backup: files added during the run are removed, missing and changed
# files are copied back, directories created during the run are removed when empty. Then every file is compared again by
# SHA-256. Returns one row per difference found; a row with status 'MISMATCH after restore' means the restore failed.
function Restore-Tree([string]$Backup, [string]$Live, [string]$Label) {
    $Backup = [IO.Path]::GetFullPath($Backup).TrimEnd('\'); $Live = [IO.Path]::GetFullPath($Live).TrimEnd('\')
    $rel = { param($Root, $Path) $Path.Substring($Root.Length + 1) }
    $row = { param($File, $Status) [pscustomobject]@{ file = "$Label\$File"; status = $Status } }
    if (!(Test-Path -LiteralPath $Live)) { New-Item -ItemType Directory -Force $Live | Out-Null }
    foreach ($f in @(Get-ChildItem -LiteralPath $Live -Recurse -File -Force)) {
        $r = & $rel $Live $f.FullName
        if (!(Test-Path -LiteralPath (Join-Path $Backup $r) -PathType Leaf)) { Remove-Item -LiteralPath $f.FullName -Force; & $row $r 'added, removed' }
    }
    foreach ($f in @(Get-ChildItem -LiteralPath $Backup -Recurse -File -Force)) {
        $r = & $rel $Backup $f.FullName; $dst = Join-Path $Live $r
        if (!(Test-Path -LiteralPath $dst -PathType Leaf)) {
            New-Item -ItemType Directory -Force (Split-Path $dst) | Out-Null; Copy-Item -LiteralPath $f.FullName $dst -Force; & $row $r 'missing, restored'
        } elseif ((Get-FileHash -LiteralPath $f.FullName).Hash -ne (Get-FileHash -LiteralPath $dst).Hash) {
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
# Relative path to SHA-256 for every file under $Root; empty when the folder is gone.
function Get-TreeHashes([string]$Root) {
    $Root = [IO.Path]::GetFullPath($Root).TrimEnd('\'); $h = @{}
    if (Test-Path -LiteralPath $Root) {
        Get-ChildItem -LiteralPath $Root -Recurse -File -Force | ForEach-Object { $h[$_.FullName.Substring($Root.Length + 1)] = (Get-FileHash -LiteralPath $_.FullName).Hash }
    }
    $h
}
function Compare-Hashes([hashtable]$A, [hashtable]$B) {
    @($A.Keys) + @($B.Keys) | Sort-Object -Unique | Where-Object { $A[$_] -ne $B[$_] }
}

# Copy $Live to the new folder $Snapshot and prove the copy matches, or throw. Returns the snapshot's hashes: store them for
# Restore-VerifiedRoots, because only a root that passed may be restored from. Restore-Tree deletes live files absent from its backup,
# so a partial snapshot, or one that changed later, would lose them.
function New-VerifiedSnapshot([string]$Live, [string]$Snapshot) {
    Copy-Item -Recurse -Force -LiteralPath $Live $Snapshot
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
    $rows = @(); $notes = @(); $changed = @()
    foreach ($name in $Roots.Keys) {
        $live = $Roots[$name]
        try {
            if ($Snapshots[$name] -is [hashtable]) {
                $b = Join-Path $Backup $name
                $diff = @(Compare-Hashes $Snapshots[$name] (Get-TreeHashes $b))
                if ($diff.Count) {
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

# The record that outlives this process: each root's live folder and its snapshot state, in the backup folder.
function Save-VerifiedBackupRecord([string]$Backup, $Roots, $Snapshots) {
    $record = [ordered]@{}
    foreach ($name in $Roots.Keys) { $record[$name] = [ordered]@{ live = $Roots[$name]; snapshot = $Snapshots[$name] } }
    $record | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $Backup 'verified_hashes.json') -Encoding UTF8
}
# The one way to restore by hand: from that record, through Restore-VerifiedRoots, so a backup that changed since is refused.
function Restore-VerifiedBackup([string]$Backup) {
    $file = Join-Path $Backup 'verified_hashes.json'
    if (!(Test-Path -LiteralPath $file)) { throw "no verified_hashes.json in $Backup; not restoring from it" }
    $roots = [ordered]@{}; $snapshots = @{}
    foreach ($root in (Get-Content -Raw -LiteralPath $file | ConvertFrom-Json).PSObject.Properties) {
        $roots[$root.Name] = $root.Value.live
        $state = $root.Value.snapshot
        if ($state -is [string]) { $snapshots[$root.Name] = $state }
        elseif ($null -ne $state) {
            $hashes = @{}
            foreach ($f in $state.PSObject.Properties) { $hashes[$f.Name] = $f.Value }
            $snapshots[$root.Name] = $hashes
        }
    }
    $restored = Restore-VerifiedRoots $Backup $roots $snapshots
    Format-RestoreRows $restored.rows
    $restored.summary
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
    if (Test-Path -LiteralPath $script:DriverDir) { $lines += "     Remove-Item -Recurse -Force '$script:DriverDir'   # first check it is this driver's copy, not someone's" }
    if ($Backup) {
        $lines += "     Restore-VerifiedBackup '$Backup'"
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
    if ($c.CommandLine -notlike "*$Marker*") { return "refused: pid $($Started.Id) command line lacks '$Marker'" }
    Stop-Process -Id $Started.Id -Force; 'stopped'
}

function Start-Game([string[]]$Arguments = @()) {
    $exe = Join-Path $script:Game 'Binaries/Win32/Borderlands2.exe'
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
    [OWRG.Native]::keybd_event(0x12, 0x38, 0, [UIntPtr]::Zero); [OWRG.Native]::keybd_event(0x12, 0x38, 2, [UIntPtr]::Zero)
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
function Send-KeyDown([string]$Key) {
    Assert-Key $Key
    Assert-GameForeground
    if ($script:ScanExt.ContainsKey($Key)) { [OWRG.Native]::keybd_event(0, [byte]$script:ScanExt[$Key], 0x9, [UIntPtr]::Zero) }
    else { [OWRG.Native]::keybd_event(0, [byte]$script:Scan[$Key], 0x8, [UIntPtr]::Zero) }
}
# A release is never refused: it lets go of what a press took. It goes to whatever has focus then, which is not guaranteed harmless but is
# better than a key left held.
function Send-KeyUp([string]$Key) {
    Assert-Key $Key
    if ($script:ScanExt.ContainsKey($Key)) { [OWRG.Native]::keybd_event(0, [byte]$script:ScanExt[$Key], 0xB, [UIntPtr]::Zero) }
    else { [OWRG.Native]::keybd_event(0, [byte]$script:Scan[$Key], 0xA, [UIntPtr]::Zero) }
}
function Send-Key([string]$Key, [int]$HoldMs = 60) {
    Send-KeyDown $Key
    try { Start-Sleep -Milliseconds $HoldMs } finally { Send-KeyUp $Key }   # a press is always released
}
function Send-MouseEvent([int]$Flags, [int]$Dx = 0, [int]$Dy = 0) { [OWRG.Native]::mouse_event($Flags, $Dx, $Dy, 0, [UIntPtr]::Zero) }
# Move the cursor to a point given in game client pixels and left-click (menus).
function Send-ClickAt([int]$X, [int]$Y) {
    Assert-GameForeground
    $r = Get-GameRect; [OWRG.Native]::SetCursorPos($r.X + $X, $r.Y + $Y) | Out-Null; Start-Sleep -Milliseconds 150; Send-Click
}
function Send-Wheel([int]$Notches) {
    Assert-GameForeground
    $data = [BitConverter]::ToUInt32([BitConverter]::GetBytes([int32](120 * $Notches)), 0)  # negative = toward the user
    [OWRG.Native]::mouse_event(0x800, 0, 0, $data, [UIntPtr]::Zero)
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
function Send-Mouse([int]$Dx, [int]$Dy) { Assert-GameForeground; [OWRG.Native]::mouse_event(0x1, $Dx, $Dy, 0, [UIntPtr]::Zero) }
function Send-Click([switch]$Right) {
    Assert-GameForeground
    $down, $up = if ($Right) { 0x8, 0x10 } else { 0x2, 0x4 }
    Send-MouseEvent $down
    try { Start-Sleep -Milliseconds 50 } finally { Send-MouseEvent $up }   # a press is always released
}

# Run Python inside the game through the openwillow_realgame mod and return its printed output.
function Invoke-GamePy([string]$Code, [int]$TimeoutSeconds = 30) {
    $name = 'c' + (Get-Date -Format 'HHmmssfff')
    $py = Join-Path $script:CmdDir "$name.py"; $out = Join-Path $script:CmdDir "$name.out"
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
