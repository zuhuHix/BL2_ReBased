[CmdletBinding()]
param(
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    [string]$Game = $env:OPENWILLOW_BL2,
    # UModel build 1590 (external, never inside the repository).
    [string]$UModel = $env:OPENWILLOW_UMODEL,
    [string]$Python = 'python',
    # Any of: extract, import, preview, capture, manifest. 'all' runs them in order.
    [ValidateSet('all', 'extract', 'import', 'attach', 'outline', 'preview', 'capture', 'manifest')][string[]]$Steps = @('all'),
    [int]$CaptureWaitSeconds = 40,
    # Keep the already imported meshes/materials and only redo the clip conversion and import (after an animation fix).
    [switch]$AnimsOnly,
    # amb_compose output of the real game (tools/real_game/scripts/ambient_npcs.py) for the attach step.
    [string]$Compose = ''
)
# AI-assisted (Claude), 2026-10-04. Seeds the ignored local inputs and UE content for the Sanctuary ambient NPCs (the
# male and female generic citizens) with tools/ambient_npc_assets.py and the unchanged tools/slice_npc_editor.py run
# on its own job file (docs/verification/SANCTUARY_AMBIENT_NPCS.md). Game-derived data stays under local/ (UModel exports,
# manifests in local/slice/ambient) and the ignored UE Content folder; nothing is added to Git.
#
#   extract   tools/ambient_npc_assets.py: identity (our reader), UModel exports (mesh, textures, AnimSet)
#   import    editor: meshes/textures/materials + reference poses; convert MD5 clips; editor: anim sequences
#   attach    tools/ambient_npc_assets.py attachments (hair, hats, gear, head textures seen on the live citizens of a real-game capture,
#             -Compose) + tools/ambient_npc_attach_editor.py (static meshes, materials, matte master, ink-line material)
#   preview   editor (fresh session): reload every asset, record bounds/bone counts/anim lengths, build preview levels
#   capture   -game launches of the preview levels, one screenshot each (window capture)
#   manifest  merge everything into local/slice/ambient/ambient_assets.json
#
# Only one editor runs at a time and the shared local/ue_run.lock is held for each editor launch. If an UnrealEditor
# process that is not ours is running this script stops instead of touching it. It only (re)creates content under
# /Game/OpenWillow/Characters/Ambient (Marcus, the dummy, Maya and every weapon asset are untouched).
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (!$Game -or !(Test-Path -LiteralPath $Game)) { throw 'Pass -Game <Borderlands 2 folder> or set OPENWILLOW_BL2' }
if (!$UModel -or !(Test-Path -LiteralPath $UModel)) { throw 'Pass -UModel <umodel.exe> or set OPENWILLOW_UMODEL' }
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$cmd = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor-Cmd.exe'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$lock = Join-Path $repo 'local/ue_run.lock'
$ambient = Join-Path $repo 'local/slice/ambient'
$logs = Join-Path $repo 'local/slice/logs'
$tool = Join-Path $PSScriptRoot 'ambient_npc_assets.py'
$editorScript = Join-Path $PSScriptRoot 'slice_npc_editor.py'
New-Item -ItemType Directory -Force -Path $ambient, $logs | Out-Null
$env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
$env:OPENWILLOW_UMODEL = (Resolve-Path -LiteralPath $UModel).Path
$run = { param($name) $Steps -contains 'all' -or $Steps -contains $name }

function Invoke-Tool([string]$Step) {
    & $Python $tool $Step
    if ($LASTEXITCODE -ne 0) { throw "ambient_npc_assets.py $Step failed: $LASTEXITCODE" }
}

# Wait for the lock (another lane may hold it), claim it atomically, run the body, release it.
function Use-EditorLock([scriptblock]$Body) {
    $deadline = (Get-Date).AddMinutes(60); $stream = $null
    while (!$stream) {
        if (!(Get-Process UnrealEditor, UnrealEditor-Cmd, Borderlands2 -ErrorAction SilentlyContinue)) {
            try { $stream = [System.IO.File]::Open($lock, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::Read) } catch { $stream = $null }
        }
        if (!$stream) {
            if ((Get-Date) -gt $deadline) { throw 'Another editor/game or local/ue_run.lock is still present; leaving it untouched.' }
            Start-Sleep -Seconds 10
        }
    }
    $bytes = [System.Text.Encoding]::UTF8.GetBytes("B seed_ambient_npc_assets $((Get-Date).ToString('o'))")
    $stream.Write($bytes, 0, $bytes.Length); $stream.Dispose()
    try { & $Body } finally { Remove-Item -LiteralPath $lock -Force -ErrorAction SilentlyContinue }
}

