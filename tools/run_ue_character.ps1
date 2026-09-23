param(
    [Parameter(Mandatory=$true)][string]$Engine,
    [Parameter(Mandatory=$true)][string]$Exports,
    [string]$Pose,
    [ValidateSet('front', 'face', 'back')][string]$View = 'front',
    [switch]$CaptureOnly,
    # Walk Sanctuary in first person with these baked arms instead of the preview level.
    [ValidateSet('', 'Pistol_Idle', 'Unarmed_Idle')][string]$Arms = '',
    [int]$Wait = 30,
    # Launch for play (WASD/mouse/space) instead of capturing a screenshot.
    [switch]$Play
)
# Import Maya's UModel exports (host/ue5/import_character.py) and capture the
# preview level in -game mode. Close any editor that has MayaPreview open first:
# Windows locks the assets and the import cannot replace them.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$commandlet = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor-Cmd.exe'
$out = Join-Path $repo 'local/character'
New-Item -ItemType Directory -Force -Path $out | Out-Null
if (!$CaptureOnly) {
    $env:OPENWILLOW_CHARACTER = (Resolve-Path -LiteralPath $Exports).Path
    $env:OPENWILLOW_CHARACTER_VIEW = $View
    $env:OPENWILLOW_CHARACTER_POSE = if ($Pose) { (Resolve-Path -LiteralPath $Pose).Path } else { '' }
    $log = Join-Path $out 'import.log'
    & $commandlet $project -run=pythonscript "-script=$(Join-Path $repo 'host/ue5/import_character.py')" -unattended -nullrhi -nosplash "-abslog=$log" | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Character import failed: $LASTEXITCODE; see $log" }
    Select-String -LiteralPath $log -Pattern 'LogPython: OW_CHARACTER' | ForEach-Object { $_.Line -replace '.*LogPython: ', '' }
}
# Capture the game window itself; the preview level's camera auto-activates.
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class OwWin {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
}
'@
$shot = Join-Path $out "preview-$View.png"
$level = @('/Game/OpenWillow/Characters/Maya/MayaPreview')
if ($Arms) {
    $shot = Join-Path $out "sanctuary-$Arms.png"
    $level = @('/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk',
        "-owarms=/Game/OpenWillow/Characters/Maya/FirstPerson/SM_Arms_1st_Person_$Arms.SM_Arms_1st_Person_$Arms")
}
$gameArgs = @('-game', '-windowed', '-WinX=0', '-WinY=0', '-ResX=1280', '-ResY=720', '-nosplash')
if ($Play) {
    Start-Process -FilePath $editor -ArgumentList (@("`"$project`"") + $level + $gameArgs)
    return
}
$game = Start-Process -FilePath $editor -PassThru -ArgumentList (@("`"$project`"") + $level + $gameArgs)
try {
    Start-Sleep -Seconds $Wait
    $game.Refresh()
    [OwWin]::SetForegroundWindow($game.MainWindowHandle) | Out-Null
    Start-Sleep -Seconds 2
    $r = New-Object OwWin+RECT
    [OwWin]::GetWindowRect($game.MainWindowHandle, [ref]$r) | Out-Null
    $bitmap = New-Object Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
    [Drawing.Graphics]::FromImage($bitmap).CopyFromScreen($r.L, $r.T, 0, 0, $bitmap.Size)
    $bitmap.Save($shot)
    Write-Output "Screenshot: $shot"
} finally {
    if (!$game.HasExited) { Stop-Process -Id $game.Id }
}
