param(
    [string]$Engine = 'C:\Program Files\Epic Games\UE_5.8',
    [string]$Game = $env:OPENWILLOW_BL2,
    [string]$Save,
    [int]$Level = 0,
    [switch]$Fresh,
    [int]$Seconds = 0
)
# Hand play of the Sanctuary slice mission (no automated steps): Maya in Sanctuary with the stock mission, Marcus,
# the range trigger, the stock target dummy and its Matinee from the ignored local/ manifests. Owns the shared
# local/ue_run.lock while the window is open so the automated suites wait; waits itself while another editor or
# the lock exists. -Fresh deletes the hand-play save first; -Seconds N closes the window after N seconds (smoke check). Keys: see docs/verification/SANCTUARY_RPG_MISSION.md.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (!$Save) { $Save = Join-Path $repo 'local/quest/manual-save.json' }
$files = @{ mover = 'local/doors/mover.json'; slice = 'local/slice/world.json'; npcs = 'local/slice/npc_assets.json'; audio = 'local/slice/audio.json' }
foreach ($key in @($files.Keys)) {
    $files[$key] = Join-Path $repo $files[$key]
    if (!(Test-Path -LiteralPath $files[$key])) { throw "Missing $($files[$key])" }
}
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$lock = Join-Path $repo 'local/ue_run.lock'
$stream = $null
while (!$stream) {
    if (!(Get-Process UnrealEditor -ErrorAction SilentlyContinue)) {
        try { $stream = [System.IO.File]::Open($lock, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::Read) } catch { $stream = $null }
    }
    if (!$stream) { Write-Output 'Waiting for another editor / lock holder to finish...'; Start-Sleep -Seconds 5 }
}
New-Item -ItemType Directory -Force (Split-Path $Save) | Out-Null
if ($Fresh -and (Test-Path -LiteralPath $Save)) { Remove-Item -LiteralPath $Save -Force }
$bytes = [System.Text.Encoding]::UTF8.GetBytes("run_quest $((Get-Date).ToString('o'))")
$stream.Write($bytes, 0, $bytes.Length); $stream.Dispose()
try {
    $env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
    $log = Join-Path $repo ('local/quest/manual-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya',
        "-owmover=`"$($files.mover)`"", "-owslice=`"$($files.slice)`"", "-ownpcs=`"$($files.npcs)`"", "-owaudio=`"$($files.audio)`"",
        '-owquest', "-owquestsave=`"$Save`"", '-game', '-windowed', '-ResX=1600', '-ResY=900', '-nosplash',
        '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"")
    if ($Level -gt 0) { $arguments += "-owlevel=$Level" }
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Output "Launched hand-play session pid $($process.Id); log: $log (close the window to finish)"
    if ($Seconds -gt 0) {
        if (!$process.WaitForExit($Seconds * 1000)) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue; Start-Sleep -Seconds 3 }
    } else { $process.WaitForExit() }
    Select-String -LiteralPath $log -Pattern 'OWQUEST|OWMOVER TRACK|not found|Maya level' -ErrorAction SilentlyContinue |
        ForEach-Object { $_.Line -replace '^.*LogTemp: (Display: |Warning: |Error: )?', '' }
} finally {
    Remove-Item -LiteralPath $lock -Force
}
