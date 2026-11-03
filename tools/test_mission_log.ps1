param(
    # Unreal Engine install (the folder that contains Engine/).
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    [string]$Game = $env:OPENWILLOW_BL2,
    [int]$Port = 8785,
    [int]$TimeoutSeconds = 360,
    # How long to wait for another run's local/ue_run.lock (or a running editor) before giving up.
    [int]$LockWaitSeconds = 900,
    [string[]]$Extra = @('-ddc=InstalledNoZenLocalFallback', '-d3d11')
)
# In-engine check of the Missions log page (tools/hud_overlay/missions.html) with the -owquest slice mission:
# -owmissionlogtest (AOpenWillowMayaHUD::TickMissionLogTest) opens the page, checks it lists nothing before the mission is
# accepted and the mission under IN PROGRESS after Accept(), Down on the only mission, the header tabs to Inventory and
# back, and Escape. Prints each OWMISSIONTEST row and the summary; screenshots are copied to local/menus/missionlog/.
# Needs the slice manifests test_quest.ps1 uses and local/ui/run/missionlog.json (tools/prepare_mission_log.py).
# Exit codes: 0 all PASS; 1 a FAIL; 2 no summary (launch/timeout). Owns one editor and the shared lock; never touches
# another editor; stops only the editor and server it started.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
if (!(Test-Path -LiteralPath $editor)) { throw "UnrealEditor.exe not found under $Engine" }
if (!$Game -or !(Test-Path -LiteralPath $Game)) { throw 'Pass -Game <Borderlands 2 folder> or set OPENWILLOW_BL2' }
$manifests = [ordered]@{ mover = 'local/doors/mover.json'; slice = 'local/slice/world.json'; npcs = 'local/slice/npc_assets.json'
    audio = 'local/slice/audio.json'; gear = 'local/items/slice/slice_manifest.json'; actionskill = 'local/character/action_skill_siren.json'
    missionlog = 'local/ui/run/missionlog.json' }
foreach ($file in $manifests.Values) {
    if (!(Test-Path -LiteralPath (Join-Path $repo $file))) { throw "Missing $file (see tools/test_quest.ps1 and tools/prepare_mission_log.py)" }
}
$path = { param($key) (Resolve-Path -LiteralPath (Join-Path $repo $manifests[$key])).Path }
$gear = & $path 'gear'
$gearLevel = [int](Get-Content -LiteralPath $gear -Raw | ConvertFrom-Json).level
$outDir = Join-Path $repo 'local/menus/missionlog'
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$log = Join-Path $outDir ('run-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')
$save = Join-Path $outDir 'save.json'
if (Test-Path -LiteralPath $save) { Remove-Item -LiteralPath $save -Force }
$shots = Join-Path $repo 'host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor'
$lockPath = Join-Path $repo 'local/ue_run.lock'

# --- lock: wait until no editor runs and nobody holds it (CreateNew is the atomic claim) ---
$deadline = (Get-Date).AddSeconds($LockWaitSeconds)
$stream = $null
while (!$stream) {
    if (!(Get-Process UnrealEditor -ErrorAction SilentlyContinue)) {
        try { $stream = [System.IO.File]::Open($lockPath, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::Read) } catch { $stream = $null }
    }
    if (!$stream) {
        if ((Get-Date) -gt $deadline) { throw 'Another Unreal editor or local/ue_run.lock is still present; leave it untouched.' }
        Start-Sleep -Seconds 5
    }
}
$bytes = [System.Text.Encoding]::UTF8.GetBytes("test_mission_log $((Get-Date).ToString('o'))")
$stream.Write($bytes, 0, $bytes.Length); $stream.Dispose()

$exitCode = 2
$process = $null
$server = $null
try {
    $url = "http://127.0.0.1:$Port/"
    $pages = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot 'hud_overlay')).Path
    try { $running = Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 $url } catch { $running = $null }
    if ($running) {
        if ("$($running.Headers['X-OpenWillow-Pages'])" -ne $pages) { throw "Port $Port serves other pages; stop that server or pass -Port." }
    } else {
        $server = Start-Process -WindowStyle Hidden -PassThru -FilePath python -ArgumentList @(
            "`"$(Join-Path $PSScriptRoot 'hud_overlay/serve.py')`"", '--port', $Port)
        Start-Sleep 2
        Invoke-WebRequest -UseBasicParsing -TimeoutSec 5 $url | Out-Null
    }
    $env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
    $env:OPENWILLOW_SCENE = Join-Path $repo 'local/sanctuary'
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya', '-owcanvashud',
        "-owmover=`"$(& $path 'mover')`"", "-owslice=`"$(& $path 'slice')`"", "-ownpcs=`"$(& $path 'npcs')`"",
        "-owaudio=`"$(& $path 'audio')`"", '-owquest', "-owquestsave=`"$save`"", "-owitems=`"$(Split-Path -Parent $gear)`"",
        "-owactionskill=`"$(& $path 'actionskill')`"", "-owlevel=$gearLevel",
        "-owflashskills=${url}skills.html", "-owflashinventory=${url}inventory.html", '-owmissionlogtest',
        '-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-unattended', "-abslog=`"$log`"") + $Extra
    $started = Get-Date
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Output "Launched UnrealEditor pid $($process.Id); log: $log"
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $summary = $null
    while (!$process.HasExited -and (Get-Date) -lt $deadline) {
        Start-Sleep -Seconds 2
        if (Test-Path -LiteralPath $log) {
            $text = Get-Content -LiteralPath $log -Raw -ErrorAction SilentlyContinue
            $m = [regex]::Match("$text", 'OWMISSIONTEST SUMMARY (.*)')
            if ($m.Success) { $summary = $m.Groups[1].Value.Trim(); break }
        }
    }
    if ($summary) { [void]$process.WaitForExit(20000) }
    $text = if (Test-Path -LiteralPath $log) { Get-Content -LiteralPath $log -Raw } else { '' }
    foreach ($m in [regex]::Matches($text, 'OWMISSIONTEST (\S+) (PASS|FAIL) (.*)')) {
        Write-Output ('{0,-5} {1,-14} {2}' -f $m.Groups[2].Value, $m.Groups[1].Value, $m.Groups[3].Value.Trim())
    }
    if (Test-Path -LiteralPath $shots) {
        Get-ChildItem -LiteralPath $shots -Filter 'OWMission_*.png' | Where-Object { $_.LastWriteTime -ge $started } |
            ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $outDir -Force }
    }
    if ($summary) {
        Write-Output "OWMISSIONTEST SUMMARY $summary"
        $exitCode = if ($summary -match 'failed=0') { 0 } else { 1 }
    } else {
        Write-Output "OWMISSIONTEST SUMMARY result=FAIL reason=$(if ($process.HasExited) { "editor exited ($($process.ExitCode))" } else { "no summary within $TimeoutSeconds s" })"
    }
    Write-Output "Log: $log"
} finally {
    if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    if ($server -and !$server.HasExited) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item -LiteralPath $lockPath -Force -ErrorAction SilentlyContinue
}
exit $exitCode
