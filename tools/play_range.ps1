param(
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    [string]$Game = $env:OPENWILLOW_BL2,
    [string]$Manifest,
    # Starts the range in this mode: normal, passive (enemies stand and take shots), invulnerable or both. Default normal.
    [ValidateSet('normal', 'passive', 'invulnerable', 'both')][string]$Mode = 'normal',
    # Unattended check of the way in: waits this many seconds, stops the editor it started and reports whether the range built. 0 = play.
    [int]$CheckSeconds = 0,
    # Unattended: a rolled pistol and a rolled shotgun each kill a stock enemy with real shots (passive enemies, no input). Frames go to
    # local/range/shots/kill_<gun>_<a|b>.png, 1.4 s and 3 s after each death. Exit 0 when each gun killed one and its corpse was measured.
    [switch]$KillRow,
    # -MovieHud (test-only): BL2's HUD movie instead of the canvas HUD: starts tools/hud_overlay/serve.py on -MovieHudPort and passes -owflashhud=<url>; the run fails
    # if its log does not show the movie accepted (tools/movie_hud.ps1). Without it nothing changes.
    [switch]$MovieHud,
    [int]$MovieHudPort = 8796,
    # With -KillRow, once, to compare: the ragdoll without its depenetration limit and speed caps (-owcorpsenolimits). Frames: kill_nolimits_<gun>_<a|b>.png. A test only.
    [switch]$NoCorpseLimits,
    [int]$WaitForEditorSeconds = 3600,
    [switch]$Build
)
# AI-assisted (Claude). The one way into the gun range for a player: starts the host (Sanctuary_P, the only map the host has), and -owrangestart
# builds the range and moves Maya to it once she stands. No console commands to chain. Heavy: it opens the editor in -game, so on a hub round
# it needs a slot. Prepare first: python tools/prepare_range.py. Owns one editor at a time and the machine-wide UE run lock (tools/ue_lock.ps1).
# Host behaviour only: UNVERIFIED in game.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (!$Game) { throw 'Set OPENWILLOW_BL2 or pass -Game (the installed Borderlands 2 folder).' }
$env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
if (!$Manifest) { $Manifest = Join-Path $repo 'local/range/range_manifest.json' }
if (!(Test-Path -LiteralPath $Manifest)) { throw "No range manifest at $Manifest (run python tools/prepare_range.py)" }
& python (Join-Path $repo 'tools/check_range.py') --preflight $Manifest --spot-json (Join-Path $repo 'local/range/range.json')
if ($LASTEXITCODE -ne 0) { throw "Range manifest not usable (see the line above): $Manifest" }
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
if (!(Test-Path -LiteralPath $editor)) { throw "UnrealEditor.exe not found under $Engine" }
. (Join-Path $PSScriptRoot 'ue_lock.ps1')
. (Join-Path $PSScriptRoot 'movie_hud.ps1')
$code = 0
$movie = $null
Enter-OwUeLock -Owner 'play_range' -TimeoutSeconds $WaitForEditorSeconds
try {
    if ($Build) {
        & (Join-Path $Engine 'Engine/Build/BatchFiles/Build.bat') OpenWillowEditor Win64 Development "-Project=$project" -WaitMutex -NoHotReloadFromIDE
        if ($LASTEXITCODE -ne 0) { throw "UE5 build failed: $LASTEXITCODE" }
    }
    $logDir = Join-Path $repo 'local/range'
    New-Item -ItemType Directory -Force $logDir | Out-Null
    $log = Join-Path $logDir ('play-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')
    $hudSwitch = '-owcanvashud'
    if ($MovieHud) { $movie = Start-OwMovieHud -Port $MovieHudPort; $hudSwitch = "-owflashhud=$($movie.Url)" }
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya', $hudSwitch, '-owrangestart',
        "-owrangemode=$Mode", "-owrange=`"$Manifest`"", '-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash',
        '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"")
    # The Fire mission's stock dummy (as tools/test_range.ps1); without these manifests the host stands engine-shape dummies and says so.
    $slice = @{ owslice = 'local/slice/world.json'; ownpcs = 'local/slice/npc_assets.json'; owaudio = 'local/slice/audio.json' }
    if (@($slice.Values | Where-Object { !(Test-Path -LiteralPath (Join-Path $repo $_)) }).Count -eq 0) {
        foreach ($key in $slice.Keys) { $arguments += "-$key=`"$(Join-Path $repo $slice[$key])`"" }
    }
    # Level 8, as the playtest that saw the flung corpses: the guns roll at that stage, and Maya can use them.
    if ($NoCorpseLimits -and !$KillRow) { throw '-NoCorpseLimits is only for -KillRow' }
    if ($NoCorpseLimits) { $arguments += '-owcorpsenolimits' }
    if ($KillRow) { $arguments += '-owrangekillrow', '-owlevel=8'; $Mode = 'passive'; $arguments = $arguments -replace '^-owrangemode=.*$', '-owrangemode=passive'; if ($CheckSeconds -le 0) { $CheckSeconds = if ($MovieHud) { 305 } else { 180 } } }
    if ($CheckSeconds -gt 0) { $arguments += '-unattended' }
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Host "Range started: pid $($process.Id); log: $log"
    if ($CheckSeconds -gt 0) {
        if (!$process.WaitForExit($CheckSeconds * 1000)) {
            # Stop only the editor this run started, and only when its command line names this run's log.
            $commandLine = (Get-CimInstance Win32_Process -Filter "ProcessId=$($process.Id)" -ErrorAction SilentlyContinue).CommandLine
            if ($commandLine -and $commandLine.Contains($log)) { Stop-Process -Id $process.Id -Force }
        }
        Start-Sleep -Seconds 3
        if ($KillRow) {
            Select-String -LiteralPath $log -Pattern 'OWRANGEKILL' | ForEach-Object { Write-Host ($_.Line -replace '^.*LogTemp: (Display: |Warning: |Error: )?', '') }
            & python (Join-Path $repo 'tools/check_range.py') $log --kill-log
            if ($LASTEXITCODE -ne 0) { $code = 1 }
        }
        $built = Select-String -LiteralPath $log -Pattern 'OWRANGE start switch: built=1' -Quiet
        Write-Host $(if ($built) { 'Range check: the switch built the range.' } else { 'Range check: FAIL, no "built=1" line in the log.' })
        if (!$built) { $code = 1 }
        if ($MovieHud -and !(Test-OwMovieHudLog -Log $log -Driver $(if ($KillRow) { 'range' } else { '' }) -RequireShot:$KillRow)) { $code = 1 }
    } else {
        $process.WaitForExit()
    }
} finally {
    Stop-OwMovieHud $movie
    Exit-OwUeLock
}
exit $code
