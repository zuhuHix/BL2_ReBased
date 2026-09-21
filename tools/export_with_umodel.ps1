param(
    [Parameter(Mandatory = $true)][string]$UModel,
    [Parameter(Mandatory = $true)][string]$Cooked,
    [Parameter(Mandatory = $true)][string]$Package,
    [string]$Object,
    [ValidateSet('gltf', 'psk')][string]$MeshFormat = 'gltf',
    [switch]$Sounds
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$umodelPath = (Resolve-Path -LiteralPath $UModel).Path
$cookedPath = (Resolve-Path -LiteralPath $Cooked).Path
if ([IO.Path]::GetFileName($umodelPath) -notmatch '^umodel(\.exe)?$') {
    throw 'UModel must point to umodel.exe'
}
if (!(Test-Path -LiteralPath (Join-Path $cookedPath 'Ash_P.upk'))) {
    throw 'Cooked path does not look like Borderlands 2 CookedPCConsole'
}

$stamp = Get-Date -Format 'yyyyMMdd-HHmmssfff'
$runId = [guid]::NewGuid().ToString('N').Substring(0, 8)
$safePackage = $Package -replace '[^A-Za-z0-9_.-]', '_'
$output = Join-Path $repo "local/external/umodel/$safePackage-$stamp-$runId"
New-Item -ItemType Directory -Force -Path $output | Out-Null
$log = Join-Path $output 'run.log'

$argsList = @(
    "-path=$cookedPath", '-game=border', '-export', "-$MeshFormat",
    '-lods', '-dds', '-nooverwrite', '-uncook', "-out=$output"
)
if ($Sounds) { $argsList += '-sounds' }
$argsList += $Package
if ($Object) { $argsList += $Object }

Write-Output "UModel: $umodelPath"
Write-Output "Package: $Package$(if ($Object) { " / $Object" })"
Write-Output "Output: $output"
$sw = [Diagnostics.Stopwatch]::StartNew()
& $umodelPath @argsList 2>&1 | Tee-Object -FilePath $log
$exitCode = $LASTEXITCODE
$sw.Stop()
$files = @(Get-ChildItem -LiteralPath $output -Recurse -File)
$bytes = [int64](($files | Measure-Object -Property Length -Sum).Sum)
Write-Output "Exit: $exitCode"
Write-Output "Elapsed seconds: $([math]::Round($sw.Elapsed.TotalSeconds, 2))"
Write-Output "Files: $($files.Count)"
Write-Output "Bytes: $bytes"
if ($exitCode -ne 0) { exit $exitCode }
