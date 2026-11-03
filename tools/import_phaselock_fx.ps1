[CmdletBinding()]
param(
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    # UModel build 1590 export roots (PNG textures, .mat slot lists, glTF meshes), all under ignored local/.
    [string[]]$UModelRoots = @('local/phaselock/umodel-test/siren', 'local/phaselock/umodel-test/targeted', 'local/phaselock/umodel-b2'),
    # research/particle_system.py output.
    [string]$Emitters = 'local/phaselock/emitters',
    # Optional: export the FX materials UModel's package-wide run did not reach (smoke, lens flare) before importing.
    [string]$UModel = $env:OPENWILLOW_UMODEL,
    [string]$Game = $env:OPENWILLOW_BL2,
    [int]$WaitForEditorSeconds = 1800
)
# AI-assisted. Imports Phaselock's effect textures and meshes and builds host materials with
# host/ue5/import_phaselock_fx.py (recreates /Game/OpenWillow/Phaselock only). Holds the UE run lock (tools/ue_lock.ps1) for the editor
# launch and stops if another Unreal editor runs (never touches it). Report: local/phaselock/fx_import.json.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$cmd = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor-Cmd.exe'
. (Join-Path $PSScriptRoot 'ue_lock.ps1')
$logs = Join-Path $repo 'local/phaselock/logs'
New-Item -ItemType Directory -Force -Path $logs | Out-Null

if ($UModel -and $Game) {
    $out = Join-Path $repo 'local/phaselock/umodel-b2'
    $cooked = Join-Path $Game 'WillowGame/CookedPCConsole'
    foreach ($object in @('Mat_Wispy_Smoke', 'Mati_Wispy_Smoke_Cloud_SubUV', 'Mat_Lens_Flare_Wide_Prime')) {
        & $UModel -export -game=border -png -groups "-path=$cooked" "-out=$out" Startup.upk $object *> (Join-Path $logs "umodel-$object.log")
    }
}
$roots = ($UModelRoots | ForEach-Object { (Resolve-Path -LiteralPath (Join-Path $repo $_)).Path }) -join ';'
$env:OPENWILLOW_PHASELOCK_UMODEL = $roots
$env:OPENWILLOW_PHASELOCK_EMITTERS = (Resolve-Path -LiteralPath (Join-Path $repo $Emitters)).Path
$env:OPENWILLOW_PHASELOCK_REPORT = Join-Path $repo 'local/phaselock/fx_import.json'

# Waits for the shared run lock (another lane's editor, a capture or a suite) and for no Unreal editor process, then claims
# the lock atomically; it never touches another editor.
Enter-OwUeLock -Owner 'import_phaselock_fx' -TimeoutSeconds $WaitForEditorSeconds
try {
    $log = Join-Path $logs 'import.log'
    $process = Start-Process -FilePath $cmd -PassThru -NoNewWindow -RedirectStandardOutput (Join-Path $logs 'import.stdout.txt') -ArgumentList @(
        "`"$project`"", '-run=pythonscript', "`"-script=$(Join-Path $repo 'host/ue5/import_phaselock_fx.py')`"", '-unattended',
        '-nullrhi', '-nosplash', '-ddc=InstalledNoZenLocalFallback', "`"-abslog=$log`"")
    $null = $process.Handle
    try {
        if (!$process.WaitForExit(1500000)) { throw 'import timed out' }
        $code = $process.ExitCode
    } finally {
        if (!$process.HasExited) { Stop-Process -Id $process.Id -Force }
    }
    Select-String -LiteralPath $log -Pattern 'LogPython: (OW_|Error)|Traceback' | ForEach-Object { $_.Line -replace '.*LogPython: ', '' }
    if ($code -ne 0 -or (Select-String -LiteralPath $log -Pattern 'Traceback|LogPython: Error' -Quiet)) { throw "import failed (exit $code); see $log" }
} finally { Exit-OwUeLock }
