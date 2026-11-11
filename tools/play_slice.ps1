param(
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    [string]$Game = $env:OPENWILLOW_BL2,
    # The hand-play save (the mission's state and Maya's progression). -Fresh deletes it first. The smoke run never uses it.
    [string]$Save,
    [switch]$Fresh,
    # Maya's level. 0: the slice gear level from local/items/slice/slice_manifest.json (an UNVERIFIED slice choice), as tools/run_quest.ps1 does.
    [int]$Level = 0,
    # The page server's port (tools/hud_overlay/serve.py). A server of this checkout's pages already on it is reused; another checkout's is refused.
    [int]$Port = 8798,
    # Unattended smoke run: plays this many seconds with a fresh save, stops the editor it started, then grades the log (one PASS/FAIL line per item).
    [int]$CheckSeconds = 0,
    # Prints the editor command line it would start and exits, with no lock, no server and no editor (tests/play_slice_test.py).
    [switch]$PrintCommandLine,
    # Grades an existing log the way -CheckSeconds does and exits; needs no local data and starts nothing.
    [string]$GradeLog,
    [int]$WaitForEditorSeconds = 3600
)
# AI-assisted (Claude). The vertical slice by hand, with no test driver: Sanctuary_P with Marcus's Fire mission (-owquest and its manifests, as
# tools/run_quest.ps1), the gun range's enemies (-owrangestart, as tools/play_range.ps1: it builds the range and moves Maya to it once she
# stands), Phaselock's data (-owactionskill), BL2's pages served by tools/hud_overlay/serve.py (the HUD movie, inventory, skills; the missions
# log, map and mission screen follow from the skills URL in the host), the sounds (-owaudioplay) and the start shield (the host's default
# when local/slice/stand_in_shield.json exists). Nothing on the line drives the game, grants a skill point, forces loot or deals damage.
# Refuses to start, naming the file and the tool that makes it, when local data it needs is missing. Holds the machine-wide UE run lock
# (tools/ue_lock.ps1) while the editor runs, and stops only the editor and the page server it started. Host behaviour only: UNVERIFIED in game.
# Exit codes: 0 played / smoke passed, 1 a smoke item failed, 2 refused (missing data).
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'movie_hud.ps1')   # Start-OwMovieHud, Stop-OwMovieHud, Test-OwMovieHudLog; it dot-sources run_limit.ps1

# What the smoke run requires of the log. Each pattern is a line the host writes (file named), each "bad" one a line that says the piece failed.
$script:OwPlaySliceItems = @(
    @{ Name = 'mission'; Good = 'OWQUEST READY mission='; Bad = 'OWQUEST ERROR'; Where = 'OpenWillowQuest.cpp' }
    @{ Name = 'range'; Good = 'OWRANGE start switch: built=1'; Bad = 'OWRANGE start switch: built=0'; Where = 'OpenWillowRange.cpp' }
    @{ Name = 'phaselock_data'; Good = 'OpenWillow Phaselock data: '; Bad = 'OpenWillow Phaselock unavailable'; Where = 'OpenWillowWalker.cpp' }
    @{ Name = 'audio_map'; Good = 'OWAUDIO role=\S+ files=[1-9]'; Bad = 'OWAUDIO play map (refused|unreadable)'; Where = 'OpenWillowAudio.cpp' }
    @{ Name = 'start_shield'; Good = 'OpenWillow Maya shield [^:\r\n]+: capacity [1-9]'; Bad = 'OpenWillow Maya starts with no (start )?shield'; Where = 'OpenWillowWalker.cpp' }
    # The status pages report through the host's console bridge (OpenWillowMayaHUD.cpp logs every console line); the text is the page's own.
    @{ Name = 'page_inventory'; Good = 'OpenWillow Inventory movie ready'; Bad = $null; Where = 'inventory.js' }
    @{ Name = 'page_skills'; Good = 'OpenWillow Skills movie ready'; Bad = $null; Where = 'skills.js' }
    @{ Name = 'page_missions'; Good = 'OWINVTIME js_missions_ready'; Bad = $null; Where = 'missions.js' }
    @{ Name = 'page_mission_screen'; Good = 'OpenWillow mission page console: OWMISSIONREADY'; Bad = 'OpenWillow mission page failed to load'; Where = 'mission_screen.js' }
)

