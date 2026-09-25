param(
    [Parameter(Mandatory=$true)][string]$Engine,
    [Parameter(Mandatory=$true)][string]$Game,
    [Parameter(Mandatory=$true)][string]$ArmsMesh,
    [Parameter(Mandatory=$true)][string]$PistolAnimSet,
    [Parameter(Mandatory=$true)][string]$SirenAnimSet,
    [string]$Python = 'python'
)

# Inputs are local UModel build-1590 MD5 exports from the user's installation.
# Only ignored local JSON and UE generated Content are written.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$out = Join-Path $repo 'local/character/anim'
$reference = Join-Path $out 'ref_pose.json'
if (!(Test-Path -LiteralPath $reference)) { throw 'Import Maya first to generate local/character/anim/ref_pose.json' }
$gamePath = (Resolve-Path -LiteralPath $Game).Path
if (!(Test-Path -LiteralPath (Join-Path $gamePath 'Binaries/Win32/Borderlands2.exe'))) {
    throw 'Game must point to an installed Borderlands 2 directory'
}
$meshPath = (Resolve-Path -LiteralPath $ArmsMesh).Path
$pistolPath = (Resolve-Path -LiteralPath $PistolAnimSet).Path
$sirenPath = (Resolve-Path -LiteralPath $SirenAnimSet).Path
$converter = Join-Path $repo 'tools/prepare_character_anims.py'
$pistolJson = Join-Path $out 'pistol_combat.json'
$sirenJson = Join-Path $out 'siren_combat.json'
& $Python $converter --mesh $meshPath --reference $reference --animset $pistolPath `
    --clips Draw ADD_Fire_Recoil --output $pistolJson
if ($LASTEXITCODE -ne 0) { throw 'Pistol animation conversion failed' }
& $Python $converter --mesh $meshPath --reference $reference --animset $sirenPath `
    --clips Phase_Lock_Lift Phase_Lock_Fail --output $sirenJson
if ($LASTEXITCODE -ne 0) { throw 'Phaselock animation conversion failed' }

$env:OPENWILLOW_BL2 = $gamePath
$env:OPENWILLOW_CHARACTER_REFERENCE = ''
$env:OPENWILLOW_CHARACTER_ANIMS = "PistolCombat=$pistolJson;SirenCombat=$sirenJson"
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$script = Join-Path $repo 'host/ue5/import_character_anims.py'
$commandlet = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor-Cmd.exe'
$log = Join-Path $out 'combat-import.log'
& $commandlet $project -run=pythonscript "-script=$script" -unattended -nullrhi -nosplash "-abslog=$log" | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Combat animation import failed: $LASTEXITCODE; see $log" }
Select-String -LiteralPath $log -Pattern 'LogPython: OW_ANIM' | ForEach-Object { $_.Line -replace '.*LogPython: ', '' }
