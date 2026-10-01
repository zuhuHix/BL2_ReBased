[CmdletBinding()]
param(
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    [string]$Game = $env:OPENWILLOW_BL2,
    # UModel build 1590 (external, never inside the repository).
    [string]$UModel = $env:OPENWILLOW_UMODEL,
    [string]$Python = 'python',
    # Any of: extract, import, preview, capture, manifest. 'all' runs them in order.
    [ValidateSet('all', 'extract', 'import', 'preview', 'capture', 'manifest')][string[]]$Steps = @('all'),
    [int]$CaptureWaitSeconds = 40,
    # Capture only these preview names (Marcus, Marcus-walk, TargetDummy, pistol); default all.
    [string[]]$Only = @()
)
# AI-assisted. Seeds the ignored local inputs and UE content for the Sanctuary slice's Marcus, target dummy and
# stock Maliwan pistol (see docs/verification/SLICE_NPC_ASSETS.md). Game-derived data stays under local/ (UModel
# exports, manifests) and the ignored UE Content folder; nothing is added to Git.
#
#   extract   tools/slice_npc_assets.py: identity (our reader), UModel exports, texture cross-check, pistol rolls
#   import    editor: meshes/textures/materials + reference poses; convert MD5 clips; editor: anim sequences; pistol
#   preview   editor (fresh session): reload every asset, record bounds/bone counts, build preview levels
#   capture   -game launches of the preview levels, one screenshot each (window capture)
#   manifest  merge everything into local/slice/npc_assets.json
#
# Only one editor runs at a time and the shared local/ue_run.lock is held for each editor launch. If an
# UnrealEditor process that is not ours is running, this script stops instead of touching it.
# It never edits Maya's assets or the existing weapon items; it only (re)creates content under
# /Game/OpenWillow/Characters/{Marcus,TargetDummy,Shared} and /Game/OpenWillow/Weapons/MaliwanPistol.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (!$Game -or !(Test-Path -LiteralPath $Game)) { throw 'Pass -Game <Borderlands 2 folder> or set OPENWILLOW_BL2' }
if (!$UModel -or !(Test-Path -LiteralPath $UModel)) { throw 'Pass -UModel <umodel.exe> or set OPENWILLOW_UMODEL' }
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$cmd = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor-Cmd.exe'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$lock = Join-Path $repo 'local/ue_run.lock'
$slice = Join-Path $repo 'local/slice'
$logs = Join-Path $slice 'logs'
$tool = Join-Path $PSScriptRoot 'slice_npc_assets.py'
$editorScript = Join-Path $PSScriptRoot 'slice_npc_editor.py'
New-Item -ItemType Directory -Force -Path $slice, $logs | Out-Null
$env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
$env:OPENWILLOW_UMODEL = (Resolve-Path -LiteralPath $UModel).Path
$run = { param($name) $Steps -contains 'all' -or $Steps -contains $name }

function Invoke-Tool([string]$Step) {
    & $Python $tool $Step
    if ($LASTEXITCODE -ne 0) { throw "slice_npc_assets.py $Step failed: $LASTEXITCODE" }
}

function Use-EditorLock([scriptblock]$Body) {
    if (Get-Process UnrealEditor, UnrealEditor-Cmd -ErrorAction SilentlyContinue) {
        throw 'An Unreal editor process is already running; leaving it untouched.'
    }
    $stream = [System.IO.File]::Open($lock, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::Read)
    $bytes = [System.Text.Encoding]::UTF8.GetBytes("seed_slice_npc_assets $((Get-Date).ToString('o'))")
    $stream.Write($bytes, 0, $bytes.Length); $stream.Dispose()
    try { & $Body } finally { Remove-Item -LiteralPath $lock -Force -ErrorAction SilentlyContinue }
}

function Invoke-Editor([string]$Mode) {
    $env:OPENWILLOW_SLICE_JOB = Join-Path $slice 'editor_job.json'
    $env:OPENWILLOW_SLICE_MODE = $Mode
    $log = Join-Path $logs "editor-$Mode.log"
    $report = Join-Path $slice "editor_report_$Mode.json"
    if (Test-Path -LiteralPath $report) { Remove-Item -LiteralPath $report -Force }
    Use-EditorLock {
        $process = Start-Process -FilePath $cmd -PassThru -NoNewWindow -RedirectStandardOutput (Join-Path $logs "editor-$Mode.stdout.txt") -ArgumentList @(
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

Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class OwSliceWin {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
}
'@

function Save-LevelShot([string]$Level, [string]$File) {
    $log = Join-Path $logs ("capture-" + [IO.Path]::GetFileNameWithoutExtension($File) + '.log')
    Use-EditorLock {
        $process = Start-Process -FilePath $editor -PassThru -ArgumentList @(
            "`"$project`"", $Level, '-game', '-windowed', '-WinX=0', '-WinY=0', '-ResX=1280', '-ResY=720', '-nosplash',
            '-unattended', '-ddc=InstalledNoZenLocalFallback', '-d3d11', "`"-abslog=$log`"")
        try {
            Start-Sleep -Seconds $CaptureWaitSeconds
            $process.Refresh()
            if ($process.HasExited) { throw "game exited early for $Level; see $log" }
            $r = New-Object OwSliceWin+RECT
            [OwSliceWin]::GetWindowRect($process.MainWindowHandle, [ref]$r) | Out-Null
            $bitmap = New-Object Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
            $graphics = [Drawing.Graphics]::FromImage($bitmap)
            $hdc = $graphics.GetHdc()
            # PW_RENDERFULLCONTENT (2) captures the window even when another one covers it.
            [OwSliceWin]::PrintWindow($process.MainWindowHandle, $hdc, 2) | Out-Null
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
    foreach ($step in 'identity', 'extract', 'pistol', 'crosscheck') { Invoke-Tool $step }
}
if (& $run 'import') {
    Invoke-Tool 'editor-job'
    Invoke-Editor 'npcs'                      # meshes, textures, materials, reference poses
    Invoke-Tool 'anims'                       # MD5 clips -> bone tracks, using the dumped reference poses
    Invoke-Tool 'editor-job'
    Invoke-Editor 'anims'
    Invoke-Editor 'pistol'
}
if (& $run 'preview') { Invoke-Editor 'preview' }
if (& $run 'capture') {
    $shots = Join-Path $slice 'screenshots'
    New-Item -ItemType Directory -Force -Path $shots | Out-Null
    $report = Get-Content -LiteralPath (Join-Path $slice 'editor_report_preview.json') -Raw | ConvertFrom-Json
    foreach ($name in $report.PSObject.Properties.Name) {
        $entry = $report.$name
        foreach ($key in 'level', 'level_walk') {
            if ($entry.PSObject.Properties.Name -contains $key) {
                $label = if ($key -eq 'level') { $name } else { "$name-walk" }
                if ($Only.Count -gt 0 -and $Only -notcontains $label) { continue }
                Save-LevelShot $entry.$key (Join-Path $shots "$label.png")
            }
        }
    }
}
if (& $run 'manifest') { Invoke-Tool 'manifest' }
Write-Output 'seed_slice_npc_assets finished'
