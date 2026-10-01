param(
    # Unreal Engine install (the folder that contains Engine/).
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    # Borderlands 2 install, only used by the overlay/asset paths.
    [string]$Game = $env:OPENWILLOW_BL2,
    [int]$Port = 8784,
    # Whole run, from launch to the summary line.
    [int]$TimeoutSeconds = 420,
    # How long to wait for another run's ue_run.lock before giving up.
    [int]$LockWaitSeconds = 900,
    # Unlocked weapon slots for the run (the locked-slot steps need 2).
    [ValidateRange(2, 4)][int]$Slots = 2,
    # Send the page synthetic DOM key events instead of Slate key events.
    [switch]$JsKeys,
    # Extra command-line switches for UnrealEditor. The defaults are the launch-only cache fallback and D3D11
    # that completed in this environment (docs/verification/INVENTORY_MOVIE_PROTOTYPE.md, 2026-10-01); they
    # change no project cache/renderer settings. Pass -Extra @() to launch without them.
    [string[]]$Extra = @('-ddc=InstalledNoZenLocalFallback', '-d3d11')
)
# In-engine functional test of the imported inventory page <-> host round
# trip (-owinventoryactions, UOpenWillowInventoryActionTest): open the page,
# equip, unequip, favorite, trash, drop, pick up, locked slot and shield level
# gate, each checked against the host inventory and the page's own snapshot.
# Prints one row per OWINVTEST step from OpenWillow.log with its status
# (PASS / FAIL / NOT_RUN = the step's own precondition did not hold / KNOWN_DIVERGENCE =
# host behaviour documented as different from the original game, never counted as a pass)
# and a tally. Exit codes: 0 all PASS; 1 FAIL or NOT_RUN present; 2 no summary (launch/timeout);
# 3 PASS_WITH_KNOWN_DIVERGENCE. Refuses to start when an Unreal editor is already running, and
# always stops the editor it started and releases local/ue_run.lock.
# Build the editor target first (docs/TOOLING.md).
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
if (!(Test-Path -LiteralPath $editor)) { throw "UnrealEditor.exe not found under $Engine" }
if (!$Game -or !(Test-Path -LiteralPath $Game)) { throw 'Pass -Game <Borderlands 2 folder> or set OPENWILLOW_BL2' }
$localDir = Join-Path $repo 'local'
New-Item -ItemType Directory -Force -Path $localDir | Out-Null
$lockPath = Join-Path $localDir 'ue_run.lock'
$logDir = Join-Path $localDir 'inventory-actions'
New-Item -ItemType Directory -Force -Path $logDir | Out-Null
$log = Join-Path $logDir ('run-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')

# --- lock: created atomically, stale after 15 minutes, removed in finally ---
function Get-Lock {
    $deadline = (Get-Date).AddSeconds($LockWaitSeconds)
    while ($true) {
        try {
            $stream = [System.IO.File]::Open($lockPath, [System.IO.FileMode]::CreateNew,
                [System.IO.FileAccess]::Write, [System.IO.FileShare]::Read)
            $bytes = [System.Text.Encoding]::UTF8.GetBytes("test_inventory_actions $((Get-Date).ToString('o'))")
            $stream.Write($bytes, 0, $bytes.Length)
            $stream.Dispose()
            return
        } catch [System.IO.IOException] {
            if (!(Test-Path -LiteralPath $lockPath)) { continue }
            $age = (Get-Date) - (Get-Item -LiteralPath $lockPath).LastWriteTime
            if ($age.TotalMinutes -gt 15) {
                Write-Output "Removing stale lock ($([int]$age.TotalMinutes) min old): $(Get-Content -LiteralPath $lockPath -Raw)"
                Remove-Item -LiteralPath $lockPath -Force -ErrorAction SilentlyContinue
                continue
            }
            if ((Get-Date) -gt $deadline) { throw "Timed out waiting for $lockPath held by: $(Get-Content -LiteralPath $lockPath -Raw)" }
            Write-Output "Waiting for $lockPath (held by: $(Get-Content -LiteralPath $lockPath -Raw))"
            Start-Sleep -Seconds 10
        }
    }
}

