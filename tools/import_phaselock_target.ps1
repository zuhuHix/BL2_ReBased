[CmdletBinding()]
param(
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    [string]$Game = $env:OPENWILLOW_BL2,
    # UModel build 1590 (external, never inside the repository).
    [string]$UModel = $env:OPENWILLOW_UMODEL,
    [string]$Python = 'python',
    # prepare: identity, UModel exports, editor job; mesh: editor import of mesh/materials + reference pose;
    # anims: MD5 clips -> bone tracks, then the editor creates the AnimSequences; zone: the zone-colour material
    # (tools/phaselock_target_editor.py). 'all' runs them in order.
    [ValidateSet('all', 'prepare', 'mesh', 'anims', 'zone')][string[]]$Steps = @('all')
)
# AI-assisted. Imports a stock Phaselock target (the bullymong, tools/prepare_phaselock_target.py) into the ignored UE
# content folder /Game/OpenWillow/Characters/PhaselockTarget, reusing the slice NPC editor script
# (tools/slice_npc_editor.py) with this tool's own job file. Game-derived data stays under local/ and the ignored
# Content folder; nothing is added to Git. The host uses it with -owcombattest -owcombatpawn=Bullymong.
#
# The editor runs under the worktree's local/ue_run.lock (as tools/seed_slice_npc_assets.ps1 does). On a machine where
# several lanes share UE, take the machine-wide lock first; this script does not manage it.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$cmd = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor-Cmd.exe'
$lock = Join-Path $repo 'local/ue_run.lock'
$out = Join-Path $repo 'local/phaselock/target'
$logs = Join-Path $out 'logs'
$tool = Join-Path $PSScriptRoot 'prepare_phaselock_target.py'
$editorScript = Join-Path $PSScriptRoot 'slice_npc_editor.py'
New-Item -ItemType Directory -Force -Path $out, $logs | Out-Null
$run = { param($name) $Steps -contains 'all' -or $Steps -contains $name }

function Invoke-Tool([string]$Step) {
    $extra = @()
    if ($Game) { $extra += @('--game', $Game) }
    if ($UModel) { $extra += @('--umodel', $UModel) }
    & $Python $tool $Step @extra
    if ($LASTEXITCODE -ne 0) { throw "prepare_phaselock_target.py $Step failed: $LASTEXITCODE" }
}

function Invoke-Editor([string]$Mode, [string]$Script = $editorScript) {
    if (Get-Process UnrealEditor, UnrealEditor-Cmd -ErrorAction SilentlyContinue) {
        throw 'An Unreal editor process is already running; leaving it untouched.'
    }
    $stream = [System.IO.File]::Open($lock, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::Read)
    $bytes = [System.Text.Encoding]::UTF8.GetBytes("import_phaselock_target $((Get-Date).ToString('o'))")
    $stream.Write($bytes, 0, $bytes.Length); $stream.Dispose()
    $env:OPENWILLOW_SLICE_JOB = Join-Path $out 'editor_job.json'
    $env:OPENWILLOW_SLICE_MODE = $Mode
    $log = Join-Path $logs "editor-$Mode.log"
    $report = Join-Path $out "editor_report_$Mode.json"
    if (Test-Path -LiteralPath $report) { Remove-Item -LiteralPath $report -Force }
    try {
        $process = Start-Process -FilePath $cmd -PassThru -NoNewWindow -RedirectStandardOutput (Join-Path $logs "editor-$Mode.stdout.txt") -ArgumentList @(
            "`"$project`"", '-run=pythonscript', "`"-script=$Script`"", '-unattended', '-nullrhi', '-nosplash',
            '-ddc=InstalledNoZenLocalFallback', "`"-abslog=$log`"")
        $null = $process.Handle   # without this, Start-Process -PassThru does not populate ExitCode
        if (!$process.WaitForExit(1500000)) { Stop-Process -Id $process.Id -Force; throw "editor mode $Mode timed out" }
        $code = $process.ExitCode
    } finally {
        Remove-Item -LiteralPath $lock -Force -ErrorAction SilentlyContinue
    }
    if ($code -ne 0 -or !(Test-Path -LiteralPath $report)) { throw "editor mode $Mode failed (exit $code); see $log" }
    Select-String -LiteralPath $log -Pattern 'LogPython: OW_' | ForEach-Object { $_.Line -replace '.*LogPython: ', '' }
}

if (& $run 'prepare') { Invoke-Tool 'all' }
if (& $run 'mesh') { Invoke-Editor 'npcs' }
if (& $run 'anims') { Invoke-Tool 'anims'; Invoke-Editor 'anims' }
if (& $run 'zone') { Invoke-Editor 'zone' (Join-Path $PSScriptRoot 'phaselock_target_editor.py') }
