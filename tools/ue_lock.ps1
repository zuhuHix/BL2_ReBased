# The machine-wide Unreal run lock (dot-source: . (Join-Path $PSScriptRoot 'ue_lock.ps1')).
#
# One Unreal Editor or commandlet (and one real-game session) at a time per machine. Every script that launches
# UnrealEditor*.exe takes this lock before the launch and releases it after the process has exited:
#
#   . (Join-Path $PSScriptRoot 'ue_lock.ps1')
#   Enter-OwUeLock -Owner 'test_quest' -TimeoutSeconds 600
#   try { ...launch, wait for exit... } finally { Exit-OwUeLock }
#
# Path: %LOCALAPPDATA%\OpenWillow\ue_run.lock, or the absolute path in OPENWILLOW_UE_LOCK (a relative override is
# rejected: it would name a different file in every worktree). Until 2026-11-03 the lock was local/ue_run.lock in each
# checkout, so editors from different worktrees did not see each other.
#
# The lock file holds its owner: the PowerShell process id, that process's start time and the script name. A lock is
# stale only when that exact process is gone (no process with the id, or one with a different start time). An unreadable
# lock, or one written by an older script without owner fields, is never treated as stale; remove it by hand after
# checking that no editor or game it guarded is running.

function Get-OwUeLockPath {
    $path = $env:OPENWILLOW_UE_LOCK
    if ([string]::IsNullOrWhiteSpace($path)) {
        $base = $env:LOCALAPPDATA
        if ([string]::IsNullOrWhiteSpace($base)) { $base = [Environment]::GetFolderPath('LocalApplicationData') }
        $path = Join-Path $base 'OpenWillow\ue_run.lock'
    } elseif ($path -notmatch '^([A-Za-z]:[\\/]|[\\/]{2})') {   # drive-absolute or UNC (Windows PowerShell 5.1 has no IsPathFullyQualified)
        throw "OPENWILLOW_UE_LOCK must be an absolute path (got '$path'); a relative path names a different lock in every worktree."
    }
    $path = [IO.Path]::GetFullPath($path)
    [IO.Directory]::CreateDirectory((Split-Path -Parent $path)) | Out-Null
    return $path
}

function Get-OwUeLockOwner([string]$Path = (Get-OwUeLockPath)) {
    # The owner record, or $null when the file is missing, still empty, unreadable or has no owner fields.
    try { $text = [IO.File]::ReadAllText($Path) } catch { return $null }
    try { $owner = $text | ConvertFrom-Json -ErrorAction Stop } catch { return $null }
    if ($null -eq $owner -or $null -eq $owner.pid -or $null -eq $owner.process_start_utc_filetime) { return $null }
    return $owner
}

function Test-OwUeLockStale([string]$Path = (Get-OwUeLockPath)) {
    # True only when the lock names an owner process and that exact process no longer exists.
    $owner = Get-OwUeLockOwner $Path
    if (!$owner) { return $false }
    $process = Get-Process -Id ([int]$owner.pid) -ErrorAction SilentlyContinue
    if (!$process) { return $true }
    try { $start = $process.StartTime.ToFileTimeUtc() } catch { return $false }   # cannot tell: keep the lock
    return $start -ne [long]$owner.process_start_utc_filetime
}

function Get-OwUeBusyProcess([string[]]$AlsoWaitFor = @()) {
    # OPENWILLOW_UE_LOCK_TEST_PROCESS_NAMES (comma-separated) replaces the two Unreal names; tests/ue_lock_test.py only.
    $names = if ($env:OPENWILLOW_UE_LOCK_TEST_PROCESS_NAMES) { $env:OPENWILLOW_UE_LOCK_TEST_PROCESS_NAMES -split ',' } else { @('UnrealEditor', 'UnrealEditor-Cmd') }
    Get-Process -Name (@($names) + $AlsoWaitFor) -ErrorAction SilentlyContinue
}

