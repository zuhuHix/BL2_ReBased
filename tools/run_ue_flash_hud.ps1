param(
    [Parameter(Mandatory=$true)][string]$Engine,
    [Parameter(Mandatory=$true)][string]$Game,
    # Start as a standalone game window instead of the editor.
    [switch]$GameWindow,
   # Use the fallback host panel only when explicitly requested.
   [switch]$NoInventoryMovie,
    # Imported BL2 StatusMenu inventory is on by default.
    [switch]$InventoryMovie = $true,
    # Extra command-line switches, e.g. '-owcombattest', '-owcombatshots'.
    [string[]]$Extra = @(),
    [int]$Port = 8767,
    # With -GameWindow: seconds before a stalled run is stopped and this script fails. 0: no limit. Default: 20 minutes for a scripted
    # run (a switch that drives it and quits), none for a person playing. Never a limit without -GameWindow (tools/run_limit.ps1).
    [int]$TimeoutSeconds = -1
)
# Prototype: walk Sanctuary as Maya with BL2's own HUD movie drawn by Ruffle in
# a transparent web page over the viewport (-owflashhud). Needs the converted
# movies and Ruffle under local/ui/run (DECISIONS.md 2026-09-26) and a built
# OpenWillow module. In the editor, press Play once it opens.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
. (Join-Path $PSScriptRoot 'ue_lock.ps1')
. (Join-Path $PSScriptRoot 'run_limit.ps1')
# Without -GameWindow this opens the full editor, with no game until someone presses Play, so an unattended capture would wait for ever.
if (!$GameWindow -and (Test-OwScriptedRun -Extra $Extra)) {
    throw "run_ue_flash_hud: -Extra '$($Extra -join ' ')' drives a game run, but without -GameWindow this opens the editor and no game starts. Add -GameWindow."
}
$limit = Get-OwRunLimitSeconds -TimeoutSeconds $TimeoutSeconds -Extra $Extra -GameWindow:$GameWindow
$url = "http://127.0.0.1:$Port/"
# A page does without a missing movie (movie_guard.js), so say here what this run lacks; the pickup card capture cannot do without its own.
$overlayCheck = @(Join-Path $PSScriptRoot 'check_overlay_data.py')
if ($Extra -contains '-owpickupcardshots') { & python @overlayCheck --fail-on 'world pickup card'; if ($LASTEXITCODE -ne 0) { throw 'The pickup card capture needs its movie: see the lines above.' } }
else { & python @overlayCheck --warn-only }

# Reuse an overlay server that is already running, else start one. Another
# checkout's server would serve its own (possibly older) pages, so refuse it.
$pages = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot 'hud_overlay')).Path
try { $running = Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 $url }
catch { $running = $null }
if ($running) {
    $served = "$($running.Headers['X-OpenWillow-Pages'])"
    if ($served -ne $pages) {
        throw "Port $Port serves pages from '$served', not '$pages'. Stop that server or pass -Port."
    }
} else {
    $server = Start-Process -PassThru -WindowStyle Hidden -FilePath python -ArgumentList @(
        "`"$(Join-Path $PSScriptRoot 'hud_overlay/serve.py')`"", '--port', $Port)
    Start-Sleep 2
    Invoke-WebRequest -UseBasicParsing -TimeoutSec 5 $url | Out-Null
}

$env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
$env:OPENWILLOW_SCENE = Join-Path $repo 'local/sanctuary'
$arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya', "-owflashhud=$url", "-owflashskills=${url}skills.html") + $Extra
if ($InventoryMovie -and !$NoInventoryMovie) { $arguments += "-owflashinventory=${url}inventory.html" }
if ($GameWindow) { $arguments += @('-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash') }
# Holds the machine-wide UE run lock until the editor window is closed.
Invoke-OwUeLocked -Owner 'run_ue_flash_hud' -Body {
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Output "Editor pid $($process.Id); the UE run lock is held until it closes."
    if ($limit -gt 0) { Write-Output "Time limit $limit s (-TimeoutSeconds 0 for none)." }
    if (Wait-OwStartedProcess -Process $process -LimitSeconds $limit) { return }
    # Stalled: stop what this run started, nothing else, and fail. A server this run did not start is left running.
    $stopped = Stop-OwStartedProcess -Process $process -Expected @($project)
    if ($server) { [void](Stop-OwStartedProcess -Process $server -Expected @('serve.py', "--port $Port")) }
    throw "run_ue_flash_hud: the editor (pid $($process.Id)) was still running after $limit s: " + $(if ($stopped) { 'stalled, stopped.' } else { 'stalled, and NOT stopped.' })
}
