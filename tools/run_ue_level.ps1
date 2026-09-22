param(
    [Parameter(Mandatory=$true)][string]$Engine,
    [Parameter(Mandatory=$true)][string]$Game,
    [string]$Scene,
    [switch]$ImportOnly,
    [switch]$ViewOnly,
    [switch]$SkipBuild,
    [switch]$Walk,
    [switch]$LowEnd
)
$ErrorActionPreference = 'Stop'
if ($ImportOnly -and $ViewOnly) { throw 'Choose either -ImportOnly or -ViewOnly' }
if ($Walk -and !$ViewOnly) { throw '-Walk requires -ViewOnly' }
$repo = Split-Path -Parent $PSScriptRoot
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$buildScript = Join-Path $Engine 'Engine/Build/BatchFiles/Build.bat'
if (!(Test-Path -LiteralPath $editor)) { throw "UnrealEditor.exe not found under $Engine" }
if (!(Test-Path -LiteralPath (Join-Path $Game 'Binaries/Win32/Borderlands2.exe'))) { throw 'BL2 installation not found' }
if (!$Scene) { $Scene = Join-Path $repo 'local/ash' }
if (!(Test-Path -LiteralPath (Join-Path $Scene 'scene.json'))) { throw 'Run tools/prepare_level.py first' }
$env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
$env:OPENWILLOW_SCENE = (Resolve-Path -LiteralPath $Scene).Path
$savedMap = $null
if ($ViewOnly) {
    $manifest = Get-Content -LiteralPath (Join-Path $Scene 'scene.json') -Raw | ConvertFrom-Json
    if ($Walk -and $manifest.collision_policy -ne 'observed_convex_and_box_v1') { throw 'Prepare and import collision before walking' }
    if ($manifest.map -notmatch '^[A-Za-z0-9_]+_P$') { throw 'Invalid persistent map name' }
    $savedMap = "/Game/OpenWillow/$($manifest.map)/$($manifest.map)"
    $mapFile = Join-Path (Split-Path $project) "Content/OpenWillow/$($manifest.map)/$($manifest.map).umap"
    if (!(Test-Path -LiteralPath $mapFile)) { throw 'Import this scene before using -ViewOnly' }
}
if (!$SkipBuild) {
    $projectKey = $project.Replace('\', '/').ToLowerInvariant()
    $running = Get-CimInstance Win32_Process -Filter "Name='UnrealEditor.exe'" |
        Where-Object { $_.CommandLine -and $_.CommandLine.Replace('\', '/').ToLowerInvariant().Contains($projectKey) }
    if ($running) { throw 'This project is open. Close its editor before rebuilding, or use -SkipBuild with the existing binary.' }
    # An editor for another checkout uses the same engine Live Coding mutex,
    # but none of this project's output files. Build this isolated project.
    & $buildScript OpenWillowEditor Win64 Development "-Project=$project" -WaitMutex -NoHotReloadFromIDE
    if ($LASTEXITCODE -ne 0) { throw "UE5 build failed: $LASTEXITCODE" }
}
$script = Join-Path $repo 'host/ue5/import_level.py'
# -LowEnd targets machines without a discrete GPU. It only changes runtime
# rendering: DX11/SM5 skips Nanite and virtual shadow maps entirely, the
# scalability groups drop to their lowest levels, and the frame renders at a
# reduced resolution. Imported content and saved scenes are unaffected.
$viewArgs = @('-windowed', '-ResX=1280', '-ResY=720')
$lowEndArgs = @()
if ($LowEnd) {
    $viewArgs = @('-windowed', '-ResX=960', '-ResY=540')
    $lowEndCmds = @(
        'sg.ShadowQuality 0', 'sg.PostProcessQuality 0', 'sg.AntiAliasingQuality 0',
        'sg.EffectsQuality 0', 'sg.GlobalIlluminationQuality 0', 'sg.ReflectionQuality 0',
        'sg.ShadingQuality 0', 'sg.TextureQuality 1', 'sg.FoliageQuality 0',
        'sg.ViewDistanceQuality 2', 'r.ScreenPercentage 66', 'r.AntiAliasingMethod 1',
        'r.Shadow.Virtual.Enable 0', 'r.DynamicRes.OperationMode 0', 't.MaxFPS 60'
    ) -join ','
    $lowEndArgs = @('-dx11', "-ExecCmds=$lowEndCmds")
}
if ($ViewOnly) {
    $walkArgs = @()
    if ($Walk) { $walkArgs += '-owwalk' }
    & $editor $project $savedMap -game @viewArgs @lowEndArgs -log @walkArgs
} elseif ($ImportOnly) {
    $commandlet = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor-Cmd.exe'
    $importLog = Join-Path $env:OPENWILLOW_SCENE 'ue-import.log'
    & $commandlet $project -run=pythonscript "-script=$script" -unattended -nullrhi -nosplash "-abslog=$importLog"
    if ($LASTEXITCODE -ne 0) { throw "Scene import failed: $LASTEXITCODE" }
    # A material whose shader fails to compile silently renders as the default
    # checkerboard; the saved-scene verifier only inspects the graph, so gate here.
    $compileFailures = @(Select-String -LiteralPath $importLog -Pattern 'Failed to compile Material' -SimpleMatch)
    if ($compileFailures.Count -gt 0) {
        $compileFailures | ForEach-Object { Write-Host $_.Line }
        throw "$($compileFailures.Count) material(s) failed to compile during import; see $importLog"
    }
    $verify = Join-Path $repo 'host/ue5/verify_level.py'
    & $commandlet $project -run=pythonscript "-script=$verify" -unattended -nullrhi -nosplash
    if ($LASTEXITCODE -ne 0) { throw "Saved scene verification failed: $LASTEXITCODE" }
    $manifest = Get-Content -LiteralPath (Join-Path $Scene 'scene.json') -Raw | ConvertFrom-Json
    if ($manifest.collision_policy -eq 'observed_convex_and_box_v1') {
        $verifyCollision = Join-Path $repo 'host/ue5/verify_collision.py'
        & $commandlet $project -run=pythonscript "-script=$verifyCollision" -unattended -nullrhi -nosplash
        if ($LASTEXITCODE -ne 0) { throw "Saved collision verification failed: $LASTEXITCODE" }
    }
    $verifyUv = Join-Path $repo 'host/ue5/verify_uv.py'
    & $commandlet $project -run=pythonscript "-script=$verifyUv" -unattended -nullrhi -nosplash
    if ($LASTEXITCODE -ne 0) { throw "Saved UV verification failed: $LASTEXITCODE" }
} else {
    & $editor $project "-ExecutePythonScript=$script" @lowEndArgs -log
}
