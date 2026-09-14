param(
    [Parameter(Mandatory=$true)][string]$Engine,
    [Parameter(Mandatory=$true)][string]$Game,
    [Parameter(Mandatory=$true)][string]$Scene,
    [switch]$Walk,
    [switch]$Selector,
    [switch]$Profile,
    [switch]$LowEnd
)
$ErrorActionPreference = 'Stop'
if (@($Walk, $Selector, $Profile | Where-Object { $_ }).Count -gt 1) { throw 'Choose one of -Walk, -Selector or -Profile' }
$repo = Split-Path -Parent $PSScriptRoot
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
if (!(Test-Path -LiteralPath $editor)) { throw 'UnrealEditor.exe not found' }
if (!(Test-Path -LiteralPath (Join-Path $Game 'Binaries/Win32/Borderlands2.exe'))) { throw 'BL2 installation not found' }
$root = (Resolve-Path -LiteralPath $Scene).Path
$manifest = Get-Content -LiteralPath (Join-Path $root 'scene.json') -Raw | ConvertFrom-Json
if ($Walk -and $manifest.collision_policy -ne 'observed_convex_and_box_v1') { throw 'Prepare and import collision before testing walking' }
if ($manifest.map -notmatch '^[A-Za-z0-9_]+_P$') { throw 'Invalid persistent map name' }
$mapFile = Join-Path (Split-Path $project) "Content/OpenWillow/$($manifest.map)/$($manifest.map).umap"
if (!(Test-Path -LiteralPath $mapFile)) { throw 'Import this map before testing its viewer' }
$log = Join-Path $root ('viewer-' + [guid]::NewGuid().ToString('N') + '.log')
$env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
$testName = if ($Walk) { 'Walking' } elseif ($Selector) { 'MapSelector' } elseif ($Profile) { 'Profile' } else { 'Viewer' }
# -LowEnd mirrors run_ue_level.ps1's runtime-only DX11/SM5 path so the
# profile test can sample both render routes. Keep the two lists identical.
$resolution = @('-ResX=1280', '-ResY=720')
$execCmds = "Automation RunTests OpenWillow.$testName"
$lowEndArgs = @()
if ($LowEnd) {
    $resolution = @('-ResX=960', '-ResY=540')
    $execCmds = (@(
        'sg.ShadowQuality 0', 'sg.PostProcessQuality 0', 'sg.AntiAliasingQuality 0',
        'sg.EffectsQuality 0', 'sg.GlobalIlluminationQuality 0', 'sg.ReflectionQuality 0',
        'sg.ShadingQuality 0', 'sg.TextureQuality 1', 'sg.FoliageQuality 0',
        'sg.ViewDistanceQuality 2', 'r.ScreenPercentage 66', 'r.AntiAliasingMethod 1',
        'r.Shadow.Virtual.Enable 0', 'r.DynamicRes.OperationMode 0', 't.MaxFPS 60'
    ) + $execCmds) -join ','
    $lowEndArgs = @('-dx11')
}
$argsList = @("`"$project`"", "/Game/OpenWillow/$($manifest.map)/$($manifest.map)",
    '-game', '-windowed') + $resolution + @('-unattended', '-nosplash',
    '-ini:Engine:[/Script/Engine.AutomationTestSettings]:DefaultInteractiveFramerate=1',
    "-ExecCmds=`"$execCmds`"",
    '-TestExit="Automation Test Queue Empty"', "-abslog=`"$log`"") + $lowEndArgs
# Only this temporary game process belongs to the harness; never close an editor.
if ($Walk) { $argsList += '-owwalk' }
$process = Start-Process -FilePath $editor -ArgumentList $argsList -WindowStyle Hidden -PassThru
$deadline = (Get-Date).AddMinutes(8)
try {
    while (!$process.WaitForExit(1000)) {
        if ((Get-Date) -gt $deadline) { throw "Viewer test timed out; see $log" }
    }
    $result = Select-String -LiteralPath $log -SimpleMatch "Test Completed. Result={Success} Name={$testName}"
    if (!$result -or $process.ExitCode -ne 0) { throw "Viewer test failed; see $log" }
    Select-String -LiteralPath $log -Pattern 'Pawn displacement:|Viewer diagnostic:|Requested screenshot:|Map selector|Profile (start|turned) view|Test Completed.' | ForEach-Object { $_.Line }
    Write-Output "Log: $log"
} finally {
    if (!$process.HasExited) { Stop-Process -Id $process.Id }
}
