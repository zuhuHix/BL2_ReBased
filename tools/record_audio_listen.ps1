param(
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    # The checkout whose ignored local/ holds the prepared slice manifests (world, npcs, gear, range, quest). Defaults to this one.
    [string]$DataRoot,
    # The play map tools/prepare_slice_audio.py wrote (decoded game sounds); defaults to <DataRoot>/local/slice/audio_play.json.
    [string]$AudioPlay,
    [string]$Gun = 'GD_Itempools.WeaponPools.Pool_Weapons_All_01_Common',
    [double]$Pause = 2.0,              # -owslicegatepause: seconds of quiet before each slice gate step
    [switch]$NoShield,                 # leave out -owslicegateshield
    [switch]$NoDownAfterFight,         # leave out -owslicegatedownafterfight (the revive stand-in)
    [int]$TimeoutSeconds = 900,
    [int]$WaitForEditorSeconds = 3600
)
# A listening session for the maintainer: one slice gate run (OpenWillowSliceGate.cpp, the same sequence tools/test_slice_gate.ps1 grades) with
# the host's sounds on (-owaudioplay), a pause before each step (-owslicegatepause) and the host recording its own main output
# (-owrecordaudio, the audio mixer's submix recorder: no external recorder). Writes, under ignored local/audio_listen/<date>/<time>/:
#   listen.wav, frames/*.png, run.log, and from tools/listen_report.py: cues.json, LISTEN.md, ANSWERS.md.
# Does not build the module (use the build slot first). Owns one editor and the machine-wide UE lock (tools/ue_lock.ps1); stops only the editor
# it started, after checking that its command line names this run's log. A WAV proves what the host sent to the mixer, not that a sound is right.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (!$DataRoot) { $DataRoot = $repo }
$DataRoot = (Resolve-Path -LiteralPath $DataRoot).Path
if (!$AudioPlay) { $AudioPlay = Join-Path $DataRoot 'local/slice/audio_play.json' }
if (!(Test-Path -LiteralPath $AudioPlay)) { throw "No play map at $AudioPlay (python tools/prepare_slice_audio.py writes it)" }
$AudioPlay = (Resolve-Path -LiteralPath $AudioPlay).Path
$manifests = [ordered]@{
    mover = 'local/doors/mover.json'; slice = 'local/slice/world.json'; npcs = 'local/slice/npc_assets.json'
    audio = 'local/slice/audio.json'; gear = 'local/items/slice/slice_manifest.json'; actionskill = 'local/character/action_skill_siren.json'
}
foreach ($key in @($manifests.Keys)) {
    $manifests[$key] = Join-Path $DataRoot $manifests[$key]
    if (!(Test-Path -LiteralPath $manifests[$key])) { throw "Missing $($manifests[$key]) (see tools/test_quest.ps1 for the tool that writes it)" }
}
$itemDir = Split-Path -Parent $manifests.gear
$gearLevel = [int](Get-Content -LiteralPath $manifests.gear -Raw | ConvertFrom-Json).level
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
if (!(Test-Path -LiteralPath $editor)) { throw "UnrealEditor.exe not found under $Engine" }
. (Join-Path $PSScriptRoot 'ue_lock.ps1')
$outDir = Join-Path $repo ("local/audio_listen/$(Get-Date -Format 'yyyy-MM-dd')/$(Get-Date -Format 'HHmmss')")
New-Item -ItemType Directory -Force (Join-Path $outDir 'frames') | Out-Null
$log = Join-Path $outDir 'run.log'
$wav = Join-Path $outDir 'listen.wav'
$save = Join-Path $outDir 'save.json'   # a fresh mission every run, never the slice gate's own save

$code = 2
$process = $null
Enter-OwUeLock -Owner 'record_audio_listen' -TimeoutSeconds $WaitForEditorSeconds
try {
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya', '-owcanvashud',
        "-owmover=`"$($manifests.mover)`"", "-owslice=`"$($manifests.slice)`"", "-ownpcs=`"$($manifests.npcs)`"",
        "-owaudio=`"$($manifests.audio)`"", '-owquest', "-owquestsave=`"$save`"", "-owitems=`"$itemDir`"",
        "-owactionskill=`"$($manifests.actionskill)`"", "-owlevel=$gearLevel", '-owslicegate', "-owslicegategun=$Gun",
        "-owslicegatepause=$Pause", "-owaudioplay=`"$AudioPlay`"", "-owrecordaudio=`"$wav`"")
    if (!$NoShield) { $arguments += '-owslicegateshield' }
    if (!$NoDownAfterFight) { $arguments += '-owslicegatedownafterfight' }
    $arguments += @('-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-unattended',
        '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"")
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    $null = $process.Handle   # keeps ExitCode readable
    Write-Host "Launched listening run pid $($process.Id); log: $log"
    if (!$process.WaitForExit($TimeoutSeconds * 1000)) {
        $commandLine = (Get-CimInstance Win32_Process -Filter "ProcessId=$($process.Id)" -ErrorAction SilentlyContinue).CommandLine
        if ($commandLine -and $commandLine.Contains($log)) {
            Stop-Process -Id $process.Id -Force
            Write-Host "Timed out after $TimeoutSeconds s; stopped pid $($process.Id) (this run's editor)."
        }
    }
    else { Write-Host "Editor exited with $($process.ExitCode)" }
    Start-Sleep -Seconds 2
    if (!(Test-Path -LiteralPath $log)) { throw "No log written: $log" }
    Select-String -LiteralPath $log -Pattern 'OWAUDIOREC (start|stop|written)|OWSLICEGATE (link|SUMMARY)' |
        ForEach-Object { Write-Host ($_.Line -replace '^.*LogTemp: (Display: |Warning: |Error: )?', '') }
    & python (Join-Path $PSScriptRoot 'listen_report.py') $log $wav $outDir --manifest $manifests.audio --play-map $AudioPlay
    $code = $LASTEXITCODE
    Write-Host "Output: $outDir"
} finally {
    Exit-OwUeLock
}
exit $code
