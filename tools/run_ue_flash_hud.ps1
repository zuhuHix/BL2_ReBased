param(
    [Parameter(Mandatory=$true)][string]$Engine,
    [Parameter(Mandatory=$true)][string]$Game,
    # Start as a standalone game window instead of the editor.
    [switch]$GameWindow,
    # Experimental imported inventory; requires prepare_inventory_movie.py.
    [switch]$InventoryMovie,
    # Extra command-line switches, e.g. '-owcombattest', '-owcombatshots'.
    [string[]]$Extra = @(),
    [int]$Port = 8767
)
# Prototype: walk Sanctuary as Maya with BL2's own HUD movie drawn by Ruffle in
# a transparent web page over the viewport (-owflashhud). Needs the converted
# movies and Ruffle under local/ui/run (DECISIONS.md 2026-09-26) and a built
# OpenWillow module. In the editor, press Play once it opens.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$url = "http://127.0.0.1:$Port/"

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
    Start-Process -WindowStyle Hidden -FilePath python -ArgumentList @(
        "`"$(Join-Path $PSScriptRoot 'hud_overlay/serve.py')`"", '--port', $Port)
    Start-Sleep 2
    Invoke-WebRequest -UseBasicParsing -TimeoutSec 5 $url | Out-Null
}

$env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
$env:OPENWILLOW_SCENE = Join-Path $repo 'local/sanctuary'
$arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya', "-owflashhud=$url", "-owflashskills=${url}skills.html") + $Extra
if ($InventoryMovie) { $arguments += "-owflashinventory=${url}inventory.html" }
if ($GameWindow) { $arguments += @('-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash') }
Start-Process -FilePath $editor -ArgumentList $arguments
