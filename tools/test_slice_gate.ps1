param(
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    [string]$Game = $env:OPENWILLOW_BL2,
    # ow.RollGun source for the no-drop fallback: a weapon balance or item pool (a base pool, as WEAPON_PART_ASSETS.md 6d used).
    [string]$Gun = 'GD_Itempools.WeaponPools.Pool_Weapons_All_01_Common',
    [int]$TimeoutSeconds = 600,
    [int]$WaitForEditorSeconds = 3600,
    [int]$MaxParallelActions = 12,
    [switch]$SkipBuild,
    [switch]$Shield,                    # -owslicegateshield: Maya fights with a shield (a fixture; see SLICE_GATE.md)
    [switch]$DownAfterFight,            # -owslicegatedownafterfight: lethal damage after the fight, to run the revive fixture
    [string]$AudioPlay,                 # -owaudioplay=<audio_play.json>: the host's sounds on during the run (tools/prepare_slice_audio.py); off when absent
    [switch]$Unforced,                  # -owslicegateunforced: the stand-ins for play left out (SLICE_GATE.md, "The unforced variant"); starts the page server
    [int]$Port = 8786                   # -Unforced only: the page server (tools/hud_overlay/serve.py) for the skills page and Marcus's mission page
)
# The Phase 4 slice gate as one unattended run (ROADMAP.md): Maya in Sanctuary_P with the Fire mission (-owquest, the
# same manifests as tools/test_quest.ps1) and -owslicegate, the host sequence in OpenWillowSliceGate.cpp. It spawns Maya,
# fights and kills an enemy (ow.Range when the build has it, else the host stand-in target), loots or rolls a gun, equips
# it, casts Phaselock on an enemy, runs the Fire mission from accept to turn-in, then goes down, bleeds out and respawns.
# tools/check_slice_gate.py grades the log: each link PASS, FAIL or NOT_AVAILABLE (a piece the host lacks yet), with
# the owning lane. Outputs go to local/slicegate/ (log, JSON, markdown table). Exit code: the checker's (0 = no FAIL).
# Builds the module first (Build.bat, -MaxParallelActions; waits while 2 UnrealBuildTool processes already run; -SkipBuild
# reuses the binary). Owns one editor at a time and the machine-wide UE lock (tools/ue_lock.ps1). Stops only the editor it
# started, after checking that its command line names this run's log. Host behaviour only: nothing here is checked
# against the real game.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
# -Unforced takes nothing from the two fixture variants: their shield and their lethal damage are stand-ins it leaves out.
if ($Unforced -and ($Shield -or $DownAfterFight)) { throw '-Unforced cannot be combined with -Shield or -DownAfterFight.' }
# Its loot link kills until a gun drops and its down link waits for the enemies, so it gets a longer limit unless one is given.
if ($Unforced -and !$PSBoundParameters.ContainsKey('TimeoutSeconds')) { $TimeoutSeconds = 1500 }
if (!$Game) { throw 'Set OPENWILLOW_BL2 or pass -Game (the installed Borderlands 2 folder).' }
$env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
$manifests = [ordered]@{
    mover = 'local/doors/mover.json'; slice = 'local/slice/world.json'; npcs = 'local/slice/npc_assets.json'
    audio = 'local/slice/audio.json'; gear = 'local/items/slice/slice_manifest.json'; actionskill = 'local/character/action_skill_siren.json'
}
foreach ($key in @($manifests.Keys)) {
    $manifests[$key] = Join-Path $repo $manifests[$key]
    if (!(Test-Path -LiteralPath $manifests[$key])) { throw "Missing $($manifests[$key]) (see tools/test_quest.ps1 for the tool that writes it)" }
}
# The fight uses ow.Range: a failed or old prepare_range.py stops here, as in tools/test_range.ps1, before any build or editor.
& python (Join-Path $repo 'tools/check_range.py') --preflight (Join-Path $repo 'local/range/range_manifest.json') --spot-json (Join-Path $repo 'local/range/range.json')
if ($LASTEXITCODE -ne 0) { throw 'Range manifest not usable (see the line above; docs/verification/COMBAT_RANGE.md says how to prepare it).' }
$itemDir = Split-Path -Parent $manifests.gear
$gearLevel = [int](Get-Content -LiteralPath $manifests.gear -Raw | ConvertFrom-Json).level
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$buildScript = Join-Path $Engine 'Engine/Build/BatchFiles/Build.bat'
if (!(Test-Path -LiteralPath $editor)) { throw "UnrealEditor.exe not found under $Engine" }
. (Join-Path $PSScriptRoot 'ue_lock.ps1')
. (Join-Path $PSScriptRoot 'movie_hud.ps1')
$outDir = Join-Path $repo 'local/slicegate'
New-Item -ItemType Directory -Force $outDir | Out-Null
$save = Join-Path $outDir 'save.json'
if (Test-Path -LiteralPath $save) { Remove-Item -LiteralPath $save -Force }   # a fresh mission every run
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$log = Join-Path $outDir "run-$stamp.log"

