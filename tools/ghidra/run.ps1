[CmdletBinding()]
param(
    # Tables: find the native registration tables, write natives.tsv and gnatives.tsv. -Apply also labels every native.
    [switch]$Tables,
    [switch]$Apply,
    # Query: resolve and decompile. Each item is a query (see OwNativeQuery.java) or @<file> with one query per line.
    [string[]]$Query,
    [int]$Callees = 0,
    # Label the natives a query resolves (persisted in the project).
    [switch]$Label,
    [string]$Ghidra = $(if ($env:OPENWILLOW_GHIDRA) { $env:OPENWILLOW_GHIDRA } else { 'C:\Users\yorad\Tools\ghidra_12.1.4_PUBLIC' }),
    [string]$Jdk = $(if ($env:OPENWILLOW_JDK) { $env:OPENWILLOW_JDK } else { 'C:\Users\yorad\Tools\jdk-21.0.12.1+1' }),
    [string]$ProjectDir = $(if ($env:OPENWILLOW_ANALYSIS) { $env:OPENWILLOW_ANALYSIS } else { Join-Path $env:USERPROFILE 'bl2-analysis' }),
    [string]$ProjectName = 'bl2',
    [string]$Program = 'Borderlands2.exe',
    # Game-derived output (tables, decompilation). Outside the repository, or under its ignored local/.
    [string]$Out = '',
    # Open the project read-only (nothing is saved, labels included).
    [switch]$ReadOnly
)
# Drives the OpenWillow Ghidra scripts headlessly against the project made by tools/ghidra_import.ps1
# (docs/NATIVE_ANALYSIS.md, "Native registration and queries"). This script and the .java files contain no game data;
# everything they write is game-derived and stays outside the repository or under ignored local/.
# Only one analyzeHeadless may use a project at a time.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (!$Out) { $Out = Join-Path $ProjectDir 'out' }
$Out = [IO.Path]::GetFullPath($Out)
$local = [IO.Path]::GetFullPath((Join-Path $repo 'local')) + '\'
if ($Out.StartsWith($repo.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) -and
    !$Out.StartsWith($local, [StringComparison]::OrdinalIgnoreCase)) { throw "Output must be outside the repository or under local/: $Out" }
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$headless = Join-Path $Ghidra 'support\analyzeHeadless.bat'
if (!(Test-Path -LiteralPath $headless)) { throw "Ghidra not found at $Ghidra (set OPENWILLOW_GHIDRA)" }
if (!(Test-Path -LiteralPath (Join-Path $Jdk 'bin\java.exe'))) { throw "JDK 21 not found at $Jdk (set OPENWILLOW_JDK)" }
if (!$Tables -and !$Query) { throw 'Pass -Tables and/or -Query <query|@file>...' }
$env:JAVA_HOME = $Jdk
$env:PATH = "$Jdk\bin;$env:PATH"

function Invoke-Script([string]$name, [string[]]$scriptArgs) {
    $log = Join-Path $Out ("ghidra-{0}-{1}.log" -f $name, (Get-Date).ToString('yyyyMMdd-HHmmss'))
    $arguments = @($ProjectDir, $ProjectName, '-process', $Program, '-noanalysis', '-scriptPath', $PSScriptRoot,
                   '-postScript', "$name.java") + $scriptArgs + @('-log', $log)
    if ($ReadOnly) { $arguments += '-readOnly' }
    $sw = [Diagnostics.Stopwatch]::StartNew()
    # analyzeHeadless exits 0 even when the script throws, so script errors are detected from its output.
    $failed = $false
    & $headless @arguments | ForEach-Object {
        if ($_ -match 'SCRIPT ERROR|Exception') { $failed = $true }
        if ($_ -match '\(GhidraScript\)|ERROR|Exception') { $_ }
    }
    $code = $LASTEXITCODE
    Write-Output ("{0}: exit {1} after {2:N0} s (log {3})" -f $name, $code, $sw.Elapsed.TotalSeconds, $log)
    if ($code -ne 0 -or $failed) { throw "$name failed" }
}

if ($Tables) {
    $inatives = Join-Path $Out 'script_natives.tsv'
    $cooked = Join-Path $env:OPENWILLOW_BL2 'WillowGame\CookedPCConsole'
    if ($env:OPENWILLOW_BL2 -and (Test-Path -LiteralPath $cooked)) {
        python (Join-Path $PSScriptRoot 'script_natives.py') $inatives --cooked $cooked
        if ($LASTEXITCODE -ne 0) { throw 'script_natives.py failed' }
    } else { Write-Warning 'OPENWILLOW_BL2 not set: gnatives.tsv (script iNative -> native) is skipped' }
    $tableArgs = @($Out)
    if ($Apply) { $tableArgs += 'apply' }
    if (Test-Path -LiteralPath $inatives) { $tableArgs += "inatives:$inatives" }
    Invoke-Script 'OwNativeTables' $tableArgs
}
if ($Query) {
    $queryArgs = @($Out) + $Query + @("callees:$Callees")
    if ($Label) { $queryArgs += 'label' }
    Invoke-Script 'OwNativeQuery' $queryArgs
}