# One PASS/FAIL line per item, then the HUD movie's own verdict (movie_hud.ps1). True when every item passed.
function Test-OwPlaySliceLog {
    param([Parameter(Mandatory)][string]$Log)
    $text = if (Test-Path -LiteralPath $Log) { "$(Get-Content -LiteralPath $Log -Raw)" } else { '' }
    $ok = $true
    foreach ($item in $script:OwPlaySliceItems) {
        $bad = if ($item.Bad) { [regex]::Match($text, "$($item.Bad)[^\r\n]*") } else { $null }
        $why = if ($bad -and $bad.Success) { $bad.Value } elseif ($text -notmatch $item.Good) { "no ""$($item.Good)"" line ($($item.Where))" } else { '' }
        if ($why) { Write-Host "FAIL $($item.Name): $why"; $ok = $false } else { Write-Host "PASS $($item.Name)" }
    }
    # The HUD movie: accepted by the host and never dropped for the canvas (Test-OwMovieHudLog prints its reason).
    if (Test-OwMovieHudLog -Log $Log) { Write-Host 'PASS page_hud' } else { Write-Host 'FAIL page_hud: see the MOVIE HUD line above'; $ok = $false }
    return $ok
}

if ($GradeLog) {
    if (!(Test-Path -LiteralPath $GradeLog)) { Write-Host "No log at $GradeLog"; exit 1 }
    exit $(if (Test-OwPlaySliceLog -Log $GradeLog) { 0 } else { 1 })
}

# Local data, each with the tool that writes it. All are checked before anything starts, and every missing one is named.
$data = [ordered]@{
    mover = @('local/doors/mover.json', 'python tools/prepare_mover.py')
    slice = @('local/slice/world.json', 'python tools/prepare_slice_world.py')
    npcs = @('local/slice/npc_assets.json', 'powershell -File tools/seed_slice_npc_assets.ps1')
    audio = @('local/slice/audio.json', 'python tools/audio_slice_chain.py')
    gear = @('local/items/slice/slice_manifest.json', 'python tools/weapon_slice_gear.py')
    actionskill = @('local/character/action_skill_siren.json', 'python tools/prepare_action_skill.py (Phaselock data)')
    audioplay = @('local/slice/audio_play.json', 'python tools/prepare_slice_audio.py (the audio play map)')
    range = @('local/range/range_manifest.json', 'python tools/prepare_range.py')
    shield = @('local/slice/stand_in_shield.json', 'written by hand: docs/verification/COMBAT_DOWN_RESPAWN.md, "Stand-in shield capacity"')
}
$refusals = @()
$paths = @{}
foreach ($key in $data.Keys) {
    $paths[$key] = Join-Path $repo $data[$key][0]
    if (!(Test-Path -LiteralPath $paths[$key])) { $refusals += "missing $($paths[$key]) (make it with: $($data[$key][1]))" }
}
if (!$Game -or !(Test-Path -LiteralPath $Game)) { $refusals += 'no Borderlands 2 folder: set OPENWILLOW_BL2 or pass -Game' }
# The range's own preflight (as play_range.ps1 and test_slice_gate.ps1): an old or failed prepare_range.py manifest is refused here.
if (Test-Path -LiteralPath $paths.range) {
    $said = & python (Join-Path $repo 'tools/check_range.py') --preflight $paths.range --spot-json (Join-Path $repo 'local/range/range.json') 2>&1
    if ($LASTEXITCODE -ne 0) { $refusals += "range manifest not usable: $(@($said) -join ' ') (re-run python tools/prepare_range.py)" }
}
# The converted movies the HUD and the status pages load (tools/check_overlay_data.py names each missing file and its tool). The mission
# screen's movie is optional there: without it the page draws a plain panel, and the smoke still needs its ready line.
$said = & python (Join-Path $repo 'tools/check_overlay_data.py') --fail-on HUD --fail-on Skills 2>&1
if ($LASTEXITCODE -ne 0) { $refusals += "overlay data missing: $(@($said | Where-Object { "$_" -match 'MISSING' }) -join '; ')" }
else { @($said | Where-Object { "$_" -match 'MISSING' }) | ForEach-Object { Write-Host "Note: $_" } }
if ($refusals) {
    foreach ($line in $refusals) { Write-Host "PLAY SLICE REFUSED: $line" }
    exit 2
}