function Enter-OwUeLock {
    param(
        [Parameter(Mandatory)][string]$Owner,
        [int]$TimeoutSeconds = 600,
        [int]$PollSeconds = 5,
        # More process names to wait for besides UnrealEditor and UnrealEditor-Cmd (e.g. Borderlands2).
        [string[]]$AlsoWaitFor = @()
    )
    # Waits until no Unreal editor or commandlet runs and the lock is free (or stale), then creates it atomically.
    $path = Get-OwUeLockPath
    $self = Get-Process -Id $PID
    $record = [ordered]@{
        owner = $Owner; pid = $PID; process_start_utc_filetime = $self.StartTime.ToFileTimeUtc()
        process_start = $self.StartTime.ToUniversalTime().ToString('o'); acquired = (Get-Date).ToUniversalTime().ToString('o')
    } | ConvertTo-Json -Compress
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $said = $false
    while ($true) {
        if (!(Get-OwUeBusyProcess $AlsoWaitFor)) {
            if ((Test-Path -LiteralPath $path) -and (Test-OwUeLockStale $path)) {
                Write-Host "Removing stale UE run lock (its owner process is gone): $([IO.File]::ReadAllText($path))"
                Remove-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
            }
            $stream = $null
            try { $stream = [IO.File]::Open($path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read) } catch { $stream = $null }
            if ($stream) {
                try { $bytes = [Text.Encoding]::UTF8.GetBytes($record); $stream.Write($bytes, 0, $bytes.Length) } finally { $stream.Dispose() }
                return
            }
        }
        if ((Get-Date) -gt $deadline) {
            $held = if (Test-Path -LiteralPath $path) { [IO.File]::ReadAllText($path) } else { '(no lock file)' }
            throw "Another Unreal editor or the UE run lock $path is still present (held by: $held); leave it untouched."
        }
        if (!$said) { Write-Host "Waiting for the UE run lock $path or a running Unreal editor..."; $said = $true }
        Start-Sleep -Seconds $PollSeconds
    }
}

function Exit-OwUeLock {
    # Removes the lock only when this PowerShell process owns it.
    $path = Get-OwUeLockPath
    $owner = Get-OwUeLockOwner $path
    if ($owner -and [int]$owner.pid -eq $PID -and [long]$owner.process_start_utc_filetime -eq (Get-Process -Id $PID).StartTime.ToFileTimeUtc()) {
        Remove-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
    }
}

function Invoke-OwUeLocked {
    # Runs a script block under the lock: Enter-OwUeLock, the block, then Exit-OwUeLock even when the block throws.
    # The block runs in its own scope: it can read the caller's variables, and $LASTEXITCODE is visible afterwards.
    param(
        [Parameter(Mandatory)][string]$Owner,
        [Parameter(Mandatory)][scriptblock]$Body,
        [int]$TimeoutSeconds = 600
    )
    Enter-OwUeLock -Owner $Owner -TimeoutSeconds $TimeoutSeconds
    try { & $Body } finally { Exit-OwUeLock }
}

function Invoke-OwUeEditor {
    # Starts UnrealEditor.exe and waits for it to exit (`& $editor` does not wait for a GUI program, so a lock released
    # after it would be released while the editor still runs). Arguments with spaces or quotes are quoted the way
    # PowerShell quotes them for `&`. Returns nothing; throws on a non-zero exit code unless -AllowFailure.
    param(
        [Parameter(Mandatory)][string]$FilePath,
        [string[]]$Arguments = @(),
        [switch]$AllowFailure
    )
    $quoted = @($Arguments | ForEach-Object { if ($_ -match '[\s"]' -and $_ -notmatch '^".*"$') { '"' + $_.Replace('"', '\"') + '"' } else { $_ } })
    $process = Start-Process -FilePath $FilePath -ArgumentList $quoted -PassThru
    $null = $process.Handle   # without this, Start-Process -PassThru does not populate ExitCode
    $process.WaitForExit()
    if (!$AllowFailure -and $process.ExitCode -ne 0) { throw "$FilePath exited with $($process.ExitCode)" }
}
