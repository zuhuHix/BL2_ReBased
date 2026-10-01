param(
    [string]$Engine = 'C:\Program Files\Epic Games\UE_5.8',
    [string]$Game = $env:OPENWILLOW_BL2,
    [string]$Manifest,
    [string]$World,
    [string]$Npcs,
    [string]$Audio,
    [int]$TimeoutSeconds = 300,
    [int]$WaitForEditorSeconds = 600
)
# Sanctuary slice loop in the host: stock Fire mission + dummy provider + the map's installed Kismet (door, Marcus's
# walk, target Matinee) with world data from the ignored slice manifests, death/respawn, save.
# Two editor launches: the first plays the loop and writes the save, the second resumes from it
# (progress retained across a restart). Owns one editor process at a time and the shared lock; waits while another
# editor or the lock exists and never touches them. Screenshots are copied to local/quest/.
# Reports host behaviour only; original-game parity is UNVERIFIED (see docs/verification/SANCTUARY_RPG_MISSION.md).
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (!$Manifest) { $Manifest = Join-Path $repo 'local/doors/mover.json' }
if (!$World) { $World = Join-Path $repo 'local/slice/world.json' }
if (!$Npcs) { $Npcs = Join-Path $repo 'local/slice/npc_assets.json' }
if (!$Audio) { $Audio = Join-Path $repo 'local/slice/audio.json' }
$gear = Join-Path $repo 'local/items/slice/slice_manifest.json'
$actionSkill = Join-Path $repo 'local/character/action_skill_siren.json'
foreach ($file in @($Manifest, $World, $Npcs, $Audio, $gear, $actionSkill)) {
    if (!(Test-Path -LiteralPath $file)) { throw "Missing manifest $file (tools/prepare_mover.py, prepare_slice_world.py, seed_slice_npc_assets.ps1, audio_slice_chain.py, weapon_slice_gear.py, prepare_action_skill.py)" }
}
# Maya starts at the slice gear level (UNVERIFIED slice choice) so the pool-rolled slice guns are usable.
$itemDir = Split-Path -Parent $gear
$gearLevel = [int](Get-Content -LiteralPath $gear -Raw | ConvertFrom-Json).level
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$lock = Join-Path $repo 'local/ue_run.lock'
$save = Join-Path $repo 'local/quest/save.json'
$shots = Join-Path $repo 'host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor'
# Take the lock as soon as no editor runs and nobody holds it (CreateNew is the atomic claim).
$deadline = (Get-Date).AddSeconds($WaitForEditorSeconds)
$stream = $null
while (!$stream) {
    if (!(Get-Process UnrealEditor -ErrorAction SilentlyContinue)) {
        try { $stream = [System.IO.File]::Open($lock, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::Read) } catch { $stream = $null }
    }
    if (!$stream) {
        if ((Get-Date) -gt $deadline) { throw 'Another Unreal editor or local/ue_run.lock is still present; leave it untouched.' }
        Start-Sleep -Seconds 5
    }
}
$bytes = [System.Text.Encoding]::UTF8.GetBytes("test_quest $((Get-Date).ToString('o'))")
$stream.Write($bytes, 0, $bytes.Length); $stream.Dispose()
New-Item -ItemType Directory -Force (Split-Path $save) | Out-Null
if (Test-Path -LiteralPath $save) { Remove-Item -LiteralPath $save -Force }
$code = 2
function Invoke-Run([string]$Mode, [string[]]$Extra) {
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $log = Join-Path $repo ('local/quest/run-' + $Mode + '-' + $stamp + '.log')
    $started = Get-Date
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya',
        "-owmover=`"$Manifest`"", "-owslice=`"$World`"", "-ownpcs=`"$Npcs`"", "-owaudio=`"$Audio`"",
        '-owquest', '-owquesttest', "-owquestsave=`"$save`"", "-owitems=`"$itemDir`"",
        "-owactionskill=`"$actionSkill`"", "-owlevel=$gearLevel") + $Extra + @(
        '-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-unattended',
        '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"")
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Host "Launched quest test ($Mode) pid $($process.Id); log: $log"
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $summary = ''
    try {
        while ((Get-Date) -lt $deadline) {
            $process.Refresh()
            $text = if (Test-Path -LiteralPath $log) { Get-Content -LiteralPath $log -Raw -ErrorAction SilentlyContinue } else { '' }
            $match = [regex]::Match($text, 'OWQUESTTEST SUMMARY (.*)')
            if ($match.Success) { $summary = $match.Groups[1].Value.Trim(); break }
            if ($process.HasExited) { break }
            Start-Sleep -Seconds 2
        }
    } finally {
        if (Test-Path -LiteralPath $log) {
            Select-String -LiteralPath $log -Pattern 'OWQUEST|OWMOVER |not found|lent weapon|returned lent|Phaselock|OpenWillow shot' | ForEach-Object { Write-Host ($_.Line -replace '^.*LogTemp: (Display: |Warning: |Error: )?', '') }
        }
        if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
        Start-Sleep -Seconds 3
    }
    # Screenshots written during this run go next to its log.
    if (Test-Path -LiteralPath $shots) {
        Get-ChildItem -LiteralPath $shots -Filter 'OWQuest_*.png' | Where-Object { $_.LastWriteTime -ge $started } | ForEach-Object {
            $target = Join-Path $repo ('local/quest/' + $_.BaseName + '-' + $stamp + '.png')
            Copy-Item -LiteralPath $_.FullName -Destination $target -Force
            Write-Host "Screenshot: $target"
        }
    }
    Write-Host "Log: $log"
    return $summary
}
try {
    $env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
    $first = Invoke-Run 'first' @()
    if (!$first) { Write-Output 'OWQUESTTEST SUMMARY result=FAIL reason=no summary (first run)' }
    elseif ($first -notmatch '^result=PASS ') { Write-Output "FIRST RUN: $first" }
    else {
        $second = Invoke-Run 'resume' @('-owquestresume')
        if (!$second) { Write-Output "FIRST RUN: $first"; Write-Output 'OWQUESTTEST SUMMARY result=FAIL reason=no summary (resume run)' }
        elseif ($second -match '^result=PASS ') { $code = 0; Write-Output "FIRST RUN: $first"; Write-Output "RESUME RUN: $second" }
        else { $code = 1; Write-Output "FIRST RUN: $first"; Write-Output "RESUME RUN: $second" }
    }
} finally {
    Remove-Item -LiteralPath $lock -Force
}
exit $code