if (!$SkipBuild) {
    # At most two UE builds at once on this machine: wait while two UnrealBuildTool processes run (never touch them).
    while (@(Get-Process -Name UnrealBuildTool -ErrorAction SilentlyContinue).Count -ge 2) {
        Write-Host 'Two UnrealBuildTool processes are running; waiting 30 s.'
        Start-Sleep -Seconds 30
    }
    & $buildScript OpenWillowEditor Win64 Development "-Project=$project" -WaitMutex -NoHotReloadFromIDE "-MaxParallelActions=$MaxParallelActions"
    if ($LASTEXITCODE -ne 0) { throw "UE5 build failed: $LASTEXITCODE" }
}

$code = 2
$pages = $null
Enter-OwUeLock -Owner 'test_slice_gate' -TimeoutSeconds $WaitForEditorSeconds
try {
    # -Unforced buys the Phaselock point on the skills page and takes the mission on Marcus's mission page: both need the page server.
    if ($Unforced) { $pages = Start-OwMovieHud -Port $Port }
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya', '-owcanvashud',
        "-owmover=`"$($manifests.mover)`"", "-owslice=`"$($manifests.slice)`"", "-ownpcs=`"$($manifests.npcs)`"",
        "-owaudio=`"$($manifests.audio)`"", '-owquest', "-owquestsave=`"$save`"", "-owitems=`"$itemDir`"",
        "-owactionskill=`"$($manifests.actionskill)`"", "-owlevel=$gearLevel", '-owslicegate')
    if ($Gun) { $arguments += "-owslicegategun=$Gun" }
    if ($Shield) { $arguments += '-owslicegateshield' }
    if ($DownAfterFight) { $arguments += '-owslicegatedownafterfight' }
    # The two variants keep the no-start-shield baseline their fixtures were written against (SLICE_GATE.md). The plain run starts as normal play does.
    if ($Shield -or $DownAfterFight) { $arguments += '-ownostartshield' }
    if ($AudioPlay) { $arguments += "-owaudioplay=`"$AudioPlay`"" }
    if ($Unforced) { $arguments += @('-owslicegateunforced', "-owflashskills=$($pages.Url)skills.html", "-owflashmissionscreen=$($pages.Url)mission_screen.html") }
    $arguments += @('-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-unattended',
        '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"")
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Host "Launched slice gate pid $($process.Id); log: $log"
    if (!$process.WaitForExit($TimeoutSeconds * 1000)) {
        $commandLine = (Get-CimInstance Win32_Process -Filter "ProcessId=$($process.Id)" -ErrorAction SilentlyContinue).CommandLine
        if ($commandLine -and $commandLine.Contains($log)) {
            Stop-Process -Id $process.Id -Force
            Write-Host "Timed out after $TimeoutSeconds s; stopped pid $($process.Id) (this run's editor)."
        }
    }
    Start-Sleep -Seconds 3
    if (!(Test-Path -LiteralPath $log)) { throw "No log written: $log" }
    $shown = if ($Unforced) { 'OWSLICEGATE|ow\.RollGun|ow\.Range|OpenWillow Skills spend|OWQUEST mission page' } else { 'OWSLICEGATE|ow\.RollGun|ow\.Range' }
    Select-String -LiteralPath $log -Pattern $shown |
        ForEach-Object { Write-Host ($_.Line -replace '^.*LogTemp: (Display: |Warning: |Error: )?', '') }
    $checkArgs = @($log, '--json', (Join-Path $outDir "links-$stamp.json"), '--markdown', (Join-Path $outDir "links-$stamp.md"))
    if ($Unforced) { $checkArgs += '--expect-unforced' }
    & python (Join-Path $repo 'tools/check_slice_gate.py') @checkArgs
    $code = $LASTEXITCODE
    Write-Host "Log: $log"
} finally {
    Stop-OwMovieHud $pages
    Exit-OwUeLock
}
exit $code