function Read-LogText {
    if (!(Test-Path -LiteralPath $log)) { return '' }
    try {
        $stream = [System.IO.File]::Open($log, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite)
        $reader = New-Object System.IO.StreamReader($stream)
        try { return $reader.ReadToEnd() } finally { $reader.Dispose() }
    } catch { return '' }
}

$exitCode = 1
$process = $null
$server = $null
if (Get-Process UnrealEditor -ErrorAction SilentlyContinue) { throw 'An Unreal editor is already running; leave it untouched.' }
Get-Lock
try {
    # --- overlay server (the pages the game window loads) ---
    $url = "http://127.0.0.1:$Port/"
    $pages = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot 'hud_overlay')).Path
    try { $running = Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 $url } catch { $running = $null }
    if ($running) {
        $served = "$($running.Headers['X-OpenWillow-Pages'])"
        if ($served -ne $pages) { throw "Port $Port serves pages from '$served', not '$pages'. Stop that server or pass -Port." }
    } else {
        $server = Start-Process -WindowStyle Hidden -PassThru -FilePath python -ArgumentList @(
            "`"$(Join-Path $PSScriptRoot 'hud_overlay/serve.py')`"", '--port', $Port)
        Start-Sleep 2
        Invoke-WebRequest -UseBasicParsing -TimeoutSec 5 $url | Out-Null
    }

    $env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
    $env:OPENWILLOW_SCENE = Join-Path $repo 'local/sanctuary'
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya',
        "-owflashhud=$url", "-owflashskills=${url}skills.html", "-owflashinventory=${url}inventory.html",
        '-owinventoryactions', "-owslots=$Slots", '-game', '-windowed', '-ResX=1280', '-ResY=720',
        '-nosplash', '-unattended', "-abslog=`"$log`"")
    if ($JsKeys) { $arguments += '-owinventoryjskeys' }
    $arguments += $Extra
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Output "Launched UnrealEditor pid $($process.Id); log: $log"

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $summary = $null
    while ($true) {
        $text = Read-LogText
        $found = [regex]::Match($text, 'OWINVTEST SUMMARY (.*)')
        if ($found.Success) { $summary = $found.Groups[1].Value.Trim(); break }
        if ($process.HasExited) { break }
        if ((Get-Date) -gt $deadline) { break }
        Start-Sleep -Seconds 2
    }
    if ($summary) { [void]$process.WaitForExit(20000) }
    $text = Read-LogText

    Write-Output ''
    $tally = [ordered]@{ PASS = 0; FAIL = 0; NOT_RUN = 0; KNOWN_DIVERGENCE = 0 }
    foreach ($m in [regex]::Matches($text, 'OWINVTEST step=(\d+) action=(\S+) status=(\S+) ok=([01]) detail=(.*)')) {
        $status = $m.Groups[3].Value
        if ($tally.Contains($status)) { $tally[$status]++ }
        Write-Output ('{0,-16} step {1,2} {2,-44} {3}' -f $status, $m.Groups[1].Value, $m.Groups[2].Value, $m.Groups[5].Value.Trim())
    }
    foreach ($m in [regex]::Matches($text, 'OWINVTEST page report at failure: (.*)')) { Write-Output "     page: $($m.Groups[1].Value.Trim())" }
    Write-Output ''
    Write-Output ('Rows: ' + (($tally.GetEnumerator() | ForEach-Object { "$($_.Key)=$($_.Value)" }) -join ' '))
    if (!$summary) {
        $reason = if ($process.HasExited) { "editor exited (code $($process.ExitCode)) before the summary" } else { "no summary within $TimeoutSeconds s" }
        Write-Output "OWINVTEST SUMMARY result=FAIL reason=$reason"
        Write-Output "Log: $log"
        $exitCode = 2
    } else {
        Write-Output "OWINVTEST SUMMARY $summary"
        Write-Output "Log: $log"
        $exitCode = if ($summary -match '^result=PASS ') { 0 } elseif ($summary -match '^result=PASS_WITH_KNOWN_DIVERGENCE ') { 3 } else { 1 }
    }
} finally {
    if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    if ($server -and !$server.HasExited) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item -LiteralPath $lockPath -Force -ErrorAction SilentlyContinue
}
exit $exitCode