$itemDir = Split-Path -Parent $paths.gear
if ($Level -le 0) {
    $Level = [int](Get-Content -LiteralPath $paths.gear -Raw | ConvertFrom-Json).level
    Write-Host "Maya level $Level = slice gear level (slice_manifest.json; UNVERIFIED slice choice, not a save)"
}
$outDir = Join-Path $repo 'local/playslice'
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
if ($CheckSeconds -gt 0) {
    # A fresh mission every smoke run, in its own file: the hand-play save is never touched by it.
    $Save = Join-Path $outDir 'smoke-save.json'
    $log = Join-Path $outDir "smoke-$stamp.log"
} else {
    if (!$Save) { $Save = Join-Path $outDir 'save.json' }
    $log = Join-Path $outDir "play-$stamp.log"
}
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$url = "http://127.0.0.1:$Port/"
$arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya',
    "-owmover=`"$($paths.mover)`"", "-owslice=`"$($paths.slice)`"", "-ownpcs=`"$($paths.npcs)`"", "-owaudio=`"$($paths.audio)`"",
    '-owquest', "-owquestsave=`"$Save`"", "-owitems=`"$itemDir`"", "-owactionskill=`"$($paths.actionskill)`"", "-owlevel=$Level",
    '-owrangestart', '-owrangemode=normal', "-owrange=`"$($paths.range)`"", "-owaudioplay=`"$($paths.audioplay)`"",
    "-owflashhud=$url", "-owflashskills=${url}skills.html", "-owflashinventory=${url}inventory.html",
    '-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"")
if ($CheckSeconds -gt 0) { $arguments += '-unattended' }   # no modal dialogs in an unattended run; changes nothing in the game

if ($PrintCommandLine) {
    Write-Host "PLAY SLICE COMMAND LINE ($(if ($CheckSeconds -gt 0) { 'smoke' } else { 'play' })):"
    foreach ($argument in $arguments) { Write-Host "  $argument" }
    exit 0
}

$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
if (!(Test-Path -LiteralPath $editor)) { Write-Host "PLAY SLICE REFUSED: UnrealEditor.exe not found under $Engine (pass -Engine or set OPENWILLOW_UE)"; exit 2 }
. (Join-Path $PSScriptRoot 'ue_lock.ps1')
New-Item -ItemType Directory -Force $outDir | Out-Null
if (($Fresh -or $CheckSeconds -gt 0) -and (Test-Path -LiteralPath $Save)) { Remove-Item -LiteralPath $Save -Force }
$code = 0
$movie = $null
Enter-OwUeLock -Owner 'play_slice' -TimeoutSeconds $WaitForEditorSeconds
try {
    $env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
    $movie = Start-OwMovieHud -Port $Port
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    [void]$process.Handle   # open now: stopping it later goes through this object (run_limit.ps1)
    Write-Host "Slice started: pid $($process.Id); log: $log; save: $Save$(if ($CheckSeconds -le 0) { ' (close the window to finish)' })"
    if ($CheckSeconds -gt 0) {
        if (!(Wait-OwStartedProcess -Process $process -LimitSeconds $CheckSeconds)) {
            # Only the editor this run started, and only while its command line still names this run's log.
            if (!(Stop-OwStartedProcess -Process $process -Expected @($log))) { Write-Host "Could not stop pid $($process.Id); it is left running." }
        }
        Start-Sleep -Seconds 3
        $code = if (Test-OwPlaySliceLog -Log $log) { 0 } else { 1 }
        Write-Host "Smoke $(if ($code -eq 0) { 'PASS' } else { 'FAIL' }): $log"
    } else {
        [void](Wait-OwStartedProcess -Process $process)
        Select-String -LiteralPath $log -Pattern 'OWQUEST READY|OWRANGE start switch|Phaselock data|Phaselock unavailable|OWAUDIO play map|HUD path|Maya level|starts with no' -ErrorAction SilentlyContinue |
            ForEach-Object { $_.Line -replace '^.*LogTemp: (Display: |Warning: |Error: )?', '' }
    }
} finally {
    Stop-OwMovieHud $movie
    Exit-OwUeLock
}
exit $code
