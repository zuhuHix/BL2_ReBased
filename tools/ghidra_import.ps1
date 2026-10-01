[CmdletBinding()]
param(
    [string]$Ghidra = $(if ($env:OPENWILLOW_GHIDRA) { $env:OPENWILLOW_GHIDRA } else { 'C:\Users\yorad\Tools\ghidra_12.1.4_PUBLIC' }),
    [string]$Jdk = $(if ($env:OPENWILLOW_JDK) { $env:OPENWILLOW_JDK } else { 'C:\Users\yorad\Tools\jdk-21.0.12.1+1' }),
    [string]$Game = $env:OPENWILLOW_BL2,
    [string]$Binary = 'Binaries\Win32\Borderlands2.exe',
    [string]$ProjectName = 'bl2',
    # Ghidra refuses project paths with a folder starting with '.' (this worktree lives under .t3), so the project goes outside
    # every worktree: %OPENWILLOW_ANALYSIS% or %USERPROFILE%\bl2-analysis.
    [string]$ProjectDir = $(if ($env:OPENWILLOW_ANALYSIS) { $env:OPENWILLOW_ANALYSIS } else { Join-Path $env:USERPROFILE 'bl2-analysis' }),
    [int]$AnalysisTimeoutSeconds = 7200,
    # Re-import and replace the program if the project already has it.
    [switch]$Overwrite
)
# Imports the player's own Borderlands 2 executable into a Ghidra project outside the repository and runs auto-analysis
# headlessly (docs/LEGAL.md, "Analysing the executable"; docs/NATIVE_ANALYSIS.md). Everything this writes is game-derived
# data and must never be committed; the repository hook and .gitignore refuse the database file types.
# This script contains no game data. Ghidra and the JDK live outside the repository (THIRD_PARTY.md).
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (!$Game -or !(Test-Path -LiteralPath $Game)) { throw 'Pass -Game <Borderlands 2 folder> or set OPENWILLOW_BL2' }
$exe = Join-Path $Game $Binary
if (!(Test-Path -LiteralPath $exe)) { throw "Missing $exe" }
$headless = Join-Path $Ghidra 'support\analyzeHeadless.bat'
if (!(Test-Path -LiteralPath $headless)) { throw "Ghidra not found at $Ghidra (set OPENWILLOW_GHIDRA)" }
if (!(Test-Path -LiteralPath (Join-Path $Jdk 'bin\java.exe'))) { throw "JDK 21 not found at $Jdk (set OPENWILLOW_JDK)" }
$project = [IO.Path]::GetFullPath($ProjectDir)
if ($project.StartsWith($repo.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) { throw "Keep the Ghidra project outside the repository: $project" }
New-Item -ItemType Directory -Force -Path $project | Out-Null
# Work on a copy under local/: analyzeHeadless.bat breaks on paths with parentheses (the usual "Program Files (x86)" Steam
# folder), and this keeps the install untouched.
$input = Join-Path $project 'input'
New-Item -ItemType Directory -Force -Path $input | Out-Null
$copy = Join-Path $input (Split-Path -Leaf $exe)
if (!(Test-Path -LiteralPath $copy) -or (Get-Item -LiteralPath $copy).Length -ne (Get-Item -LiteralPath $exe).Length) { Copy-Item -LiteralPath $exe -Destination $copy -Force }
$exe = $copy
$env:JAVA_HOME = $Jdk
$env:PATH = "$Jdk\bin;$env:PATH"
$log = Join-Path $project "import-$((Get-Date).ToString('yyyyMMdd-HHmmss')).log"
$arguments = @($project, $ProjectName, '-import', $exe, '-analysisTimeoutPerFile', $AnalysisTimeoutSeconds, '-log', $log)
if ($Overwrite) { $arguments += '-overwrite' }
Write-Output "Importing $exe into $project ($ProjectName); log: $log"
$sw = [Diagnostics.Stopwatch]::StartNew()
& $headless @arguments
Write-Output ("analyzeHeadless exit {0} after {1:N0} s" -f $LASTEXITCODE, $sw.Elapsed.TotalSeconds)
exit $LASTEXITCODE
