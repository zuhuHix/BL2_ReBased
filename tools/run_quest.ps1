param(
    [string]$Engine = 'C:\Program Files\Epic Games\UE_5.8',
    [string]$Game = $env:OPENWILLOW_BL2,
    [string]$Save,
    [int]$Level = 0,
    [switch]$Fresh,
    [int]$Seconds = 0
)
# Hand play of the Sanctuary slice mission (no automated steps): Maya in Sanctuary with the stock mission, Marcus,
# the range trigger, the stock target dummy and its Matinee from the ignored local/ manifests, and the slice gear from
# local/items/slice (tools/weapon_slice_gear.py): Maya carries the pool-rolled slice guns and Marcus lends the stock
# mission pistol. Her level defaults to the slice gear level in slice_manifest.json (an UNVERIFIED slice choice, printed
# at launch) so she can use that gear; -Level N overrides it. Owns the shared
# the UE run lock (tools/ue_lock.ps1) while the window is open so the automated suites wait; waits itself while another editor or
# the lock exists. -Fresh deletes the hand-play save first; -Seconds N closes the window after N seconds (smoke check). Keys: see docs/verification/SANCTUARY_RPG_MISSION.md.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (!$Save) { $Save = Join-Path $repo 'local/quest/manual-save.json' }
$files = @{ mover = 'local/doors/mover.json'; slice = 'local/slice/world.json'; npcs = 'local/slice/npc_assets.json'; audio = 'local/slice/audio.json' }
$files.items = 'local/items/slice/slice_manifest.json'
$files.actionskill = 'local/character/action_skill_siren.json'
foreach ($key in @($files.Keys)) {
    $files[$key] = Join-Path $repo $files[$key]
    if (!(Test-Path -LiteralPath $files[$key])) { throw "Missing $($files[$key])" }
}
$itemDir = Split-Path -Parent $files.items
$gearLevel = [int](Get-Content -LiteralPath $files.items -Raw | ConvertFrom-Json).level
if ($Level -le 0) { $Level = $gearLevel; Write-Output "Maya level $Level = slice gear level (slice_manifest.json; UNVERIFIED slice choice, not a save)" }
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
. (Join-Path $PSScriptRoot 'ue_lock.ps1')
New-Item -ItemType Directory -Force (Split-Path $Save) | Out-Null
if ($Fresh -and (Test-Path -LiteralPath $Save)) { Remove-Item -LiteralPath $Save -Force }
Enter-OwUeLock -Owner 'run_quest' -TimeoutSeconds 86400   # waits as long as another run holds it
try {
    $env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
    $log = Join-Path $repo ('local/quest/manual-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya',
        "-owmover=`"$($files.mover)`"", "-owslice=`"$($files.slice)`"", "-ownpcs=`"$($files.npcs)`"", "-owaudio=`"$($files.audio)`"",
        '-owquest', "-owquestsave=`"$Save`"", "-owitems=`"$itemDir`"", "-owactionskill=`"$($files.actionskill)`"", "-owlevel=$Level", '-game', '-windowed', '-ResX=1600', '-ResY=900', '-nosplash',
        '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"")
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Output "Launched hand-play session pid $($process.Id); log: $log (close the window to finish)"
    if ($Seconds -gt 0) {
        if (!$process.WaitForExit($Seconds * 1000)) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue; Start-Sleep -Seconds 3 }
    } else { $process.WaitForExit() }
    Select-String -LiteralPath $log -Pattern 'OWQUEST|OWMOVER TRACK|not found|Maya level|lent weapon|returned lent|Phaselock|equipped slot' -ErrorAction SilentlyContinue |
        ForEach-Object { $_.Line -replace '^.*LogTemp: (Display: |Warning: |Error: )?', '' }
} finally {
    Exit-OwUeLock
}
