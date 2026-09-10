param(
    [Parameter(Mandatory=$true)][string]$Engine,
    [Parameter(Mandatory=$true)][string]$Game,
    [string]$Probe
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$buildScript = Join-Path $Engine 'Engine/Build/BatchFiles/Build.bat'
if (!(Test-Path -LiteralPath $editor)) { throw "UnrealEditor.exe not found under $Engine" }
if (!(Test-Path -LiteralPath (Join-Path $Game 'Binaries/Win32/Borderlands2.exe'))) { throw 'BL2 installation not found' }
if (!$Probe) { $Probe = Join-Path $repo 'local/probe' }
if (!(Test-Path -LiteralPath (Join-Path $Probe 'probe.json'))) { throw 'Run tools/prepare_probe.py first' }
$env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
$env:OPENWILLOW_PROBE = (Resolve-Path -LiteralPath $Probe).Path
& $buildScript OpenWillowEditor Win64 Development "-Project=$project" -WaitMutex
if ($LASTEXITCODE -ne 0) { throw "UE5 build failed: $LASTEXITCODE" }
$script = Join-Path $repo 'host/ue5/import_probe.py'
# The editor is an interactive tool for the user to inspect the Phase 0 result.
& $editor $project "-ExecutePythonScript=$script" -log
