param(
    # A script for the host's test-only player driver (-owplaytest, OpenWillowPlaytest.h).
    [Parameter(Mandatory)][string]$Script,
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    [string]$Game = $env:OPENWILLOW_BL2,
    # X,Y,Z[,Yaw]: where ow.Range builds the range. Default: none, the host resolves the spot from the manifest.
    [string]$RangeAt,
    [int]$Level = 0,
    [int]$Money = 5000,
    [int]$Port = 8767,
    [int]$TimeoutSeconds = 600,
    [int]$LockWaitSeconds = 3600,
    # Extra switches, e.g. '-owcanvashud' to compare with the fallback HUD.
    [string[]]$Extra = @(),
    [string]$OutRoot
)
# AI-assisted (Claude). One -game session of Sanctuary_P with every slice system on at once (the movie HUD, the mission,
# vendors, lootables, range, weapon generator), driven by a text script through OpenWillowPlaytest. Writes the log, the
# frames and a warning summary to local/playtest/<script name>-<stamp>/ (ignored). Host behaviour only.
# Takes the machine-wide UE lock (tools/ue_lock.ps1). Stops only the editor and server this run started.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (!$Game) { throw 'Set OPENWILLOW_BL2 or pass -Game.' }
$env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
$env:OPENWILLOW_SCENE = Join-Path $repo 'local/sanctuary'
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$scriptPath = (Resolve-Path -LiteralPath $Script).Path
$need = [ordered]@{
    owmover = 'local/doors/mover.json'; owslice = 'local/slice/world.json'; ownpcs = 'local/slice/npc_assets.json'
    owaudio = 'local/slice/audio.json'; owactionskill = 'local/character/action_skill_siren.json'
    owvendors = 'local/slice/vendors.json'; owinteractives = 'local/slice/interactives.json'
    owrange = 'local/range/range_manifest.json'; owweapongen = 'local/weapons/gen/weapon_gen_manifest.txt'
}
$gear = Join-Path $repo 'local/items/slice/slice_manifest.json'
$switches = @()
foreach ($key in $need.Keys) {
    $file = Join-Path $repo $need[$key]
    if (Test-Path -LiteralPath $file) { $switches += "-$key=`"$file`"" } else { Write-Output "MISSING $file (no -$key)" }
}
if ($Level -le 0) { $Level = [int](Get-Content -LiteralPath $gear -Raw | ConvertFrom-Json).level }
$name = [IO.Path]::GetFileNameWithoutExtension($scriptPath)
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
if (!$OutRoot) { $OutRoot = Join-Path $repo 'local/playtest' }
$out = Join-Path $OutRoot "$name-$stamp"
New-Item -ItemType Directory -Force $out | Out-Null
$log = Join-Path $out 'run.log'
$shots = Join-Path $repo 'host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor'
. (Join-Path $PSScriptRoot 'ue_lock.ps1')
Enter-OwUeLock -Owner 'run_playtest' -TimeoutSeconds $LockWaitSeconds -PollSeconds 10
$server = $null
$code = 2
try {
    if (Test-Path -LiteralPath $shots) { Get-ChildItem -LiteralPath $shots -Filter 'OWPlay_*.png' | Remove-Item -Force }
    $server = Start-Process -PassThru -WindowStyle Hidden -FilePath python -ArgumentList @("`"$(Join-Path $PSScriptRoot 'hud_overlay/serve.py')`"", '--port', $Port)
    Start-Sleep 2
    $url = "http://127.0.0.1:$Port/"
    Invoke-WebRequest -UseBasicParsing -TimeoutSec 5 $url | Out-Null
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya', "-owflashhud=$url",
        "-owvendingpage=${url}vending.html", "-owitems=`"$(Split-Path -Parent $gear)`"", "-owlevel=$Level", "-owmoney=$Money", '-owerid=7',
        '-owquest', "-owquestsave=`"$(Join-Path $out 'quest_save.json')`"", "-owcurrencysave=`"$(Join-Path $out 'currency.json')`"",
        "-owplaytest=`"$scriptPath`"") + $switches + $Extra + @(
        '-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-unattended', '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"")
    if ($RangeAt) { $arguments += "-owrangeat=$RangeAt" }
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    $null = $process.Handle   # without this, Start-Process -PassThru does not populate ExitCode
    Write-Output "Launched pid $($process.Id); log: $log"
    if ($process.WaitForExit($TimeoutSeconds * 1000)) {
        $code = $process.ExitCode
        Write-Output "Exit code: $code"
    } else {
        $code = 3
        $commandLine = (Get-CimInstance Win32_Process -Filter "ProcessId=$($process.Id)" -ErrorAction SilentlyContinue).CommandLine
        if ($commandLine -and $commandLine.Contains($log)) { Stop-Process -Id $process.Id -Force; Write-Output "Timed out; stopped pid $($process.Id)." }
    }
} finally {
    if ($server -and !$server.HasExited) {
        $commandLine = (Get-CimInstance Win32_Process -Filter "ProcessId=$($server.Id)" -ErrorAction SilentlyContinue).CommandLine
        if ($commandLine -and $commandLine.Contains('serve.py')) { Stop-Process -Id $server.Id -Force }
    }
    if (Test-Path -LiteralPath $shots) { Get-ChildItem -LiteralPath $shots -Filter 'OWPlay_*.png' | Move-Item -Destination $out -Force }
    Exit-OwUeLock
}
Write-Output "Out: $out"
exit $code
