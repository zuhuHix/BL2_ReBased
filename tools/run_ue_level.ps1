param(
    [Parameter(Mandatory=$true)][string]$Engine,
    [Parameter(Mandatory=$true)][string]$Game,
    [string]$Scene,
    [switch]$ImportOnly,
    [switch]$ViewOnly,
    [switch]$SkipBuild,
    [switch]$Walk
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
if ($ViewOnly) {
    $walkArgs = @()
    if ($Walk) { $walkArgs += '-owwalk' }
    & $editor $project $savedMap -game -windowed -ResX=1280 -ResY=720 -log @walkArgs
} elseif ($ImportOnly) {
    $commandlet = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor-Cmd.exe'
    & $commandlet $project -run=pythonscript "-script=$script" -unattended -nullrhi -nosplash
    if ($LASTEXITCODE -ne 0) { throw "Scene import failed: $LASTEXITCODE" }
    $verify = Join-Path $repo 'host/ue5/verify_level.py'
    & $commandlet $project -run=pythonscript "-script=$verify" -unattended -nullrhi -nosplash
    if ($LASTEXITCODE -ne 0) { throw "Saved scene verification failed: $LASTEXITCODE" }
    $manifest = Get-Content -LiteralPath (Join-Path $Scene 'scene.json') -Raw | ConvertFrom-Json
    if ($manifest.collision_policy -eq 'observed_convex_and_box_v1') {
        $verifyCollision = Join-Path $repo 'host/ue5/verify_collision.py'
        & $commandlet $project -run=pythonscript "-script=$verifyCollision" -unattended -nullrhi -nosplash
        if ($LASTEXITCODE -ne 0) { throw "Saved collision verification failed: $LASTEXITCODE" }
    }
} else {
    & $editor $project "-ExecutePythonScript=$script" -log
}
