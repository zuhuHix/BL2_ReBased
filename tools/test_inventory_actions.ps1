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
    # Extra command-line switches for UnrealEditor.
    [string[]]$Extra = @()
)
# In-engine functional test of the imported inventory page <-> host round
# trip (-owinventoryactions, UOpenWillowInventoryActionTest): open the page,
# equip, unequip, favorite, trash, drop, pick up, locked slot and shield level
# gate, each checked against the host inventory and the page's own snapshot.
# Prints one row per OWINVTEST step from OpenWillow.log, exits 0 only when the
# summary line says PASS, and always stops the editor it started and releases
# local/ue_run.lock. Build the editor target first (docs/TOOLING.md).
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
    foreach ($m in [regex]::Matches($text, 'OWINVTEST step=(\d+) action=(\S+) ok=([01]) detail=(.*)')) {
        $status = if ($m.Groups[3].Value -eq '1') { 'PASS' } else { 'FAIL' }
        Write-Output ('{0,-4} step {1,2} {2,-36} {3}' -f $status, $m.Groups[1].Value, $m.Groups[2].Value, $m.Groups[4].Value.Trim())
    }
    foreach ($m in [regex]::Matches($text, 'OWINVTEST page report at failure: (.*)')) { Write-Output "     page: $($m.Groups[1].Value.Trim())" }
    Write-Output ''
    if (!$summary) {
        $reason = if ($process.HasExited) { "editor exited (code $($process.ExitCode)) before the summary" } else { "no summary within $TimeoutSeconds s" }
        Write-Output "OWINVTEST SUMMARY result=FAIL reason=$reason"
        Write-Output "Log: $log"
        $exitCode = 2
    } else {
        Write-Output "OWINVTEST SUMMARY $summary"
        Write-Output "Log: $log"
        $exitCode = if ($summary -match '^result=PASS ') { 0 } else { 1 }
    }
} finally {
    if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    if ($server -and !$server.HasExited) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item -LiteralPath $lockPath -Force -ErrorAction SilentlyContinue
}
exit $exitCode