function Invoke-Editor([string]$Mode) {
    $env:OPENWILLOW_SLICE_JOB = Join-Path $ambient 'editor_job.json'
    $env:OPENWILLOW_SLICE_MODE = $Mode
    $log = Join-Path $logs "ambient-editor-$Mode.log"
    $report = Join-Path $ambient "editor_report_$Mode.json"
    if (Test-Path -LiteralPath $report) { Remove-Item -LiteralPath $report -Force }
    Use-EditorLock {
        $process = Start-Process -FilePath $cmd -PassThru -NoNewWindow -RedirectStandardOutput (Join-Path $logs "ambient-editor-$Mode.stdout.txt") -ArgumentList @(
            "`"$project`"", '-run=pythonscript', "`"-script=$editorScript`"", '-unattended', '-nullrhi', '-nosplash',
            '-ddc=InstalledNoZenLocalFallback', "`"-abslog=$log`"")
        $null = $process.Handle   # without this, Start-Process -PassThru does not populate ExitCode
        try {
            if (!$process.WaitForExit(1500000)) { throw "editor mode $Mode timed out" }
            $code = $process.ExitCode
        } finally {
            if (!$process.HasExited) { Stop-Process -Id $process.Id -Force }
        }
        if ($code -ne 0 -or !(Test-Path -LiteralPath $report)) { throw "editor mode $Mode failed (exit $code); see $log" }
    }
    Select-String -LiteralPath $log -Pattern 'LogPython: OW_SLICE' | ForEach-Object { $_.Line -replace '.*LogPython: ', '' }
}

function Invoke-AttachEditor {
    $env:OPENWILLOW_AMBIENT_ATTACH_JOB = Join-Path $ambient 'attach_job.json'
    $log = Join-Path $logs 'ambient-editor-attach.log'
    $report = Join-Path $ambient 'editor_report_attach.json'
    if (Test-Path -LiteralPath $report) { Remove-Item -LiteralPath $report -Force }
    $script = Join-Path $PSScriptRoot 'ambient_npc_attach_editor.py'
    Use-EditorLock {
        $process = Start-Process -FilePath $cmd -PassThru -NoNewWindow -RedirectStandardOutput (Join-Path $logs 'ambient-editor-attach.stdout.txt') -ArgumentList @(
            "`"$project`"", '-run=pythonscript', "`"-script=$script`"", '-unattended', '-nullrhi', '-nosplash',
            '-ddc=InstalledNoZenLocalFallback', "`"-abslog=$log`"")
        $null = $process.Handle
        try {
            if (!$process.WaitForExit(1500000)) { throw 'attach editor timed out' }
            $code = $process.ExitCode
        } finally { if (!$process.HasExited) { Stop-Process -Id $process.Id -Force } }
        if ($code -ne 0 -or !(Test-Path -LiteralPath $report)) { throw "attach editor failed (exit $code); see $log" }
    }
    Select-String -LiteralPath $log -Pattern 'LogPython: OW_ATTACH' | ForEach-Object { $_.Line -replace '.*LogPython: ', '' }
}

function Invoke-OutlineEditor {
    # Only the ink-line material is rebuilt (the attach report and every other asset stay as they are).
    $env:OPENWILLOW_AMBIENT_ATTACH_JOB = Join-Path $ambient 'attach_job.json'
    $env:OPENWILLOW_AMBIENT_OUTLINE_ONLY = '1'
    $log = Join-Path $logs 'ambient-editor-outline.log'
    $script = Join-Path $PSScriptRoot 'ambient_npc_attach_editor.py'
    try {
        Use-EditorLock {
            $process = Start-Process -FilePath $cmd -PassThru -NoNewWindow -RedirectStandardOutput (Join-Path $logs 'ambient-editor-outline.stdout.txt') -ArgumentList @(
                "`"$project`"", '-run=pythonscript', "`"-script=$script`"", '-unattended', '-nullrhi', '-nosplash',
                '-ddc=InstalledNoZenLocalFallback', "`"-abslog=$log`"")
            $null = $process.Handle
            try {
                if (!$process.WaitForExit(900000)) { throw 'outline editor timed out' }
                $code = $process.ExitCode
            } finally { if (!$process.HasExited) { Stop-Process -Id $process.Id -Force } }
            if ($code -ne 0 -or !(Select-String -LiteralPath $log -Pattern 'OW_ATTACH outline material rebuilt' -Quiet)) { throw "outline editor failed (exit $code); see $log" }
        }
    } finally { Remove-Item Env:OPENWILLOW_AMBIENT_OUTLINE_ONLY -ErrorAction SilentlyContinue }
}

Add-Type -AssemblyName System.Drawing
if (-not ('OwAmbWin' -as [type])) {
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class OwAmbWin {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
}
'@
}

function Save-LevelShot([string]$Level, [string]$File) {
    $log = Join-Path $logs ("ambient-capture-" + [IO.Path]::GetFileNameWithoutExtension($File) + '.log')
    Use-EditorLock {
        $process = Start-Process -FilePath $editor -PassThru -ArgumentList @(
            "`"$project`"", $Level, '-game', '-windowed', '-WinX=0', '-WinY=0', '-ResX=1280', '-ResY=720', '-nosplash',
            '-unattended', '-ddc=InstalledNoZenLocalFallback', '-d3d11', "`"-abslog=$log`"")
        try {
            Start-Sleep -Seconds $CaptureWaitSeconds
            $process.Refresh()
            if ($process.HasExited) { throw "game exited early for $Level; see $log" }
            $r = New-Object OwAmbWin+RECT
            [OwAmbWin]::GetWindowRect($process.MainWindowHandle, [ref]$r) | Out-Null
            $bitmap = New-Object Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
            $graphics = [Drawing.Graphics]::FromImage($bitmap)
            $hdc = $graphics.GetHdc()
            [OwAmbWin]::PrintWindow($process.MainWindowHandle, $hdc, 2) | Out-Null
            $graphics.ReleaseHdc($hdc)
            $bitmap.Save($File)
            Write-Output "Screenshot: $File ($($r.R - $r.L)x$($r.B - $r.T))"
        } finally {
            if (!$process.HasExited) { Stop-Process -Id $process.Id -Force }
            Start-Sleep -Seconds 3
        }
    }
}

if (& $run 'extract') {
    foreach ($step in 'identity', 'extract') { Invoke-Tool $step }
}
if (& $run 'import') {
    Invoke-Tool 'editor-job'
    if (!$AnimsOnly) { Invoke-Editor 'npcs' }  # meshes, textures, materials, reference poses
    Invoke-Tool 'anims'                       # MD5 clips -> bone tracks, using the dumped reference poses
    Invoke-Editor 'anims'
}
if (& $run 'attach') {
    if ($Compose) { & $Python $tool attachments --compose $Compose } else { & $Python $tool attachments }
    if ($LASTEXITCODE -ne 0) { throw 'ambient_npc_assets.py attachments failed' }
    Invoke-AttachEditor
}
if ($Steps -contains 'outline') { Invoke-OutlineEditor }
if (& $run 'preview') { Invoke-Editor 'preview' }
if (& $run 'capture') {
    $shots = Join-Path $ambient 'screenshots'
    New-Item -ItemType Directory -Force -Path $shots | Out-Null
    $report = Get-Content -LiteralPath (Join-Path $ambient 'editor_report_preview.json') -Raw | ConvertFrom-Json
    foreach ($name in $report.PSObject.Properties.Name) {
        $entry = $report.$name
        foreach ($key in 'level', 'level_walk') {
            if ($entry.PSObject.Properties.Name -contains $key) {
                $label = if ($key -eq 'level') { $name } else { "$name-walk" }
                Save-LevelShot $entry.$key (Join-Path $shots "$label.png")
            }
        }
    }
}
if (& $run 'manifest') { Invoke-Tool 'manifest' }
Write-Output 'seed_ambient_npc_assets finished'
