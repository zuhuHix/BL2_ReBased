param(
    [Parameter(Mandatory=$true)][string]$Engine,
    [Parameter(Mandatory=$true)][string]$Exports,
    # UModel MD5 export root holding Hands_Siren and the Unarmed/Pistol AnimSets;
    # when given, the arms animations are converted and imported too.
    [string]$Md5,
    [ValidateSet('front', 'face', 'back')][string]$View = 'front',
    [switch]$CaptureOnly,
    # Walk Sanctuary in first person as Maya (-owwalk -owmaya) instead of the preview level.
    [switch]$Sanctuary,
    # With -Sanctuary: walk forward unattended (captures the run animation).
    [switch]$AutoWalk,
    # Optional X,Y,Z point for a repeatable Sanctuary walking capture.
    [double[]]$Spawn,
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
    $log = Join-Path $out 'import.log'
    & $commandlet $project -run=pythonscript "-script=$(Join-Path $repo 'host/ue5/import_character.py')" -unattended -nullrhi -nosplash "-abslog=$log" | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Character import failed: $LASTEXITCODE; see $log" }
    Select-String -LiteralPath $log -Pattern 'LogPython: OW_CHARACTER' | ForEach-Object { $_.Line -replace '.*LogPython: ', '' }
    if ($Md5) {
        # The converter needs the reference pose of the skeleton just imported.
        $md5Root = (Resolve-Path -LiteralPath $Md5).Path
        $animDir = Join-Path $out 'anim'
        New-Item -ItemType Directory -Force -Path $animDir | Out-Null
        $animScript = Join-Path $repo 'host/ue5/import_character_anims.py'
        $env:OPENWILLOW_CHARACTER_REFERENCE = Join-Path $animDir 'ref_pose.json'
        $env:OPENWILLOW_CHARACTER_ANIMS = ''
        & $commandlet $project -run=pythonscript "-script=$animScript" -unattended -nullrhi -nosplash "-abslog=$(Join-Path $animDir 'reference.log')" | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "Reference pose dump failed: $LASTEXITCODE" }
        $tracks = Join-Path $animDir 'unarmed.json'
        & python (Join-Path $PSScriptRoot 'prepare_character_anims.py') --mesh (Join-Path $md5Root 'SkeletalMesh3/Hands_Siren.md5mesh') `
            --reference $env:OPENWILLOW_CHARACTER_REFERENCE --animset (Join-Path $md5Root 'AnimSet/1st_Person_Unarmed') `
            --clips Idle Run_F Sprint Jump_Start Jump_Idle Jump_End --output $tracks
        if ($LASTEXITCODE -ne 0) { throw "Animation conversion failed: $LASTEXITCODE" }
        $pistolTracks = Join-Path $animDir 'pistol.json'
        & python (Join-Path $PSScriptRoot 'prepare_character_anims.py') --mesh (Join-Path $md5Root 'SkeletalMesh3/Hands_Siren.md5mesh') `
            --reference $env:OPENWILLOW_CHARACTER_REFERENCE --animset (Join-Path $md5Root 'AnimSet/1st_Person_Pistol') `
            --clips Idle Run_F Sprint Jump_Start Jump_Idle Jump_End --output $pistolTracks
        if ($LASTEXITCODE -ne 0) { throw "Pistol animation conversion failed: $LASTEXITCODE" }
        $env:OPENWILLOW_CHARACTER_REFERENCE = ''
        $env:OPENWILLOW_CHARACTER_ANIMS = "Unarmed=$tracks;Pistol=$pistolTracks"
        $animLog = Join-Path $animDir 'anims.log'
        & $commandlet $project -run=pythonscript "-script=$animScript" -unattended -nullrhi -nosplash "-abslog=$animLog" | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "Animation import failed: $LASTEXITCODE; see $animLog" }
        Select-String -LiteralPath $animLog -Pattern 'LogPython: OW_ANIM' | ForEach-Object { $_.Line -replace '.*LogPython: ', '' }
    }
}
# Capture the game window itself; the preview level's camera auto-activates.
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class OwWin {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
}
'@
$shot = Join-Path $out "preview-$View.png"
$level = @('/Game/OpenWillow/Characters/Maya/MayaPreview')
if ($Sanctuary) {
    $shot = Join-Path $out 'sanctuary-maya.png'
    $level = @('/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya')
    if ($Spawn) {
        if ($Spawn.Count -ne 3) { throw '-Spawn requires X,Y,Z' }
        $level += @("-owspawnx=$($Spawn[0])", "-owspawny=$($Spawn[1])", "-owspawnz=$($Spawn[2])")
        $level += '-owspawnprobe'
        $shot = Join-Path $out 'sanctuary-maya-spawn.png'
    }
    if ($AutoWalk) { $level += '-owautowalk'; $shot = Join-Path $out 'sanctuary-maya-walk.png' }
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
    # PW_RENDERFULLCONTENT (2) captures the window even when another covers it.
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    $hdc = $graphics.GetHdc()
    [OwWin]::PrintWindow($game.MainWindowHandle, $hdc, 2) | Out-Null
    $graphics.ReleaseHdc($hdc)
    $bitmap.Save($shot)
    Write-Output "Screenshot: $shot"
} finally {
    if (!$game.HasExited) { Stop-Process -Id $game.Id }
}
